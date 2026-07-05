//! The raw components of pointers.

use crate::ptr::Pointee;

/// The raw components of a pointer that stores its address first.
#[repr(C)]
pub(crate) struct AddressFirst<P>
where
    P: ?Sized + Pointee,
{
    pub(crate) address: *const (),
    pub(crate) metadata: P::Metadata,
}

impl<P> Clone for AddressFirst<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for AddressFirst<P> where P: ?Sized + Pointee {}

/// The raw components of a pointer that stores its address last.
#[repr(C)]
pub(crate) struct AddressLast<P>
where
    P: ?Sized + Pointee,
{
    pub(crate) metadata: P::Metadata,
    pub(crate) address: *const (),
}

impl<P> Clone for AddressLast<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for AddressLast<P> where P: ?Sized + Pointee {}

/// The raw components of a pointer that stores only its address.
#[repr(C)]
pub(crate) struct AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    pub(crate) address: *const (),
    pub(crate) metadata: [P::Metadata; 0],
}

impl<P> Clone for AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for AddressOnly<P> where P: ?Sized + Pointee {}

/// The raw components of a pointer.
#[repr(C)]
pub(crate) union Components<P>
where
    P: ?Sized + Pointee,
{
    pub(crate) pointer: *const P,
    pub(crate) address_first: AddressFirst<P>,
    pub(crate) address_last: AddressLast<P>,
    pub(crate) address_only: AddressOnly<P>,
}

impl<P> Clone for Components<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for Components<P> where P: ?Sized + Pointee {}
