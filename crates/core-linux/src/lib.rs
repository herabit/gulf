//! Linux specific core utilities.
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

const _: () = assert!(
    size_of::<usize>() == size_of::<core::ffi::c_ulong>(),
    "`c_ulong` is not a `usize`",
);
const _: () = assert!(
    size_of::<isize>() == size_of::<core::ffi::c_long>(),
    "`c_long` is not an `isize`",
);

const _: () = assert!(
    u32::BITS <= usize::BITS,
    "we only support 32-bit platforms and above",
);

pub mod errno;
pub mod syscall;
