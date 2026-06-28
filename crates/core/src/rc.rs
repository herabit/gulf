//! Extended functionality for [`Rc`](::alloc::rc::Rc).

use alloc::rc::{Rc, Weak};
use core::mem::MaybeUninit;

/// Trait for types that are reference counted.
pub unsafe trait RefCount {
    /// Get the weak count of this value.
    #[must_use]
    #[track_caller]
    fn weak_count(&self) -> usize;

    /// Get the strong count of this value.
    #[must_use]
    #[track_caller]
    fn strong_count(&self) -> usize;
}

unsafe impl<T> RefCount for Rc<T>
where
    T: ?Sized,
{
    #[inline(always)]
    fn weak_count(&self) -> usize {
        <Rc<T>>::weak_count(self)
    }

    #[inline(always)]
    fn strong_count(&self) -> usize {
        <Rc<T>>::strong_count(self)
    }
}

unsafe impl<T> RefCount for Weak<T>
where
    T: ?Sized,
{
    #[inline(always)]
    fn weak_count(&self) -> usize {
        <Weak<T>>::weak_count(self)
    }

    #[inline(always)]
    fn strong_count(&self) -> usize {
        <Weak<T>>::strong_count(self)
    }
}

/// A fallible version of [`Rc::new_cyclic`].
#[inline(always)]
#[track_caller]
pub fn try_new_cyclic<T, E, F>(init: F) -> Result<Rc<T>, E>
where
    F: FnOnce(&Weak<T>) -> Result<T, E>,
{
    let mut result = Ok(());

    let value = Rc::<MaybeUninit<T>>::new_cyclic(|weak: &Weak<MaybeUninit<T>>| {
        assert!(weak.strong_count() == 0, "strong count must be zero");

        // SAFETY: `MaybeUninit<T>` and `T` share a memory layout, and we know that there are no currently
        //         existing strong references, so a reinterpretation is sound here.
        let temp: Weak<T> = unsafe { Weak::from_raw(weak.clone().into_raw().cast::<T>()) };

        match init(&temp) {
            Ok(value) => MaybeUninit::new(value),
            Err(error) => {
                result = Err(error);
                MaybeUninit::uninit()
            }
        }
    });

    match result {
        // SAFETY: If we have the `Ok` case, then we know that we initialized the memory.
        Ok(()) => Ok(unsafe { value.assume_init() }),
        Err(error) => Err(error),
    }
}
