//! Call raw syscalls on `x86`.

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
            asm!(
                "int 0x80",
                inlateout("eax") number.as_usize() => ret_value,
                options(nostack, preserves_flags),
            )
        },
        1 => unsafe {
            asm!(
                "int 0x80",
                inlateout("eax") number.as_usize() => ret_value,
                in("ebx") args[0].as_usize(),
                options(nostack, preserves_flags),
            )
        },
        2 => unsafe {
            asm!(
                "int 0x80",
                inlateout("eax") number.as_usize() => ret_value,
                in("ebx") args[0].as_usize(),
                in("ecx") args[1].as_usize(),
                options(nostack, preserves_flags),
            )
        },
        3 => unsafe {
            asm!(
                "int 0x80",
                inlateout("eax") number.as_usize() => ret_value,
                in("ebx") args[0].as_usize(),
                in("ecx") args[1].as_usize(),
                in("edx") args[2].as_usize(),
                options(nostack, preserves_flags),
            )
        },
        4 => unsafe {
            asm!(
                "xchg esi, {a_3}",
                "int 0x80",
                "xchg esi, {a_3}", // FIXME: LLVM is being a bitch.
                inlateout("eax") number.as_usize() => ret_value,
                in("ebx") args[0].as_usize(),
                in("ecx") args[1].as_usize(),
                in("edx") args[2].as_usize(),
                a_3 = in(reg) args[3].as_usize(),
                options(nostack, preserves_flags),
            )
        },
        5 => unsafe {
            asm!(
                "xchg esi, {a_3}",
                "int 0x80",
                "xchg esi, {a_3}", // FIXME: LLVM is being a bitch.
                inlateout("eax") number.as_usize() => ret_value,
                in("ebx") args[0].as_usize(),
                in("ecx") args[1].as_usize(),
                in("edx") args[2].as_usize(),
                a_3 = in(reg) args[3].as_usize(),
                in("edi") args[4].as_usize(),
                options(nostack, preserves_flags),
            )
        },
        6 => unsafe {
            asm!(
                "push ebp",
                "push esi",
                "mov ebp, dword ptr [eax + 8]",
                "mov esi, dword ptr [eax + 4]",
                "mov eax, dword ptr [eax]",
                "int 0x80",
                "pop esi",
                "pop ebp",
                // FIXME: Maybe use lateinout?
                inout("eax") &[
                    number.as_usize(),
                    args[3].as_usize(),
                    args[5].as_usize(),
                ] => ret_value,
                in("ebx") args[0].as_usize(),
                in("ecx") args[1].as_usize(),
                in("edx") args[2].as_usize(),
                in("edi") args[4].as_usize(),

                options(preserves_flags),
            )
        },
        count => panic!("unsupported argument length: {count}"),
    }

    SysResult {
        error_flag: None,
        values: [ret_value.into()],
    }
}

#[unsafe(no_mangle)]
pub unsafe fn run(
    x: SysWord,
    b: [SysWord; 6],
) -> SysWord {
    unsafe { syscall(x, b) }.values[0]
}

#[cfg(test)]
#[test]
pub fn fuck() {
    unsafe {
        syscall(
            SysWord::try_from(libc::SYS_epoll_wait).unwrap(),
            [SysWord::from_usize(0); 6],
        )
    }
    .values[0]
        .as_error()
        .unwrap();
}
