//! Better pipewire channels.

use gulf_core::unreachable_unchecked;
use gulf_platform_linux::errno::Errno;
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    ffi::{c_int, c_uint},
    hint, io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

struct Channel<T> {
    /// The queue of messages that need to be received.
    buffer: Mutex<VecDeque<T>>,
    /// This is `-1` if the writer has dropped, otherwise it is a valid file descriptor.
    ///
    /// This will be set to -1 if the writer is dropped.
    ///
    /// While a writer exists, this must not be modified.
    sender: AtomicI32,
    /// This is `-1` if the reader has dropped, otherwise it is a valid file descriptor.
    ///
    /// This will be set to -1 if the receiver is dropped.
    ///
    /// While a receiver exists, this must not be modfied.
    receiver: AtomicI32,
}

impl<T> Channel<T> {
    #[inline(always)]
    #[must_use]
    fn has_receiver(&self) -> bool {
        self.receiver.load(Ordering::Acquire) != -1
    }

    #[inline(always)]
    #[must_use]
    unsafe fn take_receiver(&self) -> Option<OwnedFd> {
        match self.receiver.swap(-1, Ordering::Release) {
            -1 => None,
            receiver => Some(unsafe { OwnedFd::from_raw_fd(receiver) }),
        }
    }

    #[inline(always)]
    #[must_use]
    unsafe fn borrow_receiver(&self) -> Option<BorrowedFd<'_>> {
        match self.receiver.load(Ordering::Acquire) {
            -1 => None,
            receiver => Some(unsafe { BorrowedFd::borrow_raw(receiver) }),
        }
    }

    #[inline(always)]
    #[must_use]
    fn has_sender(&self) -> bool {
        self.sender.load(Ordering::Acquire) != -1
    }

    #[inline(always)]
    #[must_use]
    unsafe fn take_sender(&self) -> Option<OwnedFd> {
        match self.sender.swap(-1, Ordering::Release) {
            -1 => None,
            sender => Some(unsafe { OwnedFd::from_raw_fd(sender) }),
        }
    }

    #[inline(always)]
    #[must_use]
    unsafe fn borrow_sender(&self) -> Option<BorrowedFd<'_>> {
        match self.receiver.load(Ordering::Acquire) {
            -1 => None,
            sender => Some(unsafe { BorrowedFd::borrow_raw(sender) }),
        }
    }
}

impl<T> Drop for Channel<T> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY: We own the receiver.
        drop(unsafe { self.take_receiver() });
        // SAFETY: We own the sender.
        drop(unsafe { self.take_sender() });
    }
}

pub struct Sender<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    #[inline(always)]
    #[must_use]
    pub fn has_receiver(&self) -> bool {
        self.channel.has_receiver()
    }

    pub fn send_many_timeout<I>(
        &mut self,
        iter: I,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<I>>
    where
        I: IntoIterator<Item = T>,
    {
        if !self.has_receiver() {
            hint::cold_path();
            return Err(SendError::Closed(Some(iter)));
        }

        let poll_timeout = match timeout {
            Some(timeout) if let Ok(timeout) = c_uint::try_from(timeout.as_millis()) => {
                timeout.cast_signed()
            }
            // If we don't specify a duration, block foreverrr.
            None => -1,
            // This should rarely occur.
            Some(..) => {
                hint::cold_path();
                return Err(SendError::Other(
                    io::Error::from_raw_os_error(libc::EINVAL),
                    Some(iter),
                ));
            }
        };

        let mut poll_fds: [libc::pollfd; _] = [libc::pollfd {
            fd: self.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        }];

        // SAFETY: It is safe to poll here.
        let result = unsafe {
            libc::poll(
                (&raw mut poll_fds).cast(),
                (&raw const poll_fds as *const [_])
                    .len()
                    .try_into()
                    .unwrap(),
                poll_timeout,
            )
        };

        let _count = match result {
            count @ 1.. => count,
            0 => {
                hint::cold_path();
                return Err(SendError::TimedOut(Some(iter)));
            }
            -1 => {
                hint::cold_path();
                return Err(SendError::Other(io::Error::last_os_error(), Some(iter)));
            }
            ..-1 => unreachable!("libc should only ever return `-1` on a poll error"),
        };

        if poll_fds[0].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            hint::cold_path();
            return Err(SendError::Closed(Some(iter)));
        }

        assert!(
            poll_fds[0].revents & libc::POLLOUT != 0,
            "unknown returned events ({revents:02b})",
            revents = poll_fds[0].revents
        );

        // NOTE: We can read, and we consider mutex accesses to be more or less atomic,
        //       as we rely on the pipes behaving correctly.
        let mut buffer = self.channel.buffer.lock();

        let length: usize = {
            let old = buffer.len();
            buffer.extend(iter);
            buffer.len() - old
        };

        // NOTE: We need to close the mutex before telling the receiver, just for sanity.
        drop(buffer);

        // SAFETY: This is just a write syscall.
        let result = unsafe {
            libc::write(
                self.as_raw_fd(),
                (&raw const length).cast(),
                size_of::<usize>(),
            )
        };

        match result {
            -1 => {
                hint::cold_path();

                match Errno::last() {
                    Errno(libc::EPIPE) => Err(SendError::Closed(None)),
                    errno => Err(SendError::Other(errno.into(), None)),
                }
            }
            _ => Ok(()),
        }
    }
}

#[unsafe(no_mangle)]
pub fn lol(
    a: &mut Sender<String>,
    b: String,
) -> Result<(), SendError<String>> {
    a.send_many_timeout([b], None)
        .map_err(|err| err.map(|[s]| s))
}

impl<T> AsFd for Sender<T> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: We own the sender.
        match unsafe { self.channel.borrow_sender() } {
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
        // SAFETY: We own the sender.
        drop(unsafe { self.channel.take_sender() });
    }
}

#[non_exhaustive]
pub enum SendError<T> {
    /// The channel is closed.
    Closed(Option<T>),
    /// We timed out while using the channel.
    TimedOut(Option<T>),
    /// Some other io error occurred.
    Other(io::Error, Option<T>),
}

impl<T> SendError<T> {
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
            SendError::Other(error, value) => SendError::Other(error, value.map(f)),
        }
    }

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
            SendError::Other(error, value) => SendError::Other(error, value.and_then(f)),
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
        self.channel.has_sender()
    }
}

impl<T> AsFd for Receiver<T> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: We own the receiver.
        match unsafe { self.channel.borrow_receiver() } {
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
        // SAFETY: We own the receiver.
        drop(unsafe { self.channel.take_receiver() })
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
                Err(io::Error::last_os_error())
            }
            _ => match [sender, receiver] {
                // NOTE: Just in case libc is fucked.
                [-1, _] | [_, -1] => {
                    hint::cold_path();
                    Err(io::Error::from_raw_os_error(libc::EBADF))
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
        sender: AtomicI32::new(sender.into_raw_fd()),
        receiver: AtomicI32::new(receiver.into_raw_fd()),
    });

    let sender = Sender {
        channel: channel.clone(),
    };
    let receiver = Receiver { channel };

    Ok((sender, receiver))
}
