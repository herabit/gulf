//! Provides hints to the compiler.

use core::hint;

/// Marks some [`prim@bool`] as being likely [`prim@true`].
#[inline(always)]
#[must_use]
pub const fn likely(b: bool) -> bool {
    if !b {
        hint::cold_path();
    }

    b
}

/// Marks some [`prim@bool`] as being likely [`prim@false`].
#[inline(always)]
#[must_use]
pub const fn unlikely(b: bool) -> bool {
    if b {
        hint::cold_path();
    }

    b
}

/// Marks some value of type `T` as requiring an `unsafe` block.
///
/// # Safety
///
/// This function doesn't actually do anything unsafe. Instead,
/// it provides a helper for defining things as requiring an unsafe block,
/// even when they may not really need it.
///
/// This is handy for macros.
#[inline(always)]
pub const unsafe fn needs_unsafe<T>(x: T) -> T {
    x
}

/// Marks some value of type `T` as `#[must_use]`.
#[inline(always)]
#[must_use]
pub const fn must_use<T>(x: T) -> T {
    x
}

/// Returns whether UB checks are enabled.
#[inline(always)]
#[must_use]
pub const fn ub_checks() -> bool {
    cfg!(debug_assertions) | cfg!(miri)
}

#[doc(hidden)]
#[macro_export]
macro_rules! __unreachable_unchecked_impl {
    (($($first:tt)+) $(, $($rest:tt)*)?) => {
        if $crate::hint::ub_checks() {
            ::core::panic!(
                ::core::concat!(
                    "undefined behavior: ",
                    $($first)+,
                ),
                $($($rest)*)?
            )
        } else {
            ::core::hint::unreachable_unchecked()
        }
    };
}

/// A macro that marks a code path as unreachable to LLVM.
///
/// The syntax is the same as [`macro@unreachable`].
#[macro_export]
macro_rules! unreachable_unchecked {
    () => {
        $crate::unreachable_unchecked!(
            "the `unreachable_unchecked` invocation must never be reached",
        )
    };
    ($first:tt $(, $($rest:tt)*)?) => {
        $crate::__unreachable_unchecked_impl!(
            ($first),
            $( $($rest)* )?
        )
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __assert_unchecked_impl {
    ($cond:expr, ($($first:tt)+) $(, $($rest:tt)*)?) => {
        if $crate::hint::ub_checks() {
            ::core::assert!(
                $cond,
                ::core::concat!(
                    "undefined behavior: ",
                    $($first)+,
                ),
                $($($rest)*)?
            )
        } else {
            ::core::hint::assert_unchecked($cond)
        }
    };
}

/// A macro that asserts some condition to always be true to LLVM.
///
/// The syntax is the same as [`macro@assert`].
#[macro_export]
macro_rules! assert_unchecked {
    ($cond:expr $(,)?) => {
        $crate::__assert_unchecked_impl!(
            $cond,
            (::core::concat!(
                "assertion failed (`",
                ::core::stringify!($cond),
                "`)",
            )),
        )
    };
    ($cond:expr, $first:tt $(, $($rest:tt)*)?) => {
        $crate::__assert_unchecked_impl!(
            $cond,
            ($first),
            $($($rest)*)?
        )
    };
}
