//! Tools for handling PipeWire buffers.

#[repr(C, align(8))]
pub struct PodBuf([u8]);
