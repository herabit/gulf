//! Utilities for handling DMA Buffers.

// use libc::DMA

use std::{
    ffi::c_uint,
    io,
    os::fd::{AsRawFd, BorrowedFd},
};

pub const DMA_BUF_BASE: u8 = b'b';
pub const DMA_BUF_IOCTL_SYNC: u8 = 0;

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SyncFlags: u64 {
        const START = 0 << 2;
        const END = 1 << 2;
        const READ = 1 << 0;
        const WRITE = 1 << 1;

        const READ_WRITE = SyncFlags::READ.bits() | SyncFlags::WRITE.bits();

        const _ = !0;
    }
}

pub fn dma_buf_sync(
    fd: BorrowedFd<'_>,
    flags: SyncFlags,
) -> io::Result<()> {
    #[repr(C)]
    struct _dma_buf_sync {
        flags: u64,
    }

    let request = const { libc::_IOW::<_dma_buf_sync>(DMA_BUF_BASE as _, DMA_BUF_IOCTL_SYNC as _) };
    let args = _dma_buf_sync {
        flags: flags.bits(),
    };

    unsafe { libc::ioctl(fd.as_raw_fd(), request, (&raw const args)) }
        .try_into()
        .map(|_: c_uint| ())
        .map_err(|_| io::Error::last_os_error())
}

#[inline(always)]
pub fn dma_buf_access_begin(
    fd: BorrowedFd<'_>,
    read: bool,
    write: bool,
) -> io::Result<()> {
    dma_buf_sync(fd, {
        let mut flags = SyncFlags::START;

        flags.set(SyncFlags::READ, read);
        flags.set(SyncFlags::WRITE, write);

        flags
    })
}

#[inline(always)]
pub fn dma_buf_access_end(
    fd: BorrowedFd<'_>,
    read: bool,
    write: bool,
) -> io::Result<()> {
    dma_buf_sync(fd, {
        let mut flags = SyncFlags::END;

        flags.set(SyncFlags::READ, read);
        flags.set(SyncFlags::WRITE, write);

        flags
    })
}
