//! Platform specific utilities for linux.

#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod dma_buf;
pub mod errno;
pub mod memfd;
