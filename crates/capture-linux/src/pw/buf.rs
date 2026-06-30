//! Tools for handling PipeWire buffers.

// #[repr(C, align(8))]
#[repr(transparent)]
pub struct PodBuf([u8]);
