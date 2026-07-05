//! Utilities for handling pointer metadata.

use core::{
    error::{self, Error as _},
    ffi::CStr,
    fmt, hash, mem,
    num::{NonZero, TryFromIntError},
};

/// A trait for types that are valid pointer metadata.
///
/// # Safety
///
/// Types that are valid pointer metadata must ***also contain absolutely no interior
/// mutability***, ignoring through indirection. In other words, they need
/// to be [`Freeze`](::core::marker::Freeze).
///
/// We do not, however, add this as a required bound due to it being unstable. As such,
/// if the definition of [`Freeze`](::core::marker::Freeze) changes, so too does the requirements
/// of this trait, despite it not currently being a required bound.
///
/// Additionally, there may be further required invariants. As such, implementing this trait
/// yourself is not reccomended.
///
/// Currently the supported types are [`usize`] and [`()`].
pub unsafe trait Metadata:
    Copy + Send + Sync + Ord + Unpin + hash::Hash + fmt::Debug
{
}

unsafe impl Metadata for () {}
unsafe impl Metadata for usize {}

trait Select<const VALUE: usize> {
    type Output;
}

// When `*const CStr` is ptr-sized, then we don't want to store anything.
impl Select<{ size_of::<usize>() }> for CStr {
    type Output = ();
}

// When `*const CStr` is the size of two pointers, then we want to store the length.
impl Select<{ size_of::<[usize; 2]>() }> for CStr {
    type Output = usize;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
#[repr(transparent)]
pub struct CStrMetadata {
    // We want to handle when CStr eventually ceases to be fat.
    repr: <CStr as Select<{ size_of::<*const CStr>() }>>::Output,
}

impl CStrMetadata {
    /// Create a new [`CStrMetadata`] from a [`usize`].
    #[inline(always)]
    #[must_use]
    pub const fn new(length: usize) -> CStrMetadata {
        // SAFETY: If CStrs are fat pointees, then this will just be `CStrMetadata { repr: length }`,
        //         but if it ever ceases to be fat, then we're copying zero bytes from a usize,
        //         thus also making this valid due to the `repr(transparent)`.
        unsafe { mem::transmute_copy(&length) }
    }

    /// Attempt to create a [`CStrMetadata`] from a [`()`].
    #[inline(always)]
    #[must_use]
    pub const fn from_unit(unit: ()) -> Option<CStrMetadata> {
        if size_of::<CStrMetadata>() == size_of::<()>() {
            // SAFETY: We're transparent over `()`.
            Some(unsafe { mem::transmute_copy(&unit) })
        } else {
            None
        }
    }

    /// Returns the length stored within this metadata, if it exists.
    #[inline(always)]
    #[must_use]
    pub const fn get(self) -> Option<usize> {
        if size_of::<CStrMetadata>() == size_of::<usize>() {
            // SAFETY: We know the metadata stores a length.
            Some(unsafe { mem::transmute_copy(&self.repr) })
        } else {
            None
        }
    }
}

impl fmt::Debug for CStrMetadata {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self.get() {
            Some(length) => f
                .debug_struct("CStrMetadata")
                .field("length", &length)
                .finish_non_exhaustive(),
            None => f.debug_struct("CStrMetadata").finish_non_exhaustive(),
        }
    }
}

impl From<usize> for CStrMetadata {
    #[inline(always)]
    fn from(length: usize) -> Self {
        CStrMetadata::new(length)
    }
}

impl From<NonZero<usize>> for CStrMetadata {
    #[inline(always)]
    fn from(length: NonZero<usize>) -> Self {
        length.get().into()
    }
}

impl TryFrom<CStrMetadata> for usize {
    type Error = CStrMetadataError;

    #[inline(always)]
    fn try_from(metadata: CStrMetadata) -> Result<Self, Self::Error> {
        metadata.get().ok_or(CStrMetadataError::IsThin)
    }
}

impl TryFrom<CStrMetadata> for NonZero<usize> {
    type Error = CStrMetadataError;

    #[inline(always)]
    fn try_from(metadata: CStrMetadata) -> Result<Self, Self::Error> {
        usize::try_from(metadata).and_then(|length| NonZero::try_from(length).map_err(Into::into))
    }
}

impl TryFrom<()> for CStrMetadata {
    type Error = CStrMetadataError;

    #[inline(always)]
    fn try_from(unit: ()) -> Result<Self, Self::Error> {
        CStrMetadata::from_unit(unit).ok_or(CStrMetadataError::NotThin)
    }
}

unsafe impl Metadata for CStrMetadata {}

/// Error that can occur when handling [`CStrMetadata`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CStrMetadataError {
    /// [`CStr`] is a thin pointee in a context where it shouldn't be.
    IsThin,

    /// [`CStr`] is not a thin pointee in a context where it should be.
    NotThin,

    /// There was an error when converting to an integer.
    TryFromInt(TryFromIntError),
}

impl From<TryFromIntError> for CStrMetadataError {
    #[inline(always)]
    fn from(error: TryFromIntError) -> Self {
        CStrMetadataError::TryFromInt(error)
    }
}

impl fmt::Display for CStrMetadataError {
    #[allow(deprecated)]
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl error::Error for CStrMetadataError {
    #[inline(always)]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::TryFromInt(error) => Some(error),
            Self::IsThin | Self::NotThin => None,
        }
    }

    #[inline(always)]
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match self {
            Self::IsThin => "`CStr` is a thin pointer in a context where it shouldn't be",
            Self::NotThin => "`CStr` is not a thin pointer in a context when it should be",
            Self::TryFromInt(error) => error.description(),
        }
    }
}

pub unsafe trait CastMetadata<Dest: Metadata>: Metadata {}

unsafe impl<M> CastMetadata<M> for M where M: Metadata {}

unsafe impl CastMetadata<()> for usize {}
unsafe impl CastMetadata<()> for CStrMetadata {}

unsafe impl CastMetadata<usize> for CStrMetadata {}
unsafe impl CastMetadata<CStrMetadata> for usize {}

#[inline(always)]
#[must_use]
pub const fn cast_metadata<Src, Dest>(src: Src) -> Dest
where
    Src: CastMetadata<Dest>,
    Dest: Metadata,
{
    // SAFETY: This will panic if `Dest > Src`.
    unsafe { mem::transmute_copy(&src) }
}
