//! Utilities for handling the pointees of pointers.

use crate::ptr::{CStrMetadata, Metadata, PointerLayout};
use core::ffi::CStr;

#[cfg(feature = "std")]
use std::{ffi::OsStr, path::Path};

/// Trait for types that can be pointed to with a pointer.
pub unsafe trait Pointee {
    /// The metadata of this pointee.
    type Metadata: Metadata;

    /// The memory layout for pointers pointing to this type.
    const LAYOUT: PointerLayout<Self>;
}

unsafe impl<T> Pointee for T
where
    T: Sized,
{
    type Metadata = ();

    const LAYOUT: PointerLayout<Self> = PointerLayout::<T>::new();
}

unsafe impl<T> Pointee for [T]
where
    T: Sized,
{
    type Metadata = usize;

    const LAYOUT: PointerLayout<Self> = PointerLayout::<[T]>::new();
}

unsafe impl Pointee for str {
    type Metadata = <[u8] as Pointee>::Metadata;

    const LAYOUT: PointerLayout<Self> = PointerLayout::<str>::new();
}

unsafe impl Pointee for CStr {
    type Metadata = CStrMetadata;

    const LAYOUT: PointerLayout<Self> = PointerLayout::<CStr>::new();
}

#[cfg(feature = "std")]
unsafe impl Pointee for OsStr {
    type Metadata = <[u8] as Pointee>::Metadata;

    const LAYOUT: PointerLayout<Self> = PointerLayout::<OsStr>::new();
}

#[cfg(feature = "std")]
unsafe impl Pointee for Path {
    type Metadata = <OsStr as Pointee>::Metadata;

    const LAYOUT: PointerLayout<Self> = PointerLayout::<Path>::new();
}

/// Marker trait for thin pointees.
pub trait Thin: Pointee<Metadata = ()> {}

impl<T> Thin for T where T: ?Sized + Pointee<Metadata = ()> {}

// /// Marker trait for pointees that store a length.
// pub trait Length: Pointee<Metadata = usize> {}
