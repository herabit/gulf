use core::{error, fmt, hash, num::NonZero, ptr::NonNull};
use std::mem;

use crate::ptr::{CastMetadata, Pointee, cast_metadata};

pub unsafe trait Pointer:
    Copy + Ord + Unpin + hash::Hash + fmt::Debug + fmt::Pointer
{
    type Address: 'static
        + Copy
        + Send
        + Sync
        + Ord
        + Unpin
        + TryFrom<usize>
        + Into<usize>
        + hash::Hash
        + fmt::Debug
        + fmt::Display
        + fmt::Binary
        + fmt::LowerHex
        + fmt::UpperHex
        + fmt::Octal
        + fmt::UpperExp
        + fmt::LowerExp;

    type Pointee: ?Sized + Pointee;

    type Cast<U>: Pointer<Pointee = U, Address = Self::Address>
    where
        U: ?Sized + Pointee;
}

unsafe impl<P> Pointer for *const P
where
    P: ?Sized + Pointee,
{
    type Address = usize;
    type Pointee = P;

    type Cast<U>
        = *const U
    where
        U: ?Sized + Pointee;
}

unsafe impl<P> Pointer for *mut P
where
    P: ?Sized + Pointee,
{
    type Address = usize;
    type Pointee = P;

    type Cast<U>
        = *mut U
    where
        U: ?Sized + Pointee;
}

unsafe impl<P> Pointer for NonNull<P>
where
    P: ?Sized + Pointee,
{
    type Address = NonZero<usize>;
    type Pointee = P;

    type Cast<U>
        = NonNull<U>
    where
        U: ?Sized + Pointee;
}

#[inline(always)]
#[must_use]
#[track_caller]
pub const fn compose<Ptr>(
    address: Ptr::Cast<()>,
    metadata: <Ptr::Pointee as Pointee>::Metadata,
) -> Ptr
where
    Ptr: Pointer,
{
    // SAFETY: `Ptr::Cast<()>` is a some kind of raw pointer to a `()`, so transmuting between a thin pointer
    //          and a different thin pointer type is fine.
    let composed = Ptr::Pointee::LAYOUT.compose(unsafe { mem::transmute_copy(&address) }, metadata);

    // SAFETY: We know that the address is valid for `Ptr`.
    unsafe { mem::transmute_copy(&composed) }
}

#[inline(always)]
#[must_use]
#[track_caller]
pub const fn decompose<Ptr>(pointer: Ptr) -> (Ptr::Cast<()>, <Ptr::Pointee as Pointee>::Metadata)
where
    Ptr: Pointer,
{
    // SAFETY: We know that `pointer` is just a raw pointer to `Ptr::Pointee`, so we can just transmute.
    let (address, metadata) =
        Ptr::Pointee::LAYOUT.decompose(unsafe { mem::transmute_copy(&pointer) });

    // SAFETY: We know the address is a valid address for `Ptr::Cast<()>`.
    (unsafe { mem::transmute_copy(&address) }, metadata)
}

#[inline(always)]
#[must_use]
#[track_caller]
pub const fn cast<Ptr, Dest>(ptr: Ptr) -> Ptr::Cast<Dest>
where
    Ptr: Pointer,
    Dest: ?Sized + Pointee,
    Ptr::Cast<Dest>: Pointer<Cast<()> = Ptr::Cast<()>>,
    <Ptr::Pointee as Pointee>::Metadata: CastMetadata<Dest::Metadata>,
{
    let (address, metadata) = decompose(ptr);

    compose(address, cast_metadata(metadata))
}
