//! Utilities for dealing with memory layouts.

#[path = "layout_private.rs"]
pub(crate) mod private;

#[path = "align.rs"]
pub(crate) mod align;

#[path = "mask.rs"]
pub(crate) mod mask;

#[doc(inline)]
pub use align::*;

#[doc(inline)]
pub use mask::*;

// TODO: Finish the implementation for all of these types. I really, really, really want to move onto other things,
//       so I am leaving this here for now.
