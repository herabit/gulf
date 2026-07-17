//! Atomic file descriptors.

use core::fmt;
use std::{
    hint, mem,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    sync::atomic::{AtomicI16, AtomicI32, AtomicI64, Ordering},
};

use crate::errno::Errno;

trait SelectRepr<const BITS: u32> {
    type Repr;
}

impl SelectRepr<16> for AtomicFd {
    type Repr = AtomicI16;
}

impl SelectRepr<32> for AtomicFd {
    type Repr = AtomicI32;
}

impl SelectRepr<64> for AtomicFd {
    type Repr = AtomicI64;
}

type AtomicRepr = <AtomicFd as SelectRepr<{ RawFd::BITS }>>::Repr;

const _: () = {
    assert!(size_of::<AtomicFd>() == size_of::<RawFd>(), "we, fucked up");
    assert!(
        size_of::<Option<OwnedFd>>() == size_of::<RawFd>(),
        "this shouldn't happen, they're ABI compatible",
    );

    // Now here's where it gets SPOOOOOOKY. We're going to reinterpret the `None` case's bits,
    // and check that they're `-1`. If they aren't... Well then we're fucked, but MIRI should
    // hopefully catch us.
    let none_bits: RawFd = unsafe { mem::transmute::<Option<OwnedFd>, RawFd>(None) };

    // Here's the magic. We need this to make it sound.
    assert!(none_bits == -1, "FUCKKKKKKKKK");

    // We're also going to check for borrowed file descriptors.
    let none_bits: RawFd = unsafe { mem::transmute::<Option<BorrowedFd<'_>>, RawFd>(None) };

    // More magic!
    assert!(none_bits == -1, "FUCK AGAIN");
};

/// An atomic file descriptor. This is "equivalent" to an atomic `Option<OwnedFd>`.
#[repr(transparent)]
pub struct AtomicFd {
    fd: AtomicRepr,
}

impl AtomicFd {
    /// Creates a new atomic file descriptor.
    #[inline(always)]
    #[must_use]
    pub const fn new(fd: Option<OwnedFd>) -> AtomicFd {
        // SAFETY: We know this is sound, `None` is `-1`, and `OwnedFd` is just a fancy `RawFd`.
        unsafe { mem::transmute(fd) }
    }

    /// Get a reference to the underlying `Option<OwnedFd>`.
    #[inline(always)]
    #[must_use]
    pub const fn get_mut(&mut self) -> &mut Option<OwnedFd> {
        // SAFETY: We have mutable access to the underlying file descriptor, and we ensure
        //         the memory layout for `Option<OwnedFd>` is what we expect.
        unsafe {
            (&raw mut *self)
                .cast::<Option<OwnedFd>>()
                .as_mut_unchecked()
        }
    }

    /// Get a raw pointer to the underlying `Option<OwnedFd>`.
    #[inline(always)]
    #[must_use]
    pub const fn as_ptr(&self) -> *mut Option<OwnedFd> {
        self.fd.as_ptr().cast()
    }

    /// Consumes this file descriptor.
    #[inline(always)]
    #[must_use]
    pub const fn into_inner(self) -> Option<OwnedFd> {
        // SAFETY: We own this value and we already know that the memory layout is consistent.
        unsafe { mem::transmute(self) }
    }

    /// Swap this file descriptor using atomics.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn swap(
        &self,
        new: Option<OwnedFd>,
        order: Ordering,
    ) -> Option<OwnedFd> {
        // SAFETY: We know the memory layouts match and invariants are upheld.
        let new: RawFd = unsafe { mem::transmute(new) };
        let old = self.fd.swap(new, order);

        // SAFETY: We know the memory layouts match and invariants are upheld.
        unsafe { mem::transmute(old) }
    }

    /// Returns whether this atomic currently contains a file descriptor.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn is_some(
        &self,
        order: Ordering,
    ) -> bool {
        self.fd.load(order) != -1
    }

    /// Returns whether this atomic currently does not contain a file descriptor.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn is_none(
        &self,
        order: Ordering,
    ) -> bool {
        !self.is_some(order)
    }

    /// Load the raw file descriptor with the specified ordering.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub fn load_raw(
        &self,
        order: Ordering,
    ) -> RawFd {
        self.fd.load(order)
    }

    /// Borrow the file descriptor, loading with the specified ordering.
    ///
    /// # Safety
    ///
    /// The caller needs to ensure that the underlying `fd` is not closed
    /// for the lifetime `'a`.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub unsafe fn load_borrowed<'a>(
        &self,
        order: Ordering,
    ) -> Option<BorrowedFd<'a>> {
        // SAFETY: We know that the layouts are equivalent, and that the invariants are met for
        //         `fd` being a valid file descriptor.
        //
        //         Additionally the caller ensures it is safe to borrow it for `'a`.
        unsafe { mem::transmute(self.load_raw(order)) }
    }
}

impl Drop for AtomicFd {
    #[inline(always)]
    #[track_caller]
    fn drop(&mut self) {
        self.get_mut().take();
    }
}

impl From<OwnedFd> for AtomicFd {
    #[inline(always)]
    #[track_caller]
    fn from(fd: OwnedFd) -> Self {
        AtomicFd::new(Some(fd))
    }
}

impl From<Option<OwnedFd>> for AtomicFd {
    #[inline(always)]
    #[track_caller]
    fn from(fd: Option<OwnedFd>) -> Self {
        AtomicFd::new(fd)
    }
}

impl From<AtomicFd> for Option<OwnedFd> {
    #[inline(always)]
    fn from(fd: AtomicFd) -> Self {
        fd.into_inner()
    }
}

impl TryFrom<AtomicFd> for OwnedFd {
    type Error = Errno;

    #[inline(always)]
    fn try_from(fd: AtomicFd) -> Result<Self, Self::Error> {
        match fd.into_inner() {
            Some(fd) => Ok(fd),
            None => Err(Errno(libc::EBADF)),
        }
    }
}

impl TryFrom<BorrowedFd<'_>> for AtomicFd {
    type Error = Errno;

    /// This will clone the file descriptor if possible.
    #[inline(always)]
    fn try_from(fd: BorrowedFd<'_>) -> Result<Self, Self::Error> {
        // SAFETY: We're just calling libc with a valid file descriptor, ensuring that
        //         the new file descriptor does not clobber those used by stdio.
        let result = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };

        match result {
            -1 => {
                hint::cold_path();
                Err(Errno::last())
            }
            // SAFETY: We did not get an error, thus `fd` is a valid file descriptor.
            fd => Ok(unsafe { AtomicFd::from_raw_fd(fd) }),
        }
    }
}

impl AsMut<Option<OwnedFd>> for AtomicFd {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Option<OwnedFd> {
        self.get_mut()
    }
}

impl Default for AtomicFd {
    #[inline(always)]
    fn default() -> Self {
        AtomicFd::new(None)
    }
}

impl FromRawFd for AtomicFd {
    #[inline(always)]
    #[track_caller]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        AtomicFd::new(match fd {
            -1 => None,
            // SAFETY: The caller ensures this is okay.
            fd => Some(unsafe { OwnedFd::from_raw_fd(fd) }),
        })
    }
}

impl IntoRawFd for AtomicFd {
    #[inline(always)]
    #[track_caller]
    fn into_raw_fd(self) -> RawFd {
        match self.into_inner() {
            Some(fd) => fd.into_raw_fd(),
            None => -1,
        }
    }
}

impl AsRawFd for AtomicFd {
    /// # Safety
    ///
    /// The results of this function may change between calls. Use at your own discretion.
    #[inline(always)]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for AtomicFd {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("AtomicFd")
            .field(
                "fd",
                &match self.as_raw_fd() {
                    -1 => None,
                    fd => Some(fd),
                },
            )
            .finish()
    }
}

// impl From<Option<OwnedFd>>
