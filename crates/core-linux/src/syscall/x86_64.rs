//! Call raw syscalls on `x86_64`.

use crate::syscall::{SysResult, SysWord};

use core::arch::asm;

#[inline(always)]
#[track_caller]
#[must_use]
pub unsafe fn syscall<const N: usize>(
    number: SysWord,
    args: [SysWord; N],
) -> SysResult {
    let mut ret_value: usize;

    match N {
        0 => unsafe {
            asm! (
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        1 => unsafe {
            asm! (
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags)
            )
        },
        2 => unsafe {
            asm!(
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                in("rsi") args[1].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        3 => unsafe {
            asm!(
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                in("rsi") args[1].as_usize(),
                in("rdx") args[2].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        4 => unsafe {
            asm!(
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                in("rsi") args[1].as_usize(),
                in("rdx") args[2].as_usize(),
                in("r10") args[3].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        5 => unsafe {
            asm!(
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                in("rsi") args[1].as_usize(),
                in("rdx") args[2].as_usize(),
                in("r10") args[3].as_usize(),
                in("r8") args[4].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        6 => unsafe {
            asm!(
                "syscall",
                inlateout("rax") number.as_usize() => ret_value,
                in("rdi") args[0].as_usize(),
                in("rsi") args[1].as_usize(),
                in("rdx") args[2].as_usize(),
                in("r10") args[3].as_usize(),
                in("r8") args[4].as_usize(),
                in("r9") args[5].as_usize(),
                out("rcx") _,
                out("r11") _,
                options(nostack, preserves_flags),
            )
        },
        count => panic!("unsupported argument length: {count}"),
    }

    SysResult {
        error_flag: None,
        values: [ret_value.into()],
    }
}
