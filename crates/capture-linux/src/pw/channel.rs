//! Better pipewire channels.

use gulf_core::unreachable_unchecked;
use gulf_platform_linux::{atomic_fd::AtomicFd, errno::Errno};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    error::{self, Error as _},
    ffi::c_int,
    fmt, hint,
    io::{self, Error, ErrorKind},
    num::NonZero,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd},
    slice,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::{
    io::{Interest, unix::AsyncFd},
    task::yield_now,
};

struct Channel<T> {
    /// The queue of messages that need to be received.
    buffer: Mutex<VecDeque<T>>,
    /// This is `-1` if the writer has dropped, otherwise it is a valid file descriptor.
    ///
    /// This will be set to -1 if the writer is dropped.
    ///
    /// While a writer exists, this must not be modified.
    sender: AtomicFd,
    /// This is `-1` if the reader has dropped, otherwise it is a valid file descriptor.
    ///
    /// This will be set to -1 if the receiver is dropped.
    ///
    /// While a receiver exists, this must not be modfied.
    receiver: AtomicFd,
}

pub struct Sender<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    #[inline(always)]
    #[must_use]
    pub fn has_receiver(&self) -> bool {
        self.channel.receiver.is_some(Ordering::Acquire)
    }

    pub async fn send_one_async(
        &mut self,
        item: T,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<T>> {
        self.send_many_async([item], timeout)
            .await
            .map_err(|error| error.map(|[item]| item))
    }

    pub async fn send_many_async<I>(
        &mut self,
        iter: I,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<I>>
    where
        I: IntoIterator<Item = T>,
    {
        // NOTE: Fast path!
        if !self.has_receiver() {
            hint::cold_path();

            return Err(SendError::Closed(Some(iter)));
        }

        let fd = match AsyncFd::with_interest(self.as_fd(), Interest::WRITABLE) {
            Ok(fd) => fd,
            Err(error) => {
                hint::cold_path();

                return Err(SendError::Other(Some(iter), error));
            }
        };

        let start = Instant::now();
        let mut iter = Some(iter);
        let mut count = 0_usize;

        'main: loop {
            let Some(duration) = timeout.map_or(Some(Duration::MAX), |timeout| {
                timeout.checked_sub(start.elapsed())
            }) else {
                hint::cold_path();

                return Err(SendError::TimedOut(iter));
            };

            match tokio::time::timeout(duration, fd.writable()).await {
                Ok(Ok(mut ready)) => {
                    // NOTE: We're ready for a write, so immediately push the iterator if we have it.
                    //
                    //       If we don't, then we already pushed and now we're just trying to write the count.
                    if iter.is_some() {
                        let mut buffer = match timeout {
                            Some(timeout) => {
                                let Some(timeout) = start.checked_add(timeout) else {
                                    hint::cold_path();

                                    return Err(SendError::TimedOut(iter));
                                };

                                match self.channel.buffer.try_lock_until(timeout) {
                                    Some(buffer) => buffer,
                                    None => {
                                        hint::cold_path();

                                        return Err(SendError::TimedOut(iter));
                                    }
                                }
                            }
                            None => self.channel.buffer.lock(),
                        };

                        count = {
                            let old = buffer.len();
                            buffer
                                .extend(iter.take().expect("someone took the iterator before us"));

                            buffer.len() - old
                        };
                    }

                    // SAFETY: `usize` is POD, an we're just reinterpreting it as a byte buffer of equal size.
                    let message: &[u8; size_of::<usize>()] =
                        unsafe { (&raw const count).cast::<[_; _]>().as_ref_unchecked() };

                    'write: loop {
                        // NOTE: We've already inserted the items, now we need to notify the receiver.
                        //
                        // SAFETY: We're just calling write with a valid FD and buffer.
                        let result = unsafe {
                            libc::write(
                                ready.get_inner().as_raw_fd(),
                                message.as_ptr().cast(),
                                message.len(),
                            )
                        };

                        match result {
                            -1 => match Errno::last() {
                                // NOTE: We got interrupted, continue trying.
                                Errno(libc::EINTR) => {
                                    // NOTE: We want the parent future to run before we try reading again.
                                    yield_now().await;

                                    // NOTE: We're done yielding control, so now we need to ensure we
                                    //       haven't timed out in the interim.
                                    if start.elapsed() >= duration {
                                        hint::cold_path();

                                        return Err(SendError::TimedOut(iter));
                                    }

                                    continue 'write;
                                }

                                // NOTE: The write would block.
                                #[allow(unreachable_patterns)]
                                Errno(libc::EWOULDBLOCK | libc::EAGAIN) => {
                                    // NOTE: If we're gonna block, we want to get a new readiness and try again.
                                    ready.clear_ready();
                                    continue 'main;
                                }
                                // NOTE: The receiver has closed.
                                Errno(libc::EPIPE) => {
                                    hint::cold_path();

                                    return Err(SendError::Closed(iter));
                                }
                                // NOTE: Some other error has occurred.
                                errno => {
                                    hint::cold_path();

                                    return Err(SendError::Other(iter, errno.into()));
                                }
                            },
                            // NOTE: We are done writing!
                            written if written.try_into().ok() == Some(message.len()) => {
                                return Ok(());
                            }
                            // NOTE: We want to catch weird shit.
                            written => unreachable!(
                                "failed to write {message} bytes, wrote {written} bytes instead",
                                message = message.len(),
                                written = written,
                            ),
                        }
                    }
                }
                Ok(Err(error)) => {
                    hint::cold_path();

                    return Err(match error.kind() {
                        ErrorKind::BrokenPipe => SendError::Closed(iter),
                        ErrorKind::TimedOut => SendError::TimedOut(iter),
                        _ => SendError::Other(iter, error),
                    });
                }
                Err(_elapsed_error) => {
                    hint::cold_path();

                    return Err(SendError::TimedOut(iter));
                }
            }
        }
    }

    pub fn send_one(
        &mut self,
        item: T,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<T>> {
        self.send_many([item], timeout)
            .map_err(|err| err.map(|[item]| item))
    }

    // FIXME: Make the underlying logic a lot closer to what is found in the async version.
    pub fn send_many<I>(
        &mut self,
        iter: I,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<I>>
    where
        I: IntoIterator<Item = T>,
    {
        // NOTE: This is just a fast path.
        if !self.has_receiver() {
            hint::cold_path();
            return Err(SendError::Closed(Some(iter)));
        }

        // NOTE: We use this to keep track of the remaining timeout we have.
        let start = Instant::now();

        // NOTE: This loop awaits until we're ready to start writing the data, and returns a mutex guard for the buffer.
        #[allow(clippy::let_and_return)]
        let mut buffer = loop {
            let poll_timeout: NonZero<c_int> = 'timeout: {
                // NOTE: We're gonna block if we're not provided a timeout.
                let Some(timeout) = timeout else {
                    break 'timeout const { NonZero::new(-1).unwrap() };
                };

                // NOTE: We're gonna return early if we cannot time out for at least a millisecond.
                let Some(timeout @ 1..) = timeout
                    .checked_sub(start.elapsed())
                    .as_ref()
                    .map(Duration::as_millis)
                else {
                    hint::cold_path();
                    return Err(SendError::TimedOut(Some(iter)));
                };

                // NOTE: If the timeout is technically too large, then we just saturate to
                //       the largest possible timeout.
                let timeout = NonZero::new(timeout)
                    .unwrap()
                    .try_into()
                    .unwrap_or(const { NonZero::new(c_int::MAX).unwrap() });

                timeout
            };

            // NOTE: This is the poll file descriptor we're keeping track of.
            let mut poll_fd = libc::pollfd {
                fd: self.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };

            // SAFETY: We're polling one file descriptor.
            match unsafe { libc::poll(&mut poll_fd, 1, poll_timeout.get()) } {
                // NOTE: `poll` returns `-1` upon some error.
                -1 => match Errno::last() {
                    // NOTE: We were interrupted.
                    Errno(libc::EINTR | libc::EAGAIN) => continue,
                    // NOTE: We experienced a timeout that isn't communicated normally.
                    Errno(libc::ETIMEDOUT) => {
                        hint::cold_path();
                        return Err(SendError::TimedOut(Some(iter)));
                    }
                    // NOTE: We experienced some other error.
                    errno => {
                        hint::cold_path();
                        return Err(SendError::Other(Some(iter), errno.into()));
                    }
                },
                // NOTE: `poll` returns zero upon timeout.
                0 => {
                    hint::cold_path();
                    return Err(SendError::TimedOut(Some(iter)));
                }
                _ => match poll_fd.revents {
                    // NOTE: These indicate that some error has occurred.
                    revents if revents & (libc::POLLHUP | libc::POLLERR | libc::POLLRDHUP) != 0 => {
                        hint::cold_path();

                        return Err(SendError::Closed(Some(iter)));
                    }
                    // NOTE: We can write!
                    revents if revents & libc::POLLOUT != 0 => {
                        let buffer = match timeout {
                            // NOTE: We have a timeout, and we we'll pass it to how we lock the mutex.
                            Some(timeout) => {
                                let Some(timeout) = start.checked_add(timeout) else {
                                    hint::cold_path();

                                    return Err(SendError::TimedOut(Some(iter)));
                                };

                                match self.channel.buffer.try_lock_until(timeout) {
                                    // NOTE: We successfully locked within time!
                                    Some(buffer) => buffer,
                                    None => {
                                        hint::cold_path();

                                        return Err(SendError::TimedOut(Some(iter)));
                                    }
                                }
                            }
                            // NOTE: We don't have a timeout, we're free to block.
                            None => self.channel.buffer.lock(),
                        };

                        // NOTE: We have the buffer lock, time to actually write the elements.
                        break buffer;
                    }
                    revents => unreachable!("unknown `revents`: {revents:02b}"),
                },
            }
        };

        // NOTE: This is the amount of elements we inserted.
        let count = {
            let old = buffer.len();
            buffer.extend(iter);

            buffer.len() - old
        };

        // NOTE: We're dropping the guard so that the receiver can access it.
        drop(buffer);

        // NOTE: Our message is just the amount of elements we sent. This is mostly advisory.
        //
        // SAFETY: `usize`s are POD, so getting a byte buffer of equivalent length is always safe.
        let message: &[u8; size_of::<usize>()] =
            unsafe { (&raw const count).cast::<[_; _]>().as_ref_unchecked() };

        // NOTE: Now we signal the receiver... We could check for timeout, but it's probably fine not to.
        loop {
            // SAFETY: We know it's safe to write to our file descriptor, and we know `message` is a valid thing to send.
            let result =
                unsafe { libc::write(self.as_raw_fd(), message.as_ptr().cast(), message.len()) };

            match result {
                -1 => match Errno::last() {
                    // NOTE: We got interrupted, continue trying.
                    Errno(libc::EINTR) => continue,
                    // NOTE: The receiver got closed before we expected.
                    Errno(libc::EPIPE) => {
                        hint::cold_path();

                        return Err(SendError::Closed(None));
                    }
                    // NOTE: Some other error occurred.
                    errno => {
                        hint::cold_path();

                        return Err(SendError::Other(None, errno.into()));
                    }
                },
                // NOTE: We successfully wrote all of the bytes we wanted to.
                written if written.try_into().ok() == Some(message.len()) => break,
                // NOTE: This exists mainly to catch weird shit.
                written => unreachable!(
                    "failed to write {message} bytes, wrote {written} bytes instead",
                    message = message.len(),
                    written = written,
                ),
            }
        }

        // NOTE: Yey!
        Ok(())
    }
}

// #[unsafe(no_mangle)]
// pub fn lol(
//     a: &mut Sender<String>,
//     b: String,
// ) -> () {
//     use std::pin::{Pin, pin};
//     use std::task::{Context, Waker};
//     let fut = pin!(a.send_one_async(b, None)).poll(&mut Context::from_waker(Waker::noop()));
// }

impl<T> AsFd for Sender<T> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: We own the sender, and thus we can load this in a relaxed fashion,
        //         as we know nobody else will be yoinking our value.
        match unsafe { self.channel.sender.load_borrowed(Ordering::Relaxed) } {
            Some(sender) => sender,
            None => unsafe {
                unreachable_unchecked!(
                    "the sender file descriptor has been dropped before we expected it to"
                )
            },
        }
    }
}

impl<T> AsRawFd for Sender<T> {
    #[inline(always)]
    fn as_raw_fd(&self) -> RawFd {
        self.as_fd().as_raw_fd()
    }
}

impl<T> Drop for Sender<T> {
    #[inline(always)]
    fn drop(&mut self) {
        drop(self.channel.sender.swap(None, Ordering::Release));
    }
}

/// An error that can occur when sending a value over a channel.
#[non_exhaustive]
pub enum SendError<T> {
    /// The channel is closed.
    Closed(Option<T>),
    /// We timed out while using the channel.
    TimedOut(Option<T>),
    /// Some other io error occurred.
    Other(Option<T>, Error),
}

impl<T> SendError<T> {
    /// Maps the inner `T`.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn map<F, U>(
        self,
        f: F,
    ) -> SendError<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            SendError::Closed(value) => SendError::Closed(value.map(f)),
            SendError::TimedOut(value) => SendError::TimedOut(value.map(f)),
            SendError::Other(value, error) => SendError::Other(value.map(f), error),
        }
    }

    /// Performs a flat mapping of the inner `T`.
    #[doc(alias = "flat_map")]
    #[doc(alias = "flatmap")]
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn and_then<F, U>(
        self,
        f: F,
    ) -> SendError<U>
    where
        F: FnOnce(T) -> Option<U>,
    {
        match self {
            SendError::Closed(value) => SendError::Closed(value.and_then(f)),
            SendError::TimedOut(value) => SendError::TimedOut(value.and_then(f)),
            SendError::Other(value, error) => SendError::Other(value.and_then(f), error),
        }
    }

    /// Get a reference to inner value.
    #[inline(always)]
    #[must_use]
    pub const fn get(&self) -> Option<&T> {
        match self {
            SendError::Closed(value) | SendError::TimedOut(value) | SendError::Other(value, _) => {
                value.as_ref()
            }
        }
    }

    /// Get a mutable reference to the inner value.
    #[inline(always)]
    #[must_use]
    pub const fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            SendError::Closed(value) | SendError::TimedOut(value) | SendError::Other(value, _) => {
                value.as_mut()
            }
        }
    }

    /// Get the inner value as a slice.
    #[inline(always)]
    #[must_use]
    pub const fn as_slice(&self) -> &[T] {
        match self {
            SendError::Closed(value) | SendError::TimedOut(value) | SendError::Other(value, _) => {
                value.as_slice()
            }
        }
    }

    /// Get the inner value as a mutable slice.
    #[inline(always)]
    #[must_use]
    pub const fn as_slice_mut(&mut self) -> &mut [T] {
        match self {
            SendError::Closed(value) | SendError::TimedOut(value) | SendError::Other(value, _) => {
                value.as_mut_slice()
            }
        }
    }
}

/// Exists to reduce monomorphization.
#[track_caller]
fn error_debug(
    variant: &str,
    fields: &[&dyn fmt::Debug],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut tuple = f.debug_tuple(variant);

    for field in fields {
        tuple.field(field);
    }

    tuple.finish_non_exhaustive()
}

/// Exists to reduce monomorphization.
#[track_caller]
fn error_display(
    message: &str,
    additional: &[&dyn fmt::Display],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    fmt::Display::fmt(message, f)?;

    for &next in additional {
        next.fmt(f)?;
    }

    Ok(())
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let other_fields;

        let (variant, fields): (_, &[_]) = match self {
            SendError::Closed(_) => ("Closed", &[]),
            SendError::TimedOut(_) => ("TimedOut", &[]),
            SendError::Other(_, error) => ("Other", {
                other_fields = error as &dyn fmt::Debug;

                slice::from_ref(&other_fields)
            }),
        };

        // This should hopefully help reduce monomorphization.
        error_debug(variant, fields, f)
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        #[allow(deprecated)]
        let message = self.description();

        let additional_other;
        let additional: &[_] = match self {
            SendError::Closed(_) => &[],
            SendError::TimedOut(_) => &[],
            SendError::Other(_, error) => {
                additional_other = error as &dyn fmt::Display;

                slice::from_ref(&additional_other)
            }
        };

        error_display(message, additional, f)
    }
}

impl<T> error::Error for SendError<T> {
    #[allow(deprecated)]
    #[inline(always)]
    fn description(&self) -> &str {
        match self {
            SendError::Closed(_) => "channel has closed",
            SendError::TimedOut(_) => "write exceeded timeout",
            SendError::Other(_, _) => "an unknown error has occurred",
        }
    }

    #[inline(always)]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            SendError::Closed(_) => None,
            SendError::TimedOut(_) => None,
            SendError::Other(_, error) => Some(error),
        }
    }
}

/// The receiver for a channel.
///
/// This ***cannot*** be cloned.
pub struct Receiver<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Receiver<T> {
    #[inline(always)]
    #[must_use]
    pub fn has_sender(&self) -> bool {
        self.channel.sender.is_some(Ordering::Acquire)
    }
}

impl<T> AsFd for Receiver<T> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: We own the receiver, and thus we can load this in a relaxed fashion as nobody will mess up our value.
        match unsafe { self.channel.receiver.load_borrowed(Ordering::Relaxed) } {
            Some(receiver) => receiver,
            None => unsafe {
                unreachable_unchecked!(
                    "the receiver file descriptor has been dropped before we expected it to"
                )
            },
        }
    }
}

impl<T> AsRawFd for Receiver<T> {
    #[inline(always)]
    fn as_raw_fd(&self) -> RawFd {
        self.as_fd().as_raw_fd()
    }
}

impl<T> Drop for Receiver<T> {
    #[inline(always)]
    fn drop(&mut self) {
        drop(self.channel.receiver.swap(None, Ordering::Release));
    }
}

pub fn channel<T>() -> io::Result<(Sender<T>, Receiver<T>)> {
    let [sender, receiver] = {
        let mut fds @ [receiver, sender] = [-1 as c_int; 2];

        // SAFETY: We know that `fds` is a valid file descriptor buffer,
        //         and that the file descriptors will be closed on exec.
        let result = unsafe {
            libc::pipe2(
                fds.as_mut_ptr(),
                libc::O_CLOEXEC | libc::O_DIRECT | libc::O_NONBLOCK,
            )
        };

        match result {
            -1 => {
                hint::cold_path();
                Err(Error::last_os_error())
            }
            _ => match [sender, receiver] {
                // NOTE: Just in case libc is fucked.
                [-1, _] | [_, -1] => {
                    hint::cold_path();
                    Err(Error::from_raw_os_error(libc::EBADF))
                }
                [sender, receiver] => {
                    // SAFETY: We know these file descriptors are valid.
                    Ok(unsafe { [OwnedFd::from_raw_fd(sender), OwnedFd::from_raw_fd(receiver)] })
                }
            },
        }
    }?;

    let channel = Arc::new(Channel {
        buffer: Mutex::new(VecDeque::new()),
        sender: sender.into(),
        receiver: receiver.into(),
    });

    let sender = Sender {
        channel: channel.clone(),
    };
    let receiver = Receiver { channel };

    Ok((sender, receiver))
}
