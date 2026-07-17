//! Better pipewire channels.

use gulf_core::unreachable_unchecked;
use gulf_platform_linux::{atomic_fd::AtomicFd, errno::Errno};
use parking_lot::Mutex;
use std::{
    cell::Cell,
    collections::VecDeque,
    error::{self, Error as _},
    ffi::c_int,
    fmt, hint,
    io::{self, Error, ErrorKind},
    marker::PhantomData,
    num::NonZero,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd},
    slice,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
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
    /// SAFETY: We need this to ensure that [`Sender`]s are NOT [`Sync`]!!!
    ///
    ///         Reeeee.
    _not_sync: PhantomData<Cell<()>>,
}

impl<T> Sender<T> {
    #[inline(always)]
    #[must_use]
    pub fn has_receiver(&self) -> bool {
        self.channel.receiver.is_some(Ordering::Acquire)
    }

    pub async fn send_many<I>(
        &self,
        items: I,
        timeout: Option<Duration>,
    ) -> Result<(), SendError<I>>
    where
        I: IntoIterator<Item = T>,
    {
        todo!()
    }
}

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
        match Arc::strong_count(&self.channel) {
            // SAFETY: We know for a fact the strong count is never zero while the `Arc` exists.
            0 => unsafe {
                unreachable_unchecked!("we know for a fact this is not zero, as the `Arc` exists")
            },

            // NOTE: We own the channel, we can take ownership of the sender file descriptor.
            1 => drop(self.channel.sender.swap(None, Ordering::Release)),

            // NOTE: If we're one of two referants to the `Arc`, and the other is the receiver, then
            //       we can take the sender file descriptor as there are no other senders. We ensure this
            //       is the case by making `Sender` not `Sync` (using a phantom `Cell`). It is safe to clone a
            //       `Sender` on the same thread that owns that instance, and it is safe to send a `Sender`
            //       to another thread, but it is not safe to clone from another thread due to how we use the
            //       reference count and atomic file descriptors here.
            //
            //       We know that the amount of receivers will never increase as they're not clone. Additionally,
            //       since we know we can't share a sender through a reference with other threads, that means we
            //       can't clone it from another thread. Thus, the reference count here will can only ever decrease
            //       after this check.
            //
            //       With a guarantee of no other senders existing, or having the possibility of existing,
            //       it is sound to drop the file descriptor for the sender.
            2 if self.has_receiver() => drop(self.channel.sender.swap(None, Ordering::Release)),

            _ => {}
        }
    }
}

impl<T> Clone for Sender<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Sender {
            channel: self.channel.clone(),
            _not_sync: PhantomData,
        }
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
        _not_sync: PhantomData,
    };
    let receiver = Receiver { channel };

    Ok((sender, receiver))
}
