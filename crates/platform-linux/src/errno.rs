//! Tools for handling Linux error numbers.

use core::{
    borrow::{Borrow, BorrowMut},
    cell::Cell,
    cmp, error,
    ffi::{CStr, c_int},
    fmt, hash,
    mem::MaybeUninit,
    slice,
};

use gulf_core::hint;
#[cfg(feature = "std")]
use gulf_core::ty::AssertSame;

#[cfg(feature = "std")]
use std::io::{Error, ErrorKind};

/// A type alias for raw os errors. This is currently just [`c_int`], which should be fine.
pub type RawOsError = c_int;

#[cfg(feature = "std")]
const _: () = {
    let func = Error::from_raw_os_error as fn(_) -> Error;

    #[allow(clippy::type_complexity)]
    let _: AssertSame<fn(RawOsError) -> Error, _> = AssertSame::of_val(None, Some(&func));
};

/// A type for Linux error numbers.
///
/// # Safety
///
/// It is a transparent wrapper around [`RawOsError`], and it is always sound to reinterpret
/// one as the other. This relation is bidirectional.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct Errno(pub RawOsError);

impl Errno {
    /// This is currently just a type-safe wrapper around `__errno_location`,
    /// returning a type-safe way to get and mutate this thread's [`Errno`].
    ///
    /// # Safety
    ///
    /// This method is safe as on all supported platforms, `__errno_location`
    /// will return a thread-local, non-null, aligned, aliased mutable pointer.
    ///
    /// Since we know that the returned pointer is valid for the duration of this thread's
    /// existence, and will only ever be accessed from this thread, using [`Cell`] is sufficient
    /// for the semantics we require.
    ///
    /// Additionally we guarantee that [`Errno`] has the same layout and ABI as [`RawOsError`].
    ///
    /// ## Aliasing
    ///
    /// Now a lot of code using `__errno_location` creates short-lived intermediate references,
    /// but realistically this shouldn't cause much of any issue, and we personally consider such usage,
    /// to be unsound anyways.
    ///
    /// It is their mistake to create an intermediate reference to a mutable pointer returned from `libc`,
    /// when the methods on pointers, as well as the functions provided by the [`ptr`](core::ptr) module,
    /// have more obvious semantics in code.
    ///
    /// ## Lifetime
    ///
    /// The above is part of *why* we handle lifetimes here as we do. The
    ///
    /// Our reasoning is, since we expect this to be short-lived as well (the returned reference),
    /// we accept a lifetime `'a`. The returned reference has the lifetime of `'a` rather than
    /// `'static` so that it is only borrowed for as long as it is needed.
    ///
    /// This will be determined by the compiler to be the shortest required lifetime,
    /// and should *hopefully* guard against any issues that may arise from other code
    /// that is using `__errno_location` improperly (according to our standards).
    ///
    /// If you'd prefer to not operate upon our assumptions, use `__errno_location` directly,
    /// or alternatively [`io::Error`](std::io::Error).
    #[inline(always)]
    #[must_use]
    pub fn location<'a>() -> &'a Cell<Errno> {
        // SAFETY: See the method-level docs.
        unsafe {
            libc::__errno_location()
                .cast::<Cell<Errno>>()
                .as_ref_unchecked()
        }
    }

    /// Gets the last os error set, usually set by [`libc`] functions.
    ///
    /// This is equivalent to `Errno::location().get()`.
    #[inline(always)]
    #[must_use]
    pub fn last() -> Errno {
        Errno::location().get()
    }

    /// Write the message for this [`Errno`] to some buffer.
    ///
    /// # Safety
    ///
    /// While this method accepts a slice of uninitialized bytes, we guarantee that we will
    /// *never* deininitalize bytes.
    ///
    /// We zero out the entire buffer, always.
    pub fn write_message<'a>(
        self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<&'a CStr, Errno> {
        let buffer = {
            buffer.fill(MaybeUninit::new(0x00));

            // SAFETY: We just filled the buffer with zeroes.
            unsafe { buffer.assume_init_mut() }
        };

        // SAFETY: We kinda expect `libc` to not cause UB.
        let result = unsafe {
            libc::strerror_r(
                self.0,
                (&raw mut *buffer).cast(),
                (&raw const *buffer).len(),
            )
        };

        match result {
            0 if hint::ub_checks() => match CStr::from_bytes_until_nul(buffer) {
                Ok(string) => Ok(string),
                Err(..) => panic!("undefined behavior: `strerror_r` overwrote the NUL-terminator"),
            },
            // SAFETY: We don't care about UB checks.
            0 => Ok(unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }),
            error @ 1.. => Err(Errno(error)),
            ..0 => Err(Errno::last()),
        }
    }

    /// Get the [`ErrorKind`] for this [`Errno`].
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub fn error_kind(self) -> ErrorKind {
        use core::mem::ManuallyDrop;

        let error = ManuallyDrop::new(Error::from(self));

        error.kind()
    }

    #[inline(always)]
    fn format_message<B, S, F, R>(
        self,
        mut buffer: B,
        mut scratch_buffer: S,
        format: F,
    ) -> Result<R, Errno>
    where
        B: WithBuffer,
        S: WithBuffer,
        F: FnOnce(&str) -> R,
    {
        buffer.with_buffer(|buffer| {
            let message_len = self.write_message(&mut *buffer)?.count_bytes();

            // SAFETY: The above `write_string` zeroed out the buffer.
            let buffer = unsafe { buffer.assume_init_mut() };

            // SAFETY: We know that `message_len` is always in bounds.
            let message = unsafe { buffer.get_unchecked(..message_len) };

            let mut chunks = message.utf8_chunks();

            let chunk = match chunks.next() {
                Some(chunk) if chunk.invalid().is_empty() => return Ok(format(chunk.valid())),
                Some(chunk) => chunk,
                None => return Ok(format("")),
            };

            scratch_buffer
                .with_buffer(|scratch| -> Option<_> {
                    let mut index = 0_usize;

                    for chunk in Some(chunk).into_iter().chain(chunks) {
                        scratch
                            .get_mut(index..index + chunk.valid().len())?
                            .write_copy_of_slice(chunk.valid().as_bytes());

                        index += chunk.valid().len();

                        if chunk.invalid().is_empty() {
                            continue;
                        }

                        let replacement = "\u{FFFD}";
                        scratch
                            .get_mut(index..index + replacement.len())?
                            .write_copy_of_slice(replacement.as_bytes());

                        index += replacement.len();
                    }

                    let error_string = {
                        // SAFETY: We've initialized `index` bytes.
                        let bytes = unsafe { scratch.get_unchecked(..index).assume_init_ref() };

                        // SAFETY: We know the initialized bytes are valid UTF-8.
                        unsafe { str::from_utf8_unchecked(bytes) }
                    };

                    Some(format(error_string))
                })
                .ok_or(Errno(libc::ERANGE))
        })
    }
}

impl fmt::Display for Errno {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.format_message(Stack::<512>, Stack::<512>, |detail| {
            write!(f, "{detail} (os error {code})", code = self.0)
        })
        .map_err(|error| {
            log::error!(
                "failed to display os error (code {code} -> status {status})",
                code = self.0,
                status = error.0,
            );

            fmt::Error
        })
        .and_then(|format_result| format_result)
    }
}

impl fmt::Debug for Errno {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("Errno")
            .field("code", &self.0)
            .field(
                "message",
                &fmt::from_fn(|f| {
                    self.format_message(Stack::<512>, Stack::<512>, |detail| {
                        fmt::Debug::fmt(detail, f)
                    })
                    .map_err(|error| {
                        log::error!(
                            "failed to display os error (code {code} -> status {status})",
                            code = self.0,
                            status = error.0,
                        );

                        fmt::Error
                    })
                    .and_then(|format_result| format_result)
                }),
            )
            .finish_non_exhaustive()
    }
}

trait WithBuffer {
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R;
}

impl<const N: usize> WithBuffer for [MaybeUninit<u8>; N] {
    #[inline(always)]
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        f(self)
    }
}

impl WithBuffer for [MaybeUninit<u8>] {
    #[inline(always)]
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        f(self)
    }
}

impl<T> WithBuffer for MaybeUninit<T> {
    #[inline(always)]
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        // SAFETY: We're safe to reinterpret uninit bytes as uninit bytes.
        f(unsafe { slice::from_raw_parts_mut((&raw mut *self).cast(), size_of::<T>()) })
    }
}

struct Stack<const N: usize>;

impl<const N: usize> Stack<N> {
    #[inline(always)]
    #[track_caller]
    fn inlined<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        let mut buffer = [MaybeUninit::uninit(); N];

        f(&mut buffer)
    }
    #[track_caller]
    fn not_inlined<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        self.inlined(f)
    }
}

impl<const N: usize> WithBuffer for Stack<N> {
    #[inline(always)]
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        const THRESHOLD: usize = 512;

        if N > THRESHOLD {
            self.not_inlined(f)
        } else {
            self.inlined(f)
        }
    }
}

impl<B> WithBuffer for &mut B
where
    B: WithBuffer + ?Sized,
{
    #[inline(always)]
    #[track_caller]
    fn with_buffer<F, R>(
        &mut self,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> R,
    {
        B::with_buffer(self, f)
    }
}

impl hash::Hash for Errno {
    #[inline(always)]
    fn hash<H>(
        &self,
        state: &mut H,
    ) where
        H: hash::Hasher,
    {
        self.0.hash(state);
    }

    #[inline(always)]
    fn hash_slice<H>(
        data: &[Errno],
        state: &mut H,
    ) where
        H: hash::Hasher,
    {
        // SAFETY: `Errno`s are just `RawOsError`s... This is fine.
        RawOsError::hash_slice(
            unsafe { (&raw const *data as *const [RawOsError]).as_ref_unchecked() },
            state,
        )
    }
}

impl From<RawOsError> for Errno {
    #[inline(always)]
    fn from(errno: RawOsError) -> Self {
        Errno(errno)
    }
}

impl From<Errno> for RawOsError {
    #[inline(always)]
    fn from(Errno(errno): Errno) -> Self {
        errno
    }
}

impl PartialEq<RawOsError> for Errno {
    #[inline(always)]
    fn eq(
        &self,
        other: &RawOsError,
    ) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Errno> for RawOsError {
    #[inline(always)]
    fn eq(
        &self,
        other: &Errno,
    ) -> bool {
        *self == other.0
    }
}

impl PartialOrd<RawOsError> for Errno {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &RawOsError,
    ) -> Option<cmp::Ordering> {
        Some(self.0.cmp(other))
    }
}

impl PartialOrd<Errno> for RawOsError {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Errno,
    ) -> Option<cmp::Ordering> {
        Some(self.cmp(&other.0))
    }
}

impl Borrow<RawOsError> for Errno {
    #[inline(always)]
    fn borrow(&self) -> &RawOsError {
        &self.0
    }
}

impl Borrow<Errno> for RawOsError {
    #[inline(always)]
    fn borrow(&self) -> &Errno {
        // SAFETY: `Errno` is transparent over `RawOsError`, and imposes no additional invariants.
        unsafe { (&raw const *self).cast::<Errno>().as_ref_unchecked() }
    }
}

impl BorrowMut<RawOsError> for Errno {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut RawOsError {
        &mut self.0
    }
}

impl BorrowMut<Errno> for RawOsError {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Errno {
        // SAFETY: `Errno` is transparent over `RawOsError` and imposes no additional invariants.
        unsafe { (&raw mut *self).cast::<Errno>().as_mut_unchecked() }
    }
}

impl AsRef<RawOsError> for Errno {
    #[inline(always)]
    fn as_ref(&self) -> &RawOsError {
        self.borrow()
    }
}

impl AsRef<Errno> for RawOsError {
    #[inline(always)]
    fn as_ref(&self) -> &Errno {
        self.borrow()
    }
}

impl AsMut<RawOsError> for Errno {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut RawOsError {
        self.borrow_mut()
    }
}

impl AsMut<Errno> for RawOsError {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Errno {
        self.borrow_mut()
    }
}

impl AsRef<Errno> for Errno {
    #[inline(always)]
    fn as_ref(&self) -> &Errno {
        self
    }
}

impl AsMut<Errno> for Errno {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Errno {
        self
    }
}

impl error::Error for Errno {}

#[cfg(feature = "std")]
impl From<Errno> for Error {
    #[inline(always)]
    #[track_caller]
    fn from(Errno(errno): Errno) -> Self {
        Error::from_raw_os_error(errno)
    }
}

#[cfg(feature = "std")]
impl TryFrom<Error> for Errno {
    type Error = Error;

    #[inline(always)]
    fn try_from(error: Error) -> Result<Self, Self::Error> {
        use core::mem::ManuallyDrop;

        let error = ManuallyDrop::new(error);

        match error.raw_os_error() {
            Some(errno) => Ok(Errno(errno)),
            None => Err(ManuallyDrop::into_inner(error)),
        }
    }
}

#[cfg(feature = "std")]
impl PartialEq<Error> for Errno {
    #[inline(always)]
    fn eq(
        &self,
        other: &Error,
    ) -> bool {
        match other.raw_os_error() {
            Some(other) => self.0 == other,
            None => false,
        }
    }
}

#[cfg(feature = "std")]
impl PartialEq<Errno> for Error {
    #[inline(always)]
    fn eq(
        &self,
        other: &Errno,
    ) -> bool {
        match self.raw_os_error() {
            Some(this) => this == other.0,
            None => false,
        }
    }
}

#[cfg(feature = "std")]
impl PartialOrd<Error> for Errno {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Error,
    ) -> Option<cmp::Ordering> {
        match other.raw_os_error() {
            Some(other) => Some(self.0.cmp(&other)),
            None => None,
        }
    }
}

#[cfg(feature = "std")]
impl PartialOrd<Errno> for Error {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Errno,
    ) -> Option<cmp::Ordering> {
        match self.raw_os_error() {
            Some(this) => Some(this.cmp(&other.0)),
            None => None,
        }
    }
}
