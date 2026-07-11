//! Linux-specific functionality.

pub mod dma_buf;

#[cfg(not(target_os = "emscripten"))]
pub mod ioctl;

pub mod syscall;
