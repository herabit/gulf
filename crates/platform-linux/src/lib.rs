//! Platform specific utilities for linux.

#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod atomic_fd;
pub mod dma_buf;
pub mod errno;
pub mod mem_fd;
