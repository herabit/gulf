//! Type level utilities.

use core::{any, fmt, marker::PhantomData};

/// A marker trait that is implemented for all types that proves `Self == Self::This`.
///
/// Currently has little use beyond [`SameAs`].
///
/// # Safety
///
/// This is an `unsafe` trait just to indicate that if the implementations are
/// wrong, then bad things may occur. Fortunately there's only one possible implementation.
pub unsafe trait Identity {
    /// `Self` by some other name.
    type This: ?Sized;
}

// SAFETY: All types are equivalent to themselves.
unsafe impl<T> Identity for T
where
    T: ?Sized,
{
    type This = T;
}

/// A marker trait indicating that `Self == T`.
pub trait SameAs<T = Self>: Identity<This = T>
where
    T: ?Sized,
{
}

impl<A, B> SameAs<B> for A
where
    A: ?Sized + Identity<This = B>,
    B: ?Sized,
{
}

/// Type that helps do compile time-checks that two types are the same.
#[repr(transparent)]
#[must_use]
pub struct AssertSame<A = (), B = A>(PhantomData<fn(A) -> A>, PhantomData<fn(B) -> B>)
where
    A: ?Sized + SameAs<B>,
    B: ?Sized;

impl AssertSame {
    /// Convenient constructor for this.
    #[inline(always)]
    pub const fn of<A, B>() -> AssertSame<A, B>
    where
        A: ?Sized + SameAs<B>,
        B: ?Sized,
    {
        AssertSame::new()
    }

    /// Convenient constructor for this.
    #[inline(always)]
    pub const fn of_val<A, B>(
        _: Option<&A>,
        _: Option<&B>,
    ) -> AssertSame<A, B>
    where
        A: ?Sized + SameAs<B>,
        B: ?Sized,
    {
        AssertSame::new()
    }
}

impl<A, B> AssertSame<A, B>
where
    A: ?Sized + SameAs<B>,
    B: ?Sized,
{
    /// Just a constructor for this type.
    #[inline(always)]
    pub const fn new() -> AssertSame<A, B> {
        AssertSame(PhantomData, PhantomData)
    }
}

impl<A, B> Clone for AssertSame<A, B>
where
    A: ?Sized + SameAs<B>,
    B: ?Sized,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for AssertSame<A, B>
where
    A: ?Sized + SameAs<B>,
    B: ?Sized,
{
}

impl<A, B> Default for AssertSame<A, B>
where
    A: ?Sized + SameAs<B>,
    B: ?Sized,
{
    #[inline(always)]
    fn default() -> Self {
        AssertSame::new()
    }
}

impl<A, B> fmt::Debug for AssertSame<A, B>
where
    A: ?Sized + SameAs<B>,
    B: ?Sized,
{
    #[inline]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        #[track_caller]
        fn inner(
            a: &str,
            b: &str,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            write!(f, "{a} == {b}")
        }

        inner(any::type_name::<A>(), any::type_name::<B>(), f)
    }
}
