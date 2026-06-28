//! Extended functionality for [`Arc`](::alloc::sync::Arc).

use alloc::sync::{Arc, Weak};
use core::{
    convert::Infallible,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::atomic::{Ordering, fence},
};

#[cfg(feature = "std")]
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind};

/// Trait for types that are atomically reference counted.
pub unsafe trait AtomicRefCount {
    /// Get the weak count of this value with the specified memory order,
    /// defaulting to [`Ordering::Relaxed`] if `order` is [`None`].
    ///
    /// # Panics
    ///
    /// Panics if `order` is invalid.
    #[must_use]
    #[track_caller]
    fn weak_count(
        &self,
        order: Option<Ordering>,
    ) -> usize;

    /// Get the strong count of this value with the specified memory order,
    /// defaulting to [`Ordering::Relaxed`] if `order` is [`None`].
    ///
    /// Panics if `order` is invalid.
    #[must_use]
    #[track_caller]
    fn strong_count(
        &self,
        order: Option<Ordering>,
    ) -> usize;
}

#[inline(always)]
#[track_caller]
#[must_use]
fn load_count<'count, L, C>(
    load: L,
    count: &'count C,
    order: Option<Ordering>,
) -> usize
where
    L: FnOnce(&'count C) -> usize,
    C: ?Sized,
{
    match order {
        Some(Ordering::Relaxed) | None => load(count),
        Some(Ordering::Acquire) => {
            let value = load(count);
            fence(Ordering::Acquire);
            value
        }
        Some(Ordering::SeqCst) => {
            let value = load(count);
            fence(Ordering::SeqCst);
            value
        }
        Some(Ordering::Release) => panic!("there is no such thing as a release load"),
        Some(Ordering::AcqRel) => panic!("there is no such thing as an acquire-release load"),
        Some(order) => panic!("unknown memory ordering: {order:?}"),
    }
}

unsafe impl<T> AtomicRefCount for Arc<T>
where
    T: ?Sized,
{
    #[inline(always)]
    fn weak_count(
        &self,
        order: Option<Ordering>,
    ) -> usize {
        load_count(Arc::weak_count, self, order)
    }

    #[inline(always)]
    fn strong_count(
        &self,
        order: Option<Ordering>,
    ) -> usize {
        load_count(Arc::strong_count, self, order)
    }
}

unsafe impl<T> AtomicRefCount for Weak<T>
where
    T: ?Sized,
{
    #[inline(always)]
    fn strong_count(
        &self,
        order: Option<Ordering>,
    ) -> usize {
        load_count(Weak::strong_count, self, order)
    }

    #[inline(always)]
    fn weak_count(
        &self,
        order: Option<Ordering>,
    ) -> usize {
        load_count(Weak::weak_count, self, order)
    }
}

enum TryNewCyclicInner<T, E, I>
where
    I: FnOnce(&Weak<T>) -> Result<T, E>,
{
    /// We have access to the standard library
    #[cfg(feature = "std")]
    CatchUnwind { init: I },

    /// We want to panic immediately upon error...
    ///
    /// This is not ideal, but it works for `no_std` targets.
    PanicOnError { init: I },

    /// This is assumed to always be sound to call.
    Unchecked { init: I },

    /// This is just used for markers.
    #[allow(dead_code)]
    Phantom {
        marker: PhantomData<dyn FnOnce(&Weak<T>) -> Result<T, E>>,
        void: Infallible,
    },
}

impl<T, E, I> TryNewCyclicInner<T, E, I>
where
    I: FnOnce(&Weak<T>) -> Result<T, E>,
{
    #[inline]
    #[track_caller]
    #[cfg(feature = "std")]
    fn _catch_unwind(init: I) -> Result<Arc<T>, E> {
        struct InitError;

        // SAFETY: This is initialized when we catch an `InitError` panic.
        let mut error = MaybeUninit::<E>::uninit();

        let init = |weak: &Weak<MaybeUninit<T>>| {
            assert!(weak.strong_count() == 0, "strong count must be zero");

            // SAFETY: `MaybeUninit<T>` and `T` share a memory layout,
            //         and we ensure (through panicking) that the strong
            //         count remains zero when `init` fails.
            //
            //         It would be a massive soundness error for `Arc<T>::new_cyclic`
            //         to update the strong count *before* writing the value.
            //
            //         So if `init` fails, we simply panic, preventing the update of
            //         the strong count. Otherwise? We catch the panic, check if it
            //         panicked with `IsUnwind`, if it did, then we know that `result`
            //         was set. Otherwise, we continue panicking.
            let temp: Weak<T> = unsafe { Weak::from_raw(weak.clone().into_raw().cast::<T>()) };

            match init(&temp) {
                // NOTE: We're using panics to represent the error case, and the lackthereof to indicate the
                //       success case.
                Ok(init) => MaybeUninit::new(init),
                // SAFETY: This is how we ensure that we do not update the strong count, this is
                //         a private type and we look for it when we catch the later unwind to determine
                //         if the unwind resulted in an error, came from `init`, etc.
                //
                //         This ensures that any `Weak<T>`s that are created and sent to other threads
                //         don't accidentally permit the accessing of uninitialized data, as the strong_count
                //
                //         Removing any of this is a soundness issue.
                Err(err) => {
                    error.write(err);
                    panic_any(InitError)
                }
            }
        };

        match catch_unwind(AssertUnwindSafe(|| Arc::new_cyclic(init))) {
            // SAFETY: Since there was no panic or abort, we know `value` is initialized.
            Ok(value) => Ok(unsafe { value.assume_init() }),

            // NOTE: We know that we panicked with `InitError`. We now know `error` is initialized.
            Err(payload) if payload.is::<InitError>() => {
                // NOTE: `InitError` has no destructor, and I don't trust Rust to do to omit the destructor for
                //       the payload.
                ::core::mem::forget(payload);

                // SAFETY: We know `error` is initialized.
                Err(unsafe { error.assume_init() })
            }

            // NOTE: This is an actual panic, continue!
            Err(payload) => resume_unwind(payload),
        }
    }

    #[inline]
    #[track_caller]
    fn _panic_on_error(init: I) -> Result<Arc<T>, E> {
        let init = |weak: &Weak<MaybeUninit<T>>| {
            assert!(weak.strong_count() == 0, "strong count must be zero");

            // SAFETY: Same layout details as in `_catch_unwind`, but instead of
            //         panicking followed by catching the panic on an error, we instead
            //         just panic on an error, also nullifiying the ability for the strong
            //         count to become one, negating the possibility of other threads gaining
            //         access to uninitialized data.
            let temp: Weak<T> = unsafe { Weak::from_raw(weak.clone().into_raw().cast::<T>()) };

            match init(&temp) {
                Ok(init) => MaybeUninit::new(init),
                // SAFETY: This panic prevent the strong count from being incremented.
                Err(_) => panic!("failed to create cyclic `Arc`"),
            }
        };

        // SAFETY: We panic on error, thus since there was no error, then we know the value
        //         has been initialized.
        Ok(unsafe { Arc::new_cyclic(init).assume_init() })
    }

    #[inline]
    #[track_caller]
    unsafe fn _unchecked(init: I) -> Result<Arc<T>, E> {
        let mut result = Ok(());

        let init = |weak: &Weak<MaybeUninit<T>>| {
            assert!(weak.strong_count() == 0, "strong count must be zero");

            // SAFETY: Same layout details as in `_catch_unwind`, but instead of catching a panic on error to ensure
            //         short-circuiting behavior on error to prevent the strong count from being updated,
            //         we *instead* assume that `init` does *not* send any `Weak<T>` to another thread that observes an
            //         is then used to create an `Arc<T>` during the duration of this function call.
            //
            //         There is a race condition that may be observed if the caller fails to do so, and said race condition,
            //         if observed, may accidentally expose an uninitialized `T` to its user.
            //
            //         Wait until `_unchecked` finishes executing before, well, letting other threads get a `Arc<T>`.
            let temp: Weak<T> = unsafe { Weak::from_raw(weak.clone().into_raw().cast::<T>()) };

            match init(&temp) {
                Ok(init) => MaybeUninit::new(init),
                // SAFETY: After this, there will be a short period where `strong_count == 1`, which other threads may observe...
                //
                //
                //         Use at your own risk.
                Err(error) => {
                    result = Err(error);
                    MaybeUninit::uninit()
                }
            }
        };

        let value = Arc::new_cyclic(init);

        match result {
            // SAFETY: The strong count is accurate, we know the memory to be initialized.
            Ok(()) => Ok(unsafe { value.assume_init() }),
            Err(error) => {
                // NOTE: If an error has occurred, then the strong count should be equal exactly to one.
                //
                //       If it isn't, then we've entered an irrecoverable state.
                #[cfg(debug_assertions)]
                if value.strong_count(Some(Ordering::Acquire)) > 1 {
                    crate::abort()
                }

                // NOTE: We want to decrement the strong count as soon as possible.
                drop(value);

                Err(error)
            }
        }
    }
}

/// A wrapper for an initializer that can be used to fallibly create an `Arc<T>`.
#[repr(transparent)]
pub struct TryNewCyclic<T, E, I>
where
    I: FnOnce(&Weak<T>) -> Result<T, E>,
{
    inner: TryNewCyclicInner<T, E, I>,
}

impl<T, E, I> TryNewCyclic<T, E, I>
where
    I: FnOnce(&Weak<T>) -> Result<T, E>,
{
    #[inline(always)]
    #[must_use]
    #[track_caller]
    #[cfg(feature = "std")]
    pub const fn catch_unwind(init: I) -> TryNewCyclic<T, E, I> {
        TryNewCyclic {
            inner: TryNewCyclicInner::CatchUnwind { init },
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn panic_on_error(init: I) -> TryNewCyclic<T, E, I> {
        TryNewCyclic {
            inner: TryNewCyclicInner::PanicOnError { init },
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const unsafe fn unchecked(init: I) -> TryNewCyclic<T, E, I> {
        TryNewCyclic {
            inner: TryNewCyclicInner::Unchecked { init },
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn new(init: I) -> TryNewCyclic<T, E, I> {
        cfg_select! {
            feature = "std" => TryNewCyclic::catch_unwind(init),
            _ => TryNewCyclic::panic_on_error(init),
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub fn run(self) -> Result<Arc<T>, E> {
        match self.inner {
            #[cfg(feature = "std")]
            TryNewCyclicInner::CatchUnwind { init } => TryNewCyclicInner::_catch_unwind(init),
            TryNewCyclicInner::PanicOnError { init } => TryNewCyclicInner::_panic_on_error(init),
            TryNewCyclicInner::Unchecked { init } => unsafe { TryNewCyclicInner::_unchecked(init) },
            TryNewCyclicInner::Phantom { void, .. } => match void {},
        }
    }
}
