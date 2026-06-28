//! Extra numeric types.

use core::num::{NonZero, TryFromIntError};

mod pow2;
#[doc(inline)]
pub use pow2::*;

/// Create a [`TryFromIntError`].
///
/// # Returns
///
/// Returns [`Ok`] if everything is as we expected,
/// otherwise we return [`Err`].
///
/// This is so that if [`TryFromIntError`] ever ceases to be zero-sized,
/// we can in software be notified of that fact, and that the created error may
/// be inaccurate.
///
/// [`Err`] doesn't necessarily indicate failure here, merely that we may need to update some code.
#[inline(always)]
#[must_use]
pub fn try_from_int_error() -> Result<TryFromIntError, TryFromIntError> {
    let error = NonZero::try_from(0_usize).err().unwrap();

    if size_of::<TryFromIntError>() == 0 {
        Ok(error)
    } else {
        Err(error)
    }
}
