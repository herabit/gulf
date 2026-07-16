use crate::errno::Errno;
use core::{
    ffi::{c_int, c_ulong},
    ops,
};
use gulf_core::{ty::AssertSame, unreachable_unchecked};

#[cfg(feature = "std")]
use std::os::fd::{AsRawFd as _, BorrowedFd, FromRawFd, OwnedFd, RawFd};

pub const DMA_BUF_BASE: u8 = b'b';

#[cfg(feature = "std")]
const _: AssertSame<c_int, RawFd> = AssertSame::new();

/// Represents what access rights we wish to request from a DMA buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SyncRights {
    /// Indicates that the mapped DMA buffer will be read by the
    /// client via the CPU map.
    Read = 0b001,
    /// Indicates that the mapped DMA buffer will be written by the
    /// client via the CPU map.
    Write = 0b010,
    /// Indicates that the mapped DMA buffer will be read and written by the
    /// client via the CPU map.
    ReadWrite = 0b011,
}

impl SyncRights {
    /// Create a some [`SyncRights`] from a `read` and `write` flag.
    #[inline(always)]
    #[must_use]
    pub const fn new(
        read: bool,
        write: bool,
    ) -> Option<SyncRights> {
        let read = if read { SyncRights::Read as u8 } else { 0 };
        let write = if write { SyncRights::Write as u8 } else { 0 };

        SyncRights::from_bits(read | write)
    }

    /// Create some [`SyncRights`] from its raw bit representation.
    #[inline(always)]
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<SyncRights> {
        match bits {
            0b001 => {
                assert!(bits == SyncRights::Read as u8);
                Some(SyncRights::Read)
            }
            0b010 => {
                assert!(bits == SyncRights::Write as u8);
                Some(SyncRights::Write)
            }
            0b011 => {
                assert!(bits == SyncRights::ReadWrite as u8);
                Some(SyncRights::ReadWrite)
            }
            _ => None,
        }
    }

    /// Returns whether these rights permit reading.
    #[inline(always)]
    #[must_use]
    pub const fn can_read(self) -> bool {
        matches!(self, SyncRights::Read | SyncRights::ReadWrite)
    }

    /// Returns whether these rights permit writing.
    #[inline(always)]
    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self, SyncRights::Write | SyncRights::ReadWrite)
    }

    /// Import a synchronous file descriptor for the provided DMA-BUF
    /// file descriptor with the rights specified by `self`.
    ///
    /// This calls the [`dma_buf_import_sync_file`] IOCTL with `self`
    /// as the flags argument and `sync_fd` as the fd argument. See
    /// there for more info on what this does, and how to use it.
    ///
    /// [`dma_buf_import_sync_file`]: https://docs.kernel.org/driver-api/dma-buf.html#c.dma_buf_import_sync_file
    #[inline(always)]
    pub fn import_sync_raw(
        self,
        dma_buf_fd: c_int,
        sync_fd: c_int,
    ) -> Result<(), Errno> {
        const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: c_ulong =
            libc::_IOW::<DmaBufImportSyncFile>(DMA_BUF_BASE as u32, 3);

        #[repr(C)]
        struct DmaBufImportSyncFile {
            flags: u32,
            fd: i32,
        }

        let mut payload = DmaBufImportSyncFile {
            flags: self as u8 as u32,
            fd: sync_fd.try_into().map_err(|_| Errno(libc::EBADF))?,
        };

        // SAFETY: This IOCTL shouldn't really cause any unsoundness on its own.
        let result =
            unsafe { libc::ioctl(dma_buf_fd, DMA_BUF_IOCTL_IMPORT_SYNC_FILE, &raw mut payload) };

        match result {
            0.. => Ok(()),
            ..0 => Err(Errno::last()),
        }
    }

    /// Same as [`SyncRights::import_sync_raw`] but type-safe.
    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn import_sync(
        self,
        dma_buf_fd: BorrowedFd<'_>,
        sync_fd: BorrowedFd<'_>,
    ) -> Result<(), Errno> {
        self.import_sync_raw(dma_buf_fd.as_raw_fd(), sync_fd.as_raw_fd())
    }

    /// Export a synchronous file descriptor for the provided DMA-BUF
    /// file descriptor with the rights specified by `self`.
    ///
    /// This passes `self` as the flags argument to the [`dma_buf_export_sync_file`]
    /// IOCTL. See there for more info on what this does, and how to use it.
    ///
    /// # Returns
    ///
    /// If `Ok(fd)`, we guarantee that the returned file descriptor is not `-1`.
    ///
    /// [`dma_buf_export_sync_file`]: https://docs.kernel.org/driver-api/dma-buf.html#c.dma_buf_export_sync_file
    #[inline(always)]
    pub fn export_sync_raw(
        self,
        dma_buf_fd: c_int,
    ) -> Result<c_int, Errno> {
        const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: c_ulong =
            libc::_IOWR::<DmaBufExportSyncFile>(DMA_BUF_BASE as u32, 2);

        #[repr(C)]
        struct DmaBufExportSyncFile {
            flags: u32,
            fd: i32,
        }

        let mut payload = DmaBufExportSyncFile {
            flags: self as u8 as u32,
            fd: -1,
        };

        // SAFETY: This IOCTL shouldn't really cause any unsoundness on its own.
        let result =
            unsafe { libc::ioctl(dma_buf_fd, DMA_BUF_IOCTL_EXPORT_SYNC_FILE, &raw mut payload) };

        match result {
            0.. => match c_int::try_from(payload.fd) {
                Ok(-1) | Err(..) => Err(Errno(libc::EBADF)),
                Ok(exported_fd) => Ok(exported_fd),
            },
            ..0 => Err(Errno::last()),
        }
    }

    /// Same as [`SyncRights::export_sync_raw`] but type-safe.
    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn export_sync(
        self,
        dma_buf: BorrowedFd<'_>,
    ) -> Result<OwnedFd, Errno> {
        self.export_sync_raw(dma_buf.as_raw_fd())
            // SAFETY: We know that `sync_fd` is a valid file descriptor.
            .map(|sync_fd| unsafe { OwnedFd::from_raw_fd(sync_fd) })
    }
}

impl ops::BitOr for SyncRights {
    type Output = SyncRights;

    #[inline(always)]
    fn bitor(
        self,
        rhs: Self,
    ) -> Self::Output {
        match SyncRights::from_bits(self as u8 | rhs as u8) {
            Some(rights) => rights,
            // SAFETY: The bitwise OR of two `SyncRights` values is always a valid `SyncRights` value.
            None => unsafe {
                unreachable_unchecked!(
                    "the bitwise or of two synchronization access rights flags failed"
                )
            },
        }
    }
}

impl ops::BitOrAssign for SyncRights {
    #[inline(always)]
    fn bitor_assign(
        &mut self,
        rhs: Self,
    ) {
        *self = *self | rhs;
    }
}

/// Represents the options we're using when synchronizing a DMA buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SyncOptions {
    /// We're starting a CPU synchronization operation.
    Start(SyncRights) = 0b000,
    /// We're ending a CPU synchronization operation.
    End(SyncRights) = 0b100,
}

impl SyncOptions {
    /// This turns the options specified by `self` into flags suitable
    /// for a [`dma_buf_sync`] IOCTL.
    ///
    /// It is assumed that when using the [`dma_buf_sync`] IOCTL, that the result
    /// of this method is then cast to a [`u64`].
    ///
    /// [`dma_buf_sync`]: https://docs.kernel.org/driver-api/dma-buf.html#c.dma_buf_sync
    #[inline(always)]
    #[must_use]
    pub const fn to_flags(self) -> u8 {
        use SyncOptions as O;

        // SAFETY: We have a `repr(u8)` enum, so we can read the discriminant.
        let kind = unsafe { (&raw const self).cast::<u8>().read() };
        let rights = match self {
            O::Start(rights) | O::End(rights) => rights as u8,
        };

        kind | rights
    }

    /// Synchronize the provided DMA-BUF file descriptor for CPU access
    /// with the options specified by `self`.
    ///
    /// This passes `self` as the flags argument for [`dma_buf_sync`] IOCTL. See
    /// there for more info on what this does.
    ///
    /// # Safety
    ///
    /// This should be sound for all usages, realistically. However much like atomics,
    /// it is the caller's responsibility to ensure that any other code that relies on
    /// this for soundness, is implemented correctly.
    ///
    /// [`dma_buf_sync`]: https://docs.kernel.org/driver-api/dma-buf.html#c.dma_buf_sync
    #[inline(always)]
    pub fn sync_raw(
        self,
        dma_buf_fd: c_int,
    ) -> Result<(), Errno> {
        const DMA_BUF_IOCTL_SYNC: c_ulong = libc::_IOW::<DmaBufSync>(DMA_BUF_BASE as u32, 0);

        #[repr(C)]
        struct DmaBufSync {
            flags: u64,
        }

        let mut payload = DmaBufSync {
            flags: self.to_flags() as u64,
        };

        // SAFETY: The `DMA_BUF_IOCTL_SYNC` shouldn't really cause any unsoundness on its own.
        let result = unsafe { libc::ioctl(dma_buf_fd, DMA_BUF_IOCTL_SYNC, &raw mut payload) };

        match result {
            0.. => Ok(()),
            ..0 => Err(Errno::last()),
        }
    }

    /// Same as [`SyncOptions::sync_raw`], but type-safe.
    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn sync(
        self,
        dma_buf_fd: BorrowedFd<'_>,
    ) -> Result<(), Errno> {
        self.sync_raw(dma_buf_fd.as_raw_fd())
    }
}

impl ops::BitOr for SyncOptions {
    type Output = SyncOptions;

    #[inline(always)]
    fn bitor(
        self,
        rhs: Self,
    ) -> Self::Output {
        use SyncOptions as O;

        let rights = match (self, rhs) {
            (O::Start(lhs) | O::End(lhs), O::Start(rhs) | O::End(rhs)) => lhs | rhs,
        };

        match (self, rhs) {
            (O::Start(..), O::Start(..)) => O::Start(rights),
            _ => O::End(rights),
        }
    }
}

impl ops::BitOrAssign for SyncOptions {
    #[inline(always)]
    fn bitor_assign(
        &mut self,
        rhs: Self,
    ) {
        *self = *self | rhs;
    }
}

impl ops::BitOr<SyncRights> for SyncOptions {
    type Output = SyncOptions;

    #[inline(always)]
    fn bitor(
        mut self,
        rhs: SyncRights,
    ) -> Self::Output {
        self |= rhs;

        self
    }
}

impl ops::BitOrAssign<SyncRights> for SyncOptions {
    #[inline(always)]
    fn bitor_assign(
        &mut self,
        rhs: SyncRights,
    ) {
        use SyncOptions as O;

        match self {
            O::Start(lhs) | O::End(lhs) => *lhs |= rhs,
        };
    }
}

// #[cfg(feature = "std")]
// pub struct DmaMapping<'a> {
//     rights: SyncRights,
//     dma_buf: BorrowedFd<'a>,
//     buffer: Non
// }
