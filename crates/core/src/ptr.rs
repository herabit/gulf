//! Module for pointer related utilities.

// use core::{
//     fmt, hash,
//     marker::PhantomData,
//     ptr::{self, NonNull},
// };

pub(crate) mod components;
pub(crate) mod layout;
pub(crate) mod metadata;
pub(crate) mod pointee;
pub(crate) mod pointer;

#[doc(inline)]
pub use layout::*;

#[doc(inline)]
pub use metadata::*;

#[doc(inline)]
pub use pointee::*;

#[doc(inline)]
pub use pointer::*;

// /// Trait for types that are supported pointees of pointers.
// pub unsafe trait Pointee {
//     type Metadata: Metadata;

//     /// Describes the memory layout of pointer to this type.
//     const _A: () = ();
// }
