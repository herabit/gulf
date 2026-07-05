//! Utilities for handling the memory layout of pointers.

use crate::{
    ptr::{
        CStrMetadata, Pointee,
        components::{self, Components},
    },
    unreachable_unchecked,
};
use core::{ffi::CStr, fmt, marker::PhantomData, mem, ptr};

#[cfg(feature = "std")]
use std::{ffi::OsStr, path::Path};

/// A type-level proof that the address of a pointer to `P` is stored first.
pub struct AddressFirst<P>
where
    P: ?Sized + Pointee,
{
    // SAFETY: We need to be invariant over `P`.
    pub(crate) _marker: PhantomData<fn(P) -> P>,
}

impl<P> AddressFirst<P>
where
    P: ?Sized + Pointee,
{
    /// Create a type-level proof that the address of a pointer to `P` is stored
    /// before the metadata.
    ///
    /// # Safety
    ///
    /// The caller needs to ensure that this is actually true.
    #[inline(always)]
    #[allow(unused_unsafe)]
    #[must_use]
    pub const unsafe fn new_unchecked() -> AddressFirst<P> {
        // SAFETY: The caller ensures this is true.
        unsafe {
            AddressFirst {
                _marker: PhantomData,
            }
        }
    }

    /// Compose the components of a pointer whose address is stored first.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn compose(
        self,
        address: *const (),
        metadata: P::Metadata,
    ) -> *const P {
        // SAFETY: We know that pointers to `P` store the address first.
        unsafe {
            Components::<P> {
                address_first: components::AddressFirst { address, metadata },
            }
            .pointer
        }
    }

    /// Decompone the components of a pointer whose address is stored first.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn decompose(
        self,
        pointer: *const P,
    ) -> (*const (), P::Metadata) {
        // SAFETY: We know that pointers to `P` store the address first.
        let components::AddressFirst { address, metadata } =
            unsafe { Components::<P> { pointer }.address_first };

        (address, metadata)
    }

    /// Get the offset of the address from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn address_offset(self) -> usize {
        mem::offset_of!(components::AddressFirst::<P>, address)
    }

    /// Get the offset of the metadata from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn metadata_offset(self) -> usize {
        mem::offset_of!(components::AddressFirst::<P>, metadata)
    }

    /// Cast one pointer layout to another.
    #[inline(always)]
    #[must_use]
    pub const unsafe fn cast_unchecked<P1>(self) -> AddressFirst<P1>
    where
        P1: ?Sized + Pointee,
    {
        // SAFETY: The caller ensures this is okay.
        unsafe { AddressFirst::new_unchecked() }
    }
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

impl<P> fmt::Debug for AddressFirst<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        debug_fmt(
            f,
            "AddressFirst",
            self.address_offset(),
            self.metadata_offset(),
        )
    }
}

/// A type-level proof that the address of a pointer to `P` is stored last.
pub struct AddressLast<P>
where
    P: ?Sized + Pointee,
{
    // SAFETY: We need to be invariant over `P`.
    pub(crate) _marker: PhantomData<fn(P) -> P>,
}

impl<P> AddressLast<P>
where
    P: ?Sized + Pointee,
{
    /// Create a type-level proof that the address of a pointer to `P` is stored
    /// after the metadata.
    ///
    /// # Safety
    ///
    /// The caller needs to ensure that this is actually true.
    #[inline(always)]
    #[allow(unused_unsafe)]
    #[must_use]
    pub const unsafe fn new_unchecked() -> AddressLast<P> {
        // SAFETY: The caller ensures this is true.
        unsafe {
            AddressLast {
                _marker: PhantomData,
            }
        }
    }

    /// Compose the components of a pointer whose address is stored last.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn compose(
        self,
        address: *const (),
        metadata: P::Metadata,
    ) -> *const P {
        // SAFETY: We know that pointers to `P` store the address last.
        unsafe {
            Components::<P> {
                address_last: components::AddressLast { address, metadata },
            }
            .pointer
        }
    }

    /// Decompose the component of a pointer whose address is stored last.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn decompose(
        self,
        pointer: *const P,
    ) -> (*const (), P::Metadata) {
        // SAFETY: We know that pointers to `P` store the address last.
        let components::AddressLast { address, metadata } =
            unsafe { Components::<P> { pointer }.address_last };

        (address, metadata)
    }

    /// Get the offset of the address from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn address_offset(self) -> usize {
        mem::offset_of!(components::AddressLast::<P>, address)
    }

    /// Get the offset of the metadata from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn metadata_offset(self) -> usize {
        mem::offset_of!(components::AddressLast::<P>, metadata)
    }

    /// Cast only pointer layout to another.
    #[inline(always)]
    #[must_use]
    pub const unsafe fn cast_unchecked<P1>(self) -> AddressLast<P1>
    where
        P1: ?Sized + Pointee,
    {
        // SAFETY: The caller ensures this is okay.
        unsafe { AddressLast::new_unchecked() }
    }
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

impl<P> fmt::Debug for AddressLast<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        debug_fmt(
            f,
            "AddressLast",
            self.address_offset(),
            self.metadata_offset(),
        )
    }
}

/// A type-level proof that the address of a pointer to `P` is the only thing within
/// the pointer (no metadata).
///
/// # Safety
///
/// This, however, does NOT imply that `P` is a thin type... Do not conflate the two concepts,
/// as Rust is allowed to have metadata-less pointers that are still dynamically sized.
pub struct AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    // SAFETY: We need to be invariant over `P`.
    pub(crate) _marker: PhantomData<fn(P) -> P>,
}

impl<P> AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    /// Create a new type-level proof that pointers to `P` contain only an address.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `size_of::<P::Metadata>() != 0 || align_of::<P::Metadata>() != 1`.
    #[inline(always)]
    #[allow(unused_unsafe)]
    #[must_use]
    pub const fn new() -> Option<AddressOnly<P>> {
        if size_of::<P::Metadata>() == 0 && align_of::<P::Metadata>() == 1 {
            // SAFETY: We just checked that the necessary preconditions are true.
            Some(unsafe {
                AddressOnly {
                    _marker: PhantomData,
                }
            })
        } else {
            None
        }
    }

    /// Create a new type-level proof that pointers to `P` contain only an address,
    /// and that `size_of::<P::Metadata>() == 0 && align_of::<P::Metadata>() == 1`.
    ///
    /// # Safety
    ///
    /// The caller needs to ensure that the above is true.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const unsafe fn new_unchecked() -> AddressOnly<P> {
        match AddressOnly::<P>::new() {
            Some(address_only) => address_only,
            // SAFETY: The caller ensures the above is true.
            None => match (size_of::<P::Metadata>(), align_of::<P::Metadata>()) {
                (1.., 2..) => unsafe {
                    unreachable_unchecked!(
                        "`size_of::<P::Metadata>() > 0 && align_of::<P::Metadata>() > 1`"
                    )
                },
                (1.., _) => unsafe { unreachable_unchecked!("`size_of::<P::Metadata>() > 0`") },
                (_, 2..) => unsafe { unreachable_unchecked!("`align_of::<P::Metadata>() > 1`") },
                _ => unreachable!(),
            },
        }
    }

    /// Compose the components of a pointer who only stores the address.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn compose(
        self,
        address: *const (),
        metadata: P::Metadata,
    ) -> *const P {
        // SAFETY: We know that pointers to `P` only store the address.
        unsafe {
            Components::<P> {
                address_only: components::AddressOnly {
                    address,
                    metadata: [metadata; 0],
                },
            }
            .pointer
        }
    }

    /// Decompose the components of a pointer who only stores the address.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn decompose(
        self,
        pointer: *const P,
    ) -> (*const (), P::Metadata) {
        // SAFETY: We know that pointers to `P` store only the address.
        let components::AddressOnly { address, metadata } =
            unsafe { Components::<P> { pointer }.address_only };

        // SAFETY: If we have a valid `*const P` and we're storing only the address,
        //         then `P::Metadata` is a ZST that is inhabited (otherwise the
        //         creation of a `*const P` would be impossible).
        (address, unsafe { mem::transmute_copy(&metadata) })
    }

    /// Get the offset of the address from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn address_offset(self) -> usize {
        mem::offset_of!(components::AddressOnly::<P>, address)
    }

    /// Get the offset of the metadata from within the pointer.
    #[inline(always)]
    #[must_use]
    pub const fn metadata_offset(self) -> usize {
        mem::offset_of!(components::AddressOnly::<P>, metadata)
    }

    /// Cast one pointer layout to another.
    #[inline(always)]
    #[must_use]
    pub const unsafe fn cast_unchecked<P1>(self) -> AddressOnly<P1>
    where
        P1: ?Sized + Pointee,
    {
        // SAFETY: The caller ensures this is okay.
        unsafe { AddressOnly::new_unchecked() }
    }
}

impl<P> Clone for AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            _marker: self._marker.clone(),
        }
    }
}

impl<P> Copy for AddressOnly<P> where P: ?Sized + Pointee {}

impl<P> fmt::Debug for AddressOnly<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        debug_fmt(
            f,
            "AddressOnly",
            self.address_offset(),
            self.metadata_offset(),
        )
    }
}

/// Enum describing the memory layout of pointers to `P`.
#[non_exhaustive]
pub enum PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    /// The address is stored first followed by the metadata.
    AddressFirst(AddressFirst<P>),
    /// The metadata is stored first followed by the address.
    AddressLast(AddressLast<P>),
    /// Only the address is stored.
    AddressOnly(AddressOnly<P>),
}

impl<P> PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    /// Cast one pointer layout to another.
    #[inline(always)]
    #[must_use]
    pub const unsafe fn cast_unchecked<P1>(self) -> PointerLayout<P1>
    where
        P1: ?Sized + Pointee,
    {
        // SAFETY: The caller ensures this is fine.
        match self {
            PointerLayout::AddressFirst(address_first) => {
                PointerLayout::AddressFirst(unsafe { address_first.cast_unchecked() })
            }
            PointerLayout::AddressLast(address_last) => {
                PointerLayout::AddressLast(unsafe { address_last.cast_unchecked() })
            }
            PointerLayout::AddressOnly(address_only) => {
                PointerLayout::AddressOnly(unsafe { address_only.cast_unchecked() })
            }
        }
    }

    /// Compose the components of a pointer to a `P`.
    #[inline(always)]
    #[must_use]
    pub const fn compose(
        self,
        address: *const (),
        metadata: P::Metadata,
    ) -> *const P {
        match self {
            PointerLayout::AddressFirst(address_first) => address_first.compose(address, metadata),
            PointerLayout::AddressLast(address_last) => address_last.compose(address, metadata),
            PointerLayout::AddressOnly(address_only) => address_only.compose(address, metadata),
        }
    }

    /// Decompose the components of a pointer to a `P`.
    #[inline(always)]
    #[must_use]
    pub const fn decompose(
        self,
        pointer: *const P,
    ) -> (*const (), P::Metadata) {
        match self {
            PointerLayout::AddressFirst(address_first) => address_first.decompose(pointer),
            PointerLayout::AddressLast(address_last) => address_last.decompose(pointer),
            PointerLayout::AddressOnly(address_only) => address_only.decompose(pointer),
        }
    }

    /// Get the offset of the address in a pointer to a `P`.
    #[inline(always)]
    #[must_use]
    pub const fn address_offset(self) -> usize {
        match self {
            PointerLayout::AddressFirst(address_first) => address_first.address_offset(),
            PointerLayout::AddressLast(address_last) => address_last.address_offset(),
            PointerLayout::AddressOnly(address_only) => address_only.address_offset(),
        }
    }

    /// Get the offset of the metadata in a pointer to a `P`.
    #[inline(always)]
    #[must_use]
    pub const fn metadata_offset(self) -> usize {
        match self {
            PointerLayout::AddressFirst(address_first) => address_first.metadata_offset(),
            PointerLayout::AddressLast(address_last) => address_last.metadata_offset(),
            PointerLayout::AddressOnly(address_only) => address_only.metadata_offset(),
        }
    }
}

impl<T> PointerLayout<T>
where
    T: Sized,
{
    #[inline(always)]
    #[must_use]
    pub const fn new() -> PointerLayout<T> {
        const { PointerLayout::AddressOnly(AddressOnly::new().expect("what the fuck lol")) }
    }

    #[inline(always)]
    #[must_use]
    pub const fn slice() -> PointerLayout<[T]> {
        PointerLayout::<[T]>::new()
    }

    #[inline(always)]
    #[must_use]
    pub const fn to_slice(self) -> PointerLayout<[T]> {
        PointerLayout::<[T]>::new()
    }
}

impl<T> PointerLayout<[T]>
where
    T: Sized,
{
    #[inline(always)]
    #[must_use]
    #[track_caller]
    const fn new_inner() -> PointerLayout<[T]> {
        const {
            let addr = ptr::without_provenance::<T>(0xFF);
            let ptr = ptr::slice_from_raw_parts(addr, 0x00);

            let [a, b]: [usize; 2] = unsafe { mem::transmute(ptr) };

            match [a, b] {
                // SAFETY: We know that the length `0x00` is second and the address `0xFF` is first.
                [0xFF, 0x00] => {
                    PointerLayout::AddressFirst(unsafe { AddressFirst::new_unchecked() })
                }
                // SAFETY: We know that the length `0x00` is first and the address `0xFF` is second.
                [0x00, 0xFF] => PointerLayout::AddressLast(unsafe { AddressLast::new_unchecked() }),
                _ => unreachable!(),
            }
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn new() -> PointerLayout<[T]> {
        const {
            let byte_slice = PointerLayout::<[u8]>::new_inner();
            let this_slice = PointerLayout::<[T]>::new_inner();

            assert!(
                this_slice.address_offset() == byte_slice.address_offset(),
                "address offset mismatch"
            );
            assert!(
                this_slice.metadata_offset() == byte_slice.metadata_offset(),
                "metadata offset mismatch"
            );

            this_slice
        }
    }

    #[inline(always)]
    #[must_use]
    pub const fn elem() -> PointerLayout<T> {
        PointerLayout::<T>::new()
    }

    #[inline(always)]
    #[must_use]
    pub const fn to_elem(self) -> PointerLayout<T> {
        PointerLayout::<T>::new()
    }
}

impl PointerLayout<str> {
    #[inline(always)]
    #[must_use]
    pub const fn new() -> PointerLayout<str> {
        // SAFETY: We need a better way to verify the validity of this.
        unsafe { PointerLayout::<[u8]>::new().cast_unchecked() }
    }
}

impl PointerLayout<CStr> {
    #[inline(always)]
    #[must_use]
    pub const fn new() -> PointerLayout<CStr> {
        const {
            match CStrMetadata::new(0).get() {
                // SAFETY: We know `CStr` contains a length.
                Some(..) => unsafe { PointerLayout::<[u8]>::new().cast_unchecked() },
                // SAFETY: We know `CStr` contains no length.
                None => PointerLayout::AddressOnly(AddressOnly::new().expect("lol what the fuck")),
            }
        }
    }
}

#[cfg(feature = "std")]
impl PointerLayout<OsStr> {
    #[inline(always)]
    #[must_use]
    pub const fn new() -> PointerLayout<OsStr> {
        const {
            assert!(size_of::<*const OsStr>() == size_of::<[usize; 2]>());

            unsafe { PointerLayout::<[u8]>::new().cast_unchecked() }
        }
    }
}

#[cfg(feature = "std")]
impl PointerLayout<Path> {
    #[inline(always)]
    #[must_use]
    pub const fn new() -> PointerLayout<Path> {
        const { unsafe { PointerLayout::<OsStr>::new().cast_unchecked() } }
    }
}

impl<P> Clone for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for PointerLayout<P> where P: ?Sized + Pointee {}

impl<P> From<AddressFirst<P>> for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn from(layout: AddressFirst<P>) -> Self {
        PointerLayout::AddressFirst(layout)
    }
}

impl<P> From<AddressLast<P>> for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn from(layout: AddressLast<P>) -> Self {
        PointerLayout::AddressLast(layout)
    }
}

impl<P> From<AddressOnly<P>> for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn from(layout: AddressOnly<P>) -> Self {
        PointerLayout::AddressOnly(layout)
    }
}

impl<P> fmt::Debug for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            PointerLayout::AddressFirst(address_first) => address_first.fmt(f),
            PointerLayout::AddressLast(address_last) => address_last.fmt(f),
            PointerLayout::AddressOnly(address_only) => address_only.fmt(f),
        }
    }
}

impl<P> Default for PointerLayout<P>
where
    P: ?Sized + Pointee,
{
    #[inline(always)]
    fn default() -> Self {
        P::LAYOUT
    }
}

#[inline(never)]
#[track_caller]
fn debug_fmt(
    f: &mut fmt::Formatter<'_>,
    variant: &str,
    address_offset: usize,
    metadata_offset: usize,
) -> fmt::Result {
    f.debug_struct(variant)
        .field("address_offset", &address_offset)
        .field("metadata_offset", &metadata_offset)
        .finish_non_exhaustive()
}
