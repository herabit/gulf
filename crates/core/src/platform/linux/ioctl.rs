// //! Helper interface for Linux IOCTLs.

// use core::{
//     ffi::{c_int, c_uint},
//     marker::PhantomData,
// };
// use std::{
//     ffi::c_ulong,
//     io,
//     mem::MaybeUninit,
//     os::fd::{BorrowedFd, RawFd},
//     ptr,
// };

// #[derive(Clone, Copy)]
// #[repr(C)]
// struct Constants {
//     number_bits: u32,
//     type_bits: u32,

//     size_bits: u32,
//     direction_bits: u32,

//     direction_none: u32,
//     direction_read: u32,
//     direction_write: u32,

//     number_mask: u32,
//     type_mask: u32,
//     size_mask: u32,
//     direction_mask: u32,

//     number_shift: u32,
//     type_shift: u32,
//     size_shift: u32,
//     direction_shift: u32,
// }

// impl Constants {
//     #[inline(always)]
//     const fn new() -> Constants {
//         let mut constants: Constants = unsafe { core::mem::zeroed() };

//         constants.number_bits = 8;
//         constants.type_bits = 8;

//         let is_powerpc = cfg!(target_arch = "powerpc") || cfg!(target_arch = "powerpc64");
//         let is_sparc = cfg!(target_arch = "sparc") || cfg!(target_arch = "sparc64");
//         let is_mips = cfg!(target_arch = "mips") || cfg!(target_arch = "mips64");

//         if is_powerpc || is_sparc || is_mips {
//             constants.size_bits = 13;
//             constants.direction_bits = 3;
//             constants.direction_none = 1;
//             constants.direction_read = 2;
//             constants.direction_write = 4;
//         } else {
//             constants.size_bits = 14;
//             constants.direction_bits = 2;
//             constants.direction_none = 0;
//             constants.direction_write = 1;
//             constants.direction_read = 2;
//         }

//         constants.number_mask = 1_u32.strict_shl(constants.number_bits).strict_sub(1);
//         constants.type_mask = 1_u32.strict_shl(constants.type_bits).strict_sub(1);
//         constants.size_mask = 1_u32.strict_shl(constants.size_bits).strict_sub(1);
//         constants.direction_mask = 1_u32.strict_shl(constants.direction_bits).strict_sub(1);

//         constants.number_shift = 0;
//         constants.type_shift = constants.number_shift.strict_add(constants.number_bits);
//         constants.size_shift = constants.type_shift.strict_add(constants.type_bits);
//         constants.direction_shift = constants.size_shift.strict_add(constants.size_bits);

//         constants
//     }
// }

// const CONSTANTS: Constants = Constants::new();

// /// The amount of bits used to store the IOCTL number.
// #[doc(alias = "_IOC_NRBITS")]
// #[doc(alias = "NRBITS")]
// #[doc(alias = "NR_BITS")]
// pub const NUMBER_BITS: u32 = CONSTANTS.number_bits;

// /// The amount of bits used to store the group.
// #[doc(alias = "_IOC_TYPEBITS")]
// #[doc(alias = "TYPEBITS")]
// #[doc(alias = "TYPE_BITS")]
// #[doc(alias = "GROUPBITS")]
// pub const GROUP_BITS: u32 = CONSTANTS.type_bits;

// /// The amount of bits used to store the size.
// #[doc(alias = "_IOC_SIZEBITS")]
// #[doc(alias = "SIZEBITS")]
// pub const SIZE_BITS: u32 = CONSTANTS.size_bits;

// /// The amount of bits used to store the direction.
// #[doc(alias = "_IOC_DIRBITS")]
// #[doc(alias = "DIRBITS")]
// #[doc(alias = "DIR_BITS")]
// pub const DIRECTION_BITS: u32 = CONSTANTS.direction_bits;

// /// The value of no direction.
// #[doc(alias = "_IOC_NONE")]
// #[doc(alias = "NONE")]
// #[doc(alias = "DIR_NONE")]
// #[doc(alias = "DIRNONE")]
// pub const DIRECTION_NONE: u32 = CONSTANTS.direction_none;

// /// The value of the read direction.
// #[doc(alias = "_IOC_READ")]
// #[doc(alias = "READ")]
// #[doc(alias = "DIR_READ")]
// #[doc(alias = "DIRREAD")]
// pub const DIRECTION_READ: u32 = CONSTANTS.direction_read;

// /// The value of the write direction.
// #[doc(alias = "_IOC_WRITE")]
// #[doc(alias = "WRITE")]
// #[doc(alias = "DIR_WRITE")]
// #[doc(alias = "DIRWRITE")]
// pub const DIRECTION_WRITE: u32 = CONSTANTS.direction_write;

// /// A mask for the number bits.
// #[doc(alias = "_IOC_NRMASK")]
// #[doc(alias = "NRMASK")]
// #[doc(alias = "NR_MASK")]
// pub const NUMBER_MASK: u32 = CONSTANTS.number_mask;

// /// A mask for the type bits.
// #[doc(alias = "_IOC_TYPEMASK")]
// #[doc(alias = "TYPEMASK")]
// #[doc(alias = "GROUPMASK")]
// pub const GROUP_MASK: u32 = CONSTANTS.type_mask;

// /// A mask for the size bits.
// #[doc(alias = "_IOC_SIZEMASK")]
// #[doc(alias = "SIZEMASK")]
// pub const SIZE_MASK: u32 = CONSTANTS.size_mask;

// /// A mask for the direction bits.
// #[doc(alias = "_IOC_DIRMASK")]
// #[doc(alias = "DIRMASK")]
// #[doc(alias = "DIR_MASK")]
// pub const DIRECTION_MASK: u32 = CONSTANTS.direction_mask;

// /// The bit offset of the number bits.
// #[doc(alias = "_IOC_NRSHIFT")]
// #[doc(alias = "NRSHIFT")]
// #[doc(alias = "NR_SHIFT")]
// pub const NUMBER_SHIFT: u32 = CONSTANTS.number_shift;

// /// The bit offset of the group bits.
// #[doc(alias = "_IOC_TYPESHIFT")]
// #[doc(alias = "TYPESHIFT")]
// #[doc(alias = "TYPE_SHIFT")]
// #[doc(alias = "GROUPSHIFT")]
// pub const GROUP_SHIFT: u32 = CONSTANTS.type_shift;

// /// The bit offset of the size bits.
// #[doc(alias = "_IOC_SIZESHIFT")]
// #[doc(alias = "SIZESHIFT")]
// pub const SIZE_SHIFT: u32 = CONSTANTS.size_shift;

// /// The bit offset of the direction bits.
// #[doc(alias = "_IOC_DIRSHIFT")]
// #[doc(alias = "DIRSHIFT")]
// #[doc(alias = "DIR_SHIFT")]
// pub const DIRECTION_SHIFT: u32 = CONSTANTS.direction_shift;

// const _: () = assert!(DIRECTION_BITS <= 8, "a direction cannot fit within a byte");

// /// An enum representing the possible ioctl directions.
// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
// #[non_exhaustive]
// #[repr(u8)]
// pub enum Direction {
//     /// Indicates an IOCTL does nothing with memory (default).
//     #[default]
//     None = DIRECTION_NONE as u8,
//     /// Indicates an IOCTL may read from memory.
//     Read = DIRECTION_READ as u8,
//     /// Indicates an IOCTL may write to memory.
//     Write = DIRECTION_WRITE as u8,
//     /// Indicates an IOCTL may read or write to memory.
//     ReadWrite = DIRECTION_READ as u8 | DIRECTION_WRITE as u8,
// }

// impl Direction {
//     /// Create a [`Direction`] based upon whether we want read/write perms.
//     #[inline(always)]
//     #[must_use]
//     pub const fn from_perms(
//         read: bool,
//         write: bool,
//     ) -> Direction {
//         match (read, write) {
//             (false, false) => Direction::None,
//             (true, false) => Direction::Read,
//             (false, true) => Direction::Write,
//             (true, true) => Direction::ReadWrite,
//         }
//     }

//     /// Get the perms associated with this direction as a `(read, write)` tuple.
//     #[inline(always)]
//     #[must_use]
//     pub const fn to_perms(self) -> (bool, bool) {
//         match self {
//             Direction::None => (false, false),
//             Direction::Read => (true, false),
//             Direction::Write => (false, true),
//             Direction::ReadWrite => (true, true),
//         }
//     }

//     /// Returns whether this direction can read.
//     #[inline(always)]
//     #[must_use]
//     pub const fn can_read(self) -> bool {
//         matches!(self, Direction::Read | Direction::ReadWrite)
//     }

//     /// Returns whether this direction can write.
//     #[inline(always)]
//     #[must_use]
//     pub const fn can_write(self) -> bool {
//         matches!(self, Direction::Write | Direction::ReadWrite)
//     }

//     /// Returns whether this direction is [`Direction::None`].
//     #[inline(always)]
//     #[must_use]
//     pub const fn is_none(self) -> bool {
//         matches!(self, Direction::None)
//     }

//     /// Returns whether this direction is [`Direction::Read`].
//     #[inline(always)]
//     #[must_use]
//     pub const fn is_read(self) -> bool {
//         matches!(self, Direction::Read)
//     }

//     /// Returns whether this direction is [`Direction::Write`].
//     #[inline(always)]
//     #[must_use]
//     pub const fn is_write(self) -> bool {
//         matches!(self, Direction::Write)
//     }

//     /// Returns whether this direction is [`Direction::ReadWrite`].
//     #[inline(always)]
//     #[must_use]
//     pub const fn is_read_write(self) -> bool {
//         matches!(self, Direction::ReadWrite)
//     }
// }

// mod sealed {
//     pub struct Token(pub(crate) ());
//     pub trait Dir {}
// }

// /// A trait for statically known directions.
// pub unsafe trait Dir: sealed::Dir {
//     /// The type that this direction requires access to.
//     type Input: Sized;

//     /// The type that is returned after the operation.
//     type Output: Sized;

//     /// The actual direction this points to.
//     const DIRECTION: Direction;

//     #[track_caller]
//     unsafe fn ioctl(
//         _: sealed::Token,
//         request: u32,
//         fd: RawFd,
//         input: Self::Input,
//     ) -> io::Result<Self::Output>;
// }

// #[inline(always)]
// fn get_request<T>(request: u32) -> io::Result<T>
// where
//     T: TryFrom<u32>,
// {
//     request
//         .try_into()
//         .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "could not cast request type"))
// }

// /// A type-level representation of [`Direction::None`].
// #[derive(Debug, Clone, Copy, Default)]
// pub struct None;

// impl sealed::Dir for None {}
// unsafe impl Dir for None {
//     type Input = ();
//     type Output = c_uint;

//     const DIRECTION: Direction = Direction::None;

//     #[inline(always)]
//     unsafe fn ioctl(
//         _: sealed::Token,
//         request: u32,
//         fd: RawFd,
//         (): (),
//     ) -> io::Result<c_uint> {
//         let request = get_request(request)?;

//         // SAFETY: The caller ensures it is safe to use this IOCTL in this way.
//         unsafe { libc::ioctl(fd, request, ptr::null_mut::<()>()) }
//             .try_into()
//             .map_err(|_| io::Error::last_os_error())
//     }
// }

// /// A type-level representation of [`Direction::Read`].
// #[derive(Debug)]
// pub struct Read<I>(PhantomData<I>);

// impl<I> Read<I> {
//     /// Create a new [`Read`].
//     #[inline(always)]
//     pub const fn new() -> Read<I> {
//         Read(PhantomData)
//     }
// }

// impl<I: Clone> Clone for Read<I> {
//     #[inline(always)]
//     fn clone(&self) -> Self {
//         Read(PhantomData)
//     }
// }

// impl<I: Copy> Copy for Read<I> {}

// impl<I> Default for Read<I> {
//     #[inline(always)]
//     fn default() -> Self {
//         Read::new()
//     }
// }

// const _: () = assert!(usize::BITS <= c_ulong::BITS);

// impl sealed::Dir for Read<()> {}
// unsafe impl Dir for Read<()> {
//     type Input = ();
//     type Output = c_uint;

//     const DIRECTION: Direction = Direction::Read;

//     #[inline(always)]
//     unsafe fn ioctl(
//         _: sealed::Token,
//         request: u32,
//         fd: RawFd,
//         (): (),
//     ) -> io::Result<c_uint> {
//         let request = get_request(request)?;

//         // SAFETY: We're reading nothing, so passing along a nulls should be fine... Should.
//         unsafe { libc::ioctl(fd, request, ptr::null_mut::<()>()) }
//             .try_into()
//             .map_err(|_| io::Error::last_os_error())
//     }
// }

// impl<'a, T> sealed::Dir for Read<&'a T> {}
// unsafe impl<'a, T> Dir for Read<&'a T> {
//     type Input = ();
//     type Output = (T, c_uint);

//     const DIRECTION: Direction = Direction::Read;

//     #[inline(always)]
//     unsafe fn ioctl(
//         _: sealed::Token,
//         request: u32,
//         fd: RawFd,
//         (): (),
//     ) -> io::Result<(T, c_uint)> {
//         let request = get_request(request)?;
//         let mut data = MaybeUninit::uninit();

//         // SAFETY: We're assuming that the
//         unsafe { libc::ioctl(fd, request, data.as_mut_ptr()) }
//             .try_into()
//             .map_err(|_| io::Error::last_os_error())
//             .map(|return_code| (unsafe { data.assume_init() }, return_code))
//     }
// }

// /// A type-level representation of [`Direction::Write`].
// #[derive(Debug)]
// pub struct Write<I>(PhantomData<I>);

// impl<I> Write<I> {
//     /// Create a new [`Write`].
//     #[inline(always)]
//     pub const fn new() -> Write<I> {
//         Write(PhantomData)
//     }
// }

// impl<I: Clone> Clone for Write<I> {
//     #[inline(always)]
//     fn clone(&self) -> Self {
//         Self(PhantomData)
//     }
// }

// impl<I: Copy> Copy for Write<I> {}

// impl<I> Default for Write<I> {
//     #[inline(always)]
//     fn default() -> Self {
//         Write::new()
//     }
// }

// impl sealed::Dir for Write<()> {}
// unsafe impl Dir for Write<()> {
//     type Input = ();
//     type Output = c_uint;

//     const DIRECTION: Direction = Direction::Write;

//     #[inline(always)]
//     unsafe fn ioctl(
//         _: sealed::Token,
//         request: u32,
//         fd: RawFd,
//         (): (),
//     ) -> io::Result<c_uint> {
//         let request = get_request(request)?;

//         // SAFETY: The caller ensures it is safe to use this IOCTL in this way.
//         unsafe { libc::ioctl(fd, request, ptr::null_mut::<()>()) }
//             .try_into()
//             .map_err(|_| io::Error::last_os_error())
//     }
// }

// // impl<'a, T> sealed::Dir for Write<&'a mut T> {}
// // unsafe impl<'a, T> Dir for Write<&'a mut T> {
// //     type Input = ;
// //     type Output;

// //     const DIRECTION: Direction;

// //     unsafe fn ioctl(
// //         _: sealed::Token,
// //         request: u32,
// //         fd: RawFd,
// //         input: Self::Input,
// //     ) -> io::Result<Self::Output> {
// //         todo!()
// //     }
// // }

// // // impl sealed::Dir for Write<usize> {}

// // // unsafe impl Dir for Write<usize> {
// // //     type Input = usize;
// // //     type Output = c_uint;

// // //     const DIRECTION: Direction = Direction::Write;

// // //     #[inline(always)]
// // //     unsafe fn ioctl(
// // //         _: sealed::Token,
// // //         request: u32,
// // //         fd: RawFd,
// // //         input: usize,
// // //     ) -> io::Result<Self::Output> {
// // //         let request = get_request(request)?;

// // //         // SAFETY: The caller ensures it is safe to use this IOCTL in this way.
// // //         unsafe { libc::ioctl(fd, request, input) }
// // //             .try_into()
// // //             .map_err(|_| io::Error::last_os_error())
// // //     }
// // // }

// // // #[cfg(test)]
// // // #[test]
// // // fn fuck() {
// // //     let file = std::fs::File::open("/dev/kvm").unwrap();

// // //     let val = unsafe {
// // //         use std::os::fd::AsRawFd;
// // //         None::ioctl(
// // //             sealed::Token(()),
// // //             ::libc::_IO(0xAE, 0xFF) as _,
// // //             file.as_raw_fd(),
// // //             (),
// // //         )
// // //     }
// // //     .unwrap();

// // //     println!("{val}");
// // // }
