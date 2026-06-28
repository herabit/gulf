//! Video related functionality.

/// An enumeration for pixel formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum PixelFormat {
    #[default]
    Nv12,
}
