use std::{
    io::{Error, ErrorKind},
    num::NonZero,
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
    ptr::{self, NonNull},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use bytes::Bytes;
use gulf_core::mem::Align;
use libc::_SC_PAGESIZE;

/// Get the system page size.
#[track_caller]
pub fn page_size() -> std::io::Result<Align> {
    static CACHE: AtomicUsize = AtomicUsize::new(0);

    match Align::new(CACHE.load(Ordering::Acquire)) {
        Some(page_size) => Ok(page_size),
        None => {
            let page_size = Ok(unsafe { libc::sysconf(_SC_PAGESIZE) })
                .and_then(|page_size| usize::try_from(page_size))
                .map_err(|_| Error::last_os_error())
                .and_then(|page_size| {
                    Align::try_from(page_size).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "page size must be a power of two")
                    })
                })?;

            CACHE.store(page_size.get(), Ordering::Release);

            Ok(page_size)
        }
    }
}

/// The underlying memory to be shared.
#[derive(Debug)]
#[non_exhaustive]
pub enum Memory {
    /// The underlying memory is just a buffer.
    Bytes(Bytes),
    /// The underlying memory is a DMA-BUF file descriptor.
    DmaBuf(OwnedFd),
}

impl From<Bytes> for Memory {
    #[inline(always)]
    fn from(bytes: Bytes) -> Self {
        Memory::Bytes(bytes)
    }
}

impl From<Vec<u8>> for Memory {
    #[inline(always)]
    fn from(bytes: Vec<u8>) -> Self {
        Memory::Bytes(bytes.into())
    }
}

impl From<Box<[u8]>> for Memory {
    #[inline(always)]
    fn from(bytes: Box<[u8]>) -> Self {
        Memory::Bytes(bytes.into())
    }
}

impl From<Arc<[u8]>> for Memory {
    #[inline(always)]
    fn from(bytes: Arc<[u8]>) -> Self {
        Memory::Bytes(Bytes::from_owner(bytes))
    }
}

impl From<&'static [u8]> for Memory {
    #[inline(always)]
    fn from(bytes: &'static [u8]) -> Self {
        Memory::Bytes(Bytes::from_static(bytes))
    }
}

impl<const N: usize> From<[u8; N]> for Memory {
    #[inline(always)]
    fn from(bytes: [u8; N]) -> Self {
        Memory::Bytes(if N > 0 {
            Vec::from(bytes).into()
        } else {
            Bytes::new()
        })
    }
}

impl<const N: usize> From<&'static [u8; N]> for Memory {
    #[inline(always)]
    fn from(bytes: &'static [u8; N]) -> Self {
        Memory::Bytes(Bytes::from_static(bytes))
    }
}

impl<const N: usize> From<Box<[u8; N]>> for Memory {
    #[inline(always)]
    fn from(bytes: Box<[u8; N]>) -> Self {
        Memory::Bytes(if N > 0 {
            Vec::from(bytes as Box<[u8]>).into()
        } else {
            Bytes::new()
        })
    }
}

impl<const N: usize> From<Arc<[u8; N]>> for Memory {
    #[inline(always)]
    fn from(bytes: Arc<[u8; N]>) -> Self {
        (bytes as Arc<[u8]>).into()
    }
}
