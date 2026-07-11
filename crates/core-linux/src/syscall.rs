//! Tools for directly doing syscalls (using inline assembly where possible).

use crate::errno::Errno;

use core::{
    convert,
    error::{self, Error as _},
    fmt,
    num::{NonZero, TryFromIntError},
    ptr::{self, NonNull},
};

#[cfg(feature = "std")]
use std::io;

cfg_select! {
    all(target_arch = "x86_64", target_pointer_width = "64") => {
        pub mod x86_64;
    },
    all(target_arch = "x86_64", target_pointer_width = "32") => {
        ::core::compiler_error!("x32 is a degenerate ABI that i refuse to support");
    },
    target_arch = "x86" => {
        pub mod x86;
    },
    _ => {
        ::core::compiler_error!("unsupported platform");
    }
}

/// The type that is passed along to syscalls.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct SysWord(*const ());

impl SysWord {
    /// Create a new [`SysWord`] from a mutable pointer.
    ///
    /// # Safety
    ///
    /// This preserves provenance.
    #[inline(always)]
    #[must_use]
    pub const fn from_mut_ptr<T>(ptr: *mut T) -> SysWord
    where
        T: Sized,
    {
        SysWord(ptr.cast::<()>().cast_const())
    }

    /// Get this [`SysWord`] as a mutable pointer.
    ///
    /// # Safety
    ///
    /// If this [`SysWord`] was created in a manner that preserves
    /// provenance, the returned pointer will have that provenance.
    ///
    /// Do note that if this is the return value of a syscall, it will have
    /// *no associated provenance*. So if this is a pointer that is *derived from the input arguments*,
    /// it is up to you to give it the correct provenance. See [`SysWord::with_provenance_of`].
    #[inline(always)]
    #[must_use]
    pub const fn as_mut_ptr<T>(self) -> *mut T
    where
        T: Sized,
    {
        self.0.cast::<T>().cast_mut()
    }

    /// Create a new [`SysWord`] from a pointer.
    ///
    /// # Safety
    ///
    /// This preserves provenance.
    #[inline(always)]
    #[must_use]
    pub const fn from_ptr<T>(ptr: *const T) -> SysWord
    where
        T: Sized,
    {
        SysWord(ptr.cast::<()>())
    }

    /// Get this [`SysWord`] as a pointer.
    ///
    /// # Safety
    ///
    /// If this [`SysWord`] was created in a manner that preserves
    /// provenance, the returned pointer will have that provenance.
    #[inline(always)]
    #[must_use]
    pub const fn as_ptr<T>(self) -> *const T
    where
        T: Sized,
    {
        self.0.cast::<T>()
    }

    /// Create a new [`SysWord`] from a [`NonNull`] pointer.
    ///
    /// # Safety
    ///
    /// This preserves provenance.
    #[inline(always)]
    #[must_use]
    pub const fn from_nonnull<T>(ptr: NonNull<T>) -> SysWord
    where
        T: Sized,
    {
        SysWord::from_mut_ptr(ptr.as_ptr())
    }

    /// Get this [`SysWord`] as a [`NonNull`] pointer.
    ///
    /// # Safety
    ///
    /// If this [`SysWord`] was created in a manner that preserves
    /// provenance, the returned pointer will have that provenance.
    #[inline(always)]
    #[must_use]
    pub const fn as_nonnull<T>(self) -> Option<NonNull<T>>
    where
        T: Sized,
    {
        NonNull::new(self.as_mut_ptr())
    }

    /// Create a new [`SysWord`] from a reference.
    ///
    /// # Safety
    ///
    /// This preserves provenance, but does not create an intermediate reference.
    #[inline(always)]
    #[must_use]
    pub const fn from_ref<T>(r: &T) -> SysWord
    where
        T: Sized,
    {
        SysWord::from_ptr(&raw const *r)
    }

    /// Create a new [`SysWord`] from a mutable reference.
    ///
    /// # Safety
    ///
    /// This preserves provenance, but does not create an intermediate reference.
    #[inline(always)]
    #[must_use]
    pub const fn from_mut<T>(r: &mut T) -> SysWord
    where
        T: Sized,
    {
        SysWord::from_mut_ptr(&raw mut *r)
    }
    /// Create a new [`SysWord`] from a [`usize`].
    ///
    /// # Safety
    ///
    /// This does not preserve provenance if `x` has any associated provenance. If possible,
    /// create the [`SysWord`] from a pointer with the proper provenance.
    #[inline(always)]
    #[must_use]
    pub const fn from_usize(x: usize) -> SysWord {
        SysWord(ptr::without_provenance_mut(x))
    }

    /// Get this [`SysWord`] as a [`usize`].
    ///
    /// # Safety
    ///
    /// This does not preserve provenance even if this [`SysWord`] has provenance.
    ///
    /// If you need to preserve provenance, maybe opt for `self.as_ptr::<()>().expose_provenance()`.
    #[inline(always)]
    #[must_use]
    pub fn as_usize(self) -> usize {
        self.0.addr()
    }

    /// Create a new [`SysWord`] from an [`isize`].
    ///
    /// # Safety
    ///
    /// This does not preserve provenance if `x` has any associated provenance. If possible,
    /// create the [`SysWord`] from a pointer with the proper provenance.
    #[inline(always)]
    #[must_use]
    pub const fn from_isize(x: isize) -> SysWord {
        // SAFETY: Same bitsize, this loses no data.
        SysWord::from_usize(x.cast_unsigned())
    }

    /// Get this [`SysWord`] as an [`isize`].
    ///
    /// # Safety
    ///
    /// This does not preserve provenance even if this [`SysWord`] has provenance.
    ///
    /// If you need to preserve provenance, maybe opt for `self.as_ptr::<()>().expose_provenance().cast_signed()`.
    #[inline(always)]
    #[must_use]
    pub fn as_isize(self) -> isize {
        self.as_usize().cast_signed()
    }

    /// Create a [`SysWord`] from a [`u32`].
    ///
    /// This is a convenience function for `SysWord::from_usize(x as usize)`, which will never
    /// truncate as we do not support 16-bit platforms.
    ///
    /// See [`SysWord::from_usize`] for more details.
    #[inline(always)]
    #[must_use]
    pub const fn from_u32(x: u32) -> SysWord {
        const {
            assert!(
                u32::BITS <= usize::BITS,
                "we don't support 16-bit platforms"
            )
        };

        SysWord::from_usize(x as usize)
    }

    /// Get this [`SysWord`] as a [`u32`].
    ///
    /// This is a convenience function for `self.as_usize().try_into().ok()`.
    ///
    /// See [`SysWord::as_usize`] for more details.
    #[inline(always)]
    #[must_use]
    pub fn as_u32(self) -> Option<u32> {
        self.as_usize().try_into().ok()
    }

    /// Create a [`SysWord`] from an [`i32`].
    ///
    /// This is a convenience function for `SysWord::from_isize(x as isize)`, which will
    /// never truncate as we do not support 16-bit platforms.
    ///
    /// Note, this is ***different*** from `SysWord::from_u32(x as u32)`, as this function
    /// will perform a sign extension on platforms that are not 32-bit.
    ///
    /// See [`SysWord::from_isize`] for more details.
    #[inline(always)]
    #[must_use]
    pub const fn from_i32(x: i32) -> SysWord {
        const {
            assert!(
                i32::BITS <= isize::BITS,
                "we don't support 16-bit platforms"
            )
        };

        SysWord::from_isize(x as isize)
    }

    /// Get this [`SysWord`] as an [`i32`].
    ///
    /// This is a convenience function for `self.as_isize().try_into()`.
    ///
    /// See [`SysWord::as_isize`] for more details.
    #[inline(always)]
    #[must_use]
    pub fn as_i32(self) -> Option<i32> {
        self.as_isize().try_into().ok()
    }

    /// Create a [`SysWord`] from a [`u64`].
    ///
    /// This is a convenience function for `x.try_into().ok().map(SysWord::from_usize)`.
    ///
    /// See [`SysWord::from_usize`] for more details.
    #[inline(always)]
    #[must_use]
    pub const fn from_u64(x: u64) -> Option<SysWord> {
        if usize::BITS >= u64::BITS || x <= (usize::MAX as u64) {
            Some(SysWord::from_usize(x as usize))
        } else {
            None
        }
    }

    /// Get this [`SysWord`] as a [`u64`].
    ///
    /// This is a convenience function for `self.as_usize().try_into().ok()`.
    ///
    /// See [`SysWord::as_usize`] for more details.
    #[inline(always)]
    #[must_use]
    pub fn as_u64(self) -> Option<u64> {
        self.as_usize().try_into().ok()
    }

    /// Create a [`SysWord`] from an [`i64`].
    ///
    /// This is a convenience function for `x.try_into().ok().map(SysWord::from_isize)`.
    ///
    /// See [`SysWord::from_isize`] for more details.
    #[inline(always)]
    #[must_use]
    pub const fn from_i64(x: i64) -> Option<SysWord> {
        if isize::BITS >= i64::BITS || ((x >= isize::MIN as i64) && (x <= isize::MAX as i64)) {
            Some(SysWord::from_isize(x as isize))
        } else {
            None
        }
    }

    /// Get this [`SysWord`] as an [`i64`].
    ///
    /// This is a convenience function for `self.as_isize().try_into().ok()`.
    ///
    /// See [`SysWord::as_isize`] for more details.
    #[inline(always)]
    #[must_use]
    pub fn as_i64(self) -> Option<i64> {
        self.as_isize().try_into().ok()
    }

    /// Give this [`SysWord`] the provenance of the provided pointer.
    ///
    /// # Safety
    ///
    /// This will disregard the provenance of this [`SysWord`] for that of `ptr`.
    #[inline(always)]
    #[must_use]
    pub fn with_provenance_of<T>(
        self,
        ptr: *const T,
    ) -> SysWord
    where
        T: Sized,
    {
        // NOTE: We don't really care about the provenance of `self`, so losing it is fine.
        SysWord::from_ptr(ptr.with_addr(self.as_usize()))
    }

    /// Returns whether this [`SysWord`], if treated as a pointer, is null.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.as_ptr::<()>().is_null()
    }

    /// If this seems to be a valid [`Errno`], return it as an error, otherwise return `self`.
    #[inline(always)]
    pub fn as_errno(self) -> Result<SysWord, Errno> {
        match Errno::from_syscall(self.as_isize()) {
            Ok(..) => Ok(self),
            Err(errno) => Err(errno),
        }
    }

    /// Same as [`SysWord::as_errno`], but turn it into a [`io::Error`].
    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn as_error(self) -> Result<SysWord, io::Error> {
        self.as_errno().map_err(From::from)
    }
}

impl<T> From<*const T> for SysWord
where
    T: Sized,
{
    #[inline(always)]
    fn from(ptr: *const T) -> Self {
        SysWord::from_ptr(ptr)
    }
}

impl<T> From<SysWord> for *const T
where
    T: Sized,
{
    #[inline(always)]
    fn from(word: SysWord) -> Self {
        word.as_ptr()
    }
}

impl<T> From<*mut T> for SysWord
where
    T: Sized,
{
    #[inline(always)]
    fn from(ptr: *mut T) -> Self {
        SysWord::from_mut_ptr(ptr)
    }
}

impl<T> From<SysWord> for *mut T
where
    T: Sized,
{
    #[inline(always)]
    fn from(word: SysWord) -> Self {
        word.as_mut_ptr()
    }
}

impl<T> From<NonNull<T>> for SysWord
where
    T: Sized,
{
    #[inline(always)]
    fn from(ptr: NonNull<T>) -> Self {
        SysWord::from_nonnull(ptr)
    }
}

impl<T> TryFrom<SysWord> for NonNull<T>
where
    T: Sized,
{
    type Error = NullPtrError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_nonnull().ok_or(NullPtrError(()))
    }
}

impl<T> From<Option<NonNull<T>>> for SysWord
where
    T: Sized,
{
    /// # Safety
    ///
    /// If `ptr` is [`None`], then the provenance is already probably lost
    /// due to how most `Option<NonNull<T>>`s are constructed.
    ///
    /// As such, the returned value on [`None`] will not have provenance.
    #[inline(always)]
    fn from(ptr: Option<NonNull<T>>) -> Self {
        SysWord::from_mut_ptr(match ptr {
            Some(ptr) => ptr.as_ptr(),
            None => ptr::null_mut(),
        })
    }
}

impl<T> From<SysWord> for Option<NonNull<T>>
where
    T: Sized,
{
    /// # Safety
    ///
    /// If this value is null, then the returned [`None`], if reinterpreted as a pointer,
    /// will have no provenance.
    #[inline(always)]
    fn from(word: SysWord) -> Self {
        word.as_nonnull()
    }
}

impl<'a, T> From<&'a T> for SysWord
where
    T: Sized,
{
    #[inline(always)]
    fn from(r: &'a T) -> Self {
        SysWord::from_ref(r)
    }
}

impl<'a, T> From<Option<&'a T>> for SysWord
where
    T: Sized,
{
    /// # Safety
    ///
    /// Upon the [`None`] case, the returned [`SysWord`] will have no provenance.
    #[inline(always)]
    fn from(r: Option<&'a T>) -> Self {
        SysWord::from_ptr(match r {
            Some(r) => &raw const *r,
            None => ptr::null(),
        })
    }
}

impl<'a, T> From<&'a mut T> for SysWord
where
    T: Sized,
{
    #[inline(always)]
    fn from(r: &'a mut T) -> Self {
        SysWord::from_mut(r)
    }
}

impl<'a, T> From<Option<&'a mut T>> for SysWord
where
    T: Sized,
{
    /// # Safety
    ///
    /// Upon the [`None`] case, the returned [`SysWord`] will have no provenance.
    #[inline(always)]
    fn from(r: Option<&'a mut T>) -> Self {
        SysWord::from_mut_ptr(match r {
            Some(r) => &raw mut *r,
            None => ptr::null_mut(),
        })
    }
}

impl From<usize> for SysWord {
    #[inline(always)]
    fn from(x: usize) -> Self {
        SysWord::from_usize(x)
    }
}

impl From<SysWord> for usize {
    #[inline(always)]
    fn from(word: SysWord) -> Self {
        word.as_usize()
    }
}

impl From<NonZero<usize>> for SysWord {
    #[inline(always)]
    fn from(x: NonZero<usize>) -> Self {
        SysWord::from_usize(x.get())
    }
}

impl TryFrom<SysWord> for NonZero<usize> {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_usize().try_into()
    }
}

impl From<isize> for SysWord {
    #[inline(always)]
    fn from(x: isize) -> Self {
        SysWord::from_isize(x)
    }
}

impl From<SysWord> for isize {
    #[inline(always)]
    fn from(word: SysWord) -> Self {
        word.as_isize()
    }
}

impl From<NonZero<isize>> for SysWord {
    #[inline(always)]
    fn from(x: NonZero<isize>) -> Self {
        SysWord::from_isize(x.get())
    }
}

impl TryFrom<SysWord> for NonZero<isize> {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_isize().try_into()
    }
}

impl From<u32> for SysWord {
    #[inline(always)]
    fn from(x: u32) -> Self {
        SysWord::from_u32(x)
    }
}

impl TryFrom<SysWord> for u32 {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_usize().try_into()
    }
}

impl From<i32> for SysWord {
    #[inline(always)]
    fn from(x: i32) -> Self {
        SysWord::from_i32(x)
    }
}

impl TryFrom<SysWord> for i32 {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_isize().try_into()
    }
}

impl TryFrom<u64> for SysWord {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        x.try_into().map(SysWord::from_usize)
    }
}

impl TryFrom<SysWord> for u64 {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_usize().try_into()
    }
}

impl TryFrom<i64> for SysWord {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(x: i64) -> Result<Self, Self::Error> {
        x.try_into().map(SysWord::from_isize)
    }
}

impl TryFrom<SysWord> for i64 {
    type Error = TryFromIntError;

    #[inline(always)]
    fn try_from(word: SysWord) -> Result<Self, Self::Error> {
        word.as_isize().try_into()
    }
}

impl fmt::Display for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        std::fmt::Display::fmt(&self.as_usize(), f)
    }
}

impl fmt::Debug for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Debug::fmt(&self.as_usize(), f)
    }
}

impl fmt::LowerExp for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::LowerExp::fmt(&self.as_usize(), f)
    }
}

impl fmt::UpperExp for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::UpperExp::fmt(&self.as_usize(), f)
    }
}

impl fmt::LowerHex for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::LowerHex::fmt(&self.as_usize(), f)
    }
}

impl fmt::UpperHex for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::UpperHex::fmt(&self.as_usize(), f)
    }
}

impl fmt::Octal for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Octal::fmt(&self.as_usize(), f)
    }
}

impl fmt::Binary for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Binary::fmt(&self.as_usize(), f)
    }
}

impl fmt::Pointer for SysWord {
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr::<()>(), f)
    }
}

/// An error that gets returned if we get a null pointer where we do not expect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct NullPtrError(());

impl fmt::Display for NullPtrError {
    #[inline(always)]
    #[allow(deprecated)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.description().fmt(f)
    }
}

impl error::Error for NullPtrError {
    #[inline(always)]
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "provided pointer was null"
    }
}

impl From<convert::Infallible> for NullPtrError {
    #[inline(always)]
    fn from(never: convert::Infallible) -> Self {
        match never {}
    }
}

impl From<NullPtrError> for Errno {
    #[inline(always)]
    fn from(_: NullPtrError) -> Self {
        Errno(libc::EINVAL)
    }
}

#[cfg(feature = "std")]
impl From<NullPtrError> for io::Error {
    #[inline(always)]
    fn from(_: NullPtrError) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, "provided pointer was null")
    }
}

/// A struct representing the return value of a syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SysResult<const N: usize = 1> {
    /// An error flag on supported platforms that denotes whether
    /// the syscall returned erroneously.
    ///
    /// On systems without support, [`None`] is returned.
    pub error_flag: Option<bool>,

    /// The return values.
    ///
    /// All platforms return `values[0]`, but not all support `values[1]`,
    /// in those cases we just supply a null.
    pub values: [SysWord; N],
}

impl<const N: usize> SysResult<N> {
    /// This creates an empty [`SysResult`].
    #[inline(always)]
    #[must_use]
    pub const fn empty() -> SysResult<N> {
        SysResult {
            error_flag: None,
            values: [SysWord::from_usize(0); _],
        }
    }
}

impl<const N: usize> Default for SysResult<N> {
    #[inline(always)]
    fn default() -> Self {
        SysResult::empty()
    }
}
