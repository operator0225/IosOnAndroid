//! Darwin syscall → Linux syscall translation.
//!
//! This module has no side effects and performs no syscalls itself — it
//! only computes *what Linux syscall (number + arguments) to run instead*.
//! That's a deliberate design choice, not just a testing convenience: the
//! syscalls that matter here (`mmap`, `mprotect`, `write` against an
//! inherited fd, ...) are only correct if they execute *in the traced
//! guest process's own context* (its own address space, its own fd
//! table). A tracer running them on its own behalf instead would mmap
//! into the wrong process and silently corrupt the emulation. So the
//! translated `LinuxSyscall` this module produces is meant to be POKEd
//! into the guest's registers and let the real kernel execute it for the
//! guest — see `ptrace::intercept_and_translate` — not executed here.
//!
//! Because this module is pure data translation, it's fully testable on
//! any host/arch, with no ptrace or ARM64 execution involved.

use crate::darwin;

/// A syscall request as read out of a traced ARM64 process's registers:
/// `x16` (the Darwin convention for the syscall number) and `x0..x5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallRequest {
    pub number: i64,
    pub args: [u64; 6],
}

/// What to actually ask the Linux kernel to do: the Linux syscall number
/// (`x8` in Linux's arm64 convention) and its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxSyscall {
    pub number: i64,
    pub args: [u64; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateError {
    Unsupported(i64),
}

pub fn to_linux(req: &SyscallRequest) -> Result<LinuxSyscall, TranslateError> {
    let mut args = req.args;
    let number = match req.number {
        darwin::SYS_EXIT => libc::SYS_exit,
        darwin::SYS_WRITE => libc::SYS_write,
        darwin::SYS_MPROTECT => {
            // Darwin's vm_prot_t bits (darwin::PROT_*) are numerically
            // identical to Linux's PROT_*; see tests::mprotect_prot_bits_match.
            libc::SYS_mprotect
        }
        darwin::SYS_MMAP => {
            args[3] = translate_mmap_flags(args[3] as u32) as u64;
            if args[3] as i32 & libc::MAP_ANONYMOUS != 0 {
                // Defensive normalization: Darwin requires fd == -1 for
                // anonymous mappings too, but don't trust a guest to have
                // gotten that right.
                args[4] = i64::from(-1i32) as u64;
            }
            libc::SYS_mmap
        }
        other => return Err(TranslateError::Unsupported(other)),
    };
    Ok(LinuxSyscall { number, args })
}

fn translate_mmap_flags(darwin_flags: u32) -> i32 {
    let mut linux = 0;
    if darwin_flags & darwin::MAP_SHARED != 0 {
        linux |= libc::MAP_SHARED;
    }
    if darwin_flags & darwin::MAP_PRIVATE != 0 {
        linux |= libc::MAP_PRIVATE;
    }
    if darwin_flags & darwin::MAP_FIXED != 0 {
        linux |= libc::MAP_FIXED;
    }
    if darwin_flags & darwin::MAP_ANON != 0 {
        linux |= libc::MAP_ANONYMOUS;
    }
    linux
}

/// Converts a raw Linux syscall return value (negative = `-errno`) into
/// Darwin's arm64 return convention: on success `x0` holds the result and
/// the carry flag is clear; on error `x0` holds the positive `errno` and
/// the carry flag is set. Returns `(x0_value, carry_set)`.
pub fn to_darwin_return(linux_ret: i64) -> (u64, bool) {
    if linux_ret < 0 {
        ((-linux_ret) as u64, true)
    } else {
        (linux_ret as u64, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(number: i64, args: [u64; 6]) -> SyscallRequest {
        SyscallRequest { number, args }
    }

    #[test]
    fn mprotect_prot_bits_match_linux() {
        assert_eq!(darwin::PROT_READ as i32, libc::PROT_READ);
        assert_eq!(darwin::PROT_WRITE as i32, libc::PROT_WRITE);
        assert_eq!(darwin::PROT_EXEC as i32, libc::PROT_EXEC);
    }

    #[test]
    fn translates_exit() {
        let out = to_linux(&req(darwin::SYS_EXIT, [42, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!(out.number, libc::SYS_exit);
        assert_eq!(out.args[0], 42);
    }

    #[test]
    fn translates_write_args_passthrough() {
        let args = [1, 0xdead_beef, 3, 0, 0, 0];
        let out = to_linux(&req(darwin::SYS_WRITE, args)).unwrap();
        assert_eq!(out.number, libc::SYS_write);
        assert_eq!(out.args, args);
    }

    #[test]
    fn translates_mprotect_args_passthrough() {
        let args = [0x1_0000, 0x1000, darwin::PROT_READ as u64, 0, 0, 0];
        let out = to_linux(&req(darwin::SYS_MPROTECT, args)).unwrap();
        assert_eq!(out.number, libc::SYS_mprotect);
        assert_eq!(out.args, args);
    }

    #[test]
    fn translates_mmap_anon_private_flag_bits() {
        let darwin_flags = darwin::MAP_PRIVATE | darwin::MAP_ANON;
        // Prove this genuinely differs bit-for-bit from a naive pass-through.
        assert_ne!(darwin_flags as i32, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS);

        let args = [
            0,
            0x1000,
            darwin::PROT_READ as u64,
            darwin_flags as u64,
            5,
            0,
        ];
        let out = to_linux(&req(darwin::SYS_MMAP, args)).unwrap();
        assert_eq!(out.number, libc::SYS_mmap);
        assert_eq!(out.args[3] as i32, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS);
        // fd forced to -1 for an anonymous mapping regardless of what the
        // guest passed.
        assert_eq!(out.args[4] as i64 as i32, -1);
    }

    #[test]
    fn translates_mmap_shared_file_backed_flag_bits() {
        let darwin_flags = darwin::MAP_SHARED;
        let args = [
            0,
            0x1000,
            darwin::PROT_READ as u64,
            darwin_flags as u64,
            7,
            0x2000,
        ];
        let out = to_linux(&req(darwin::SYS_MMAP, args)).unwrap();
        assert_eq!(out.args[3] as i32, libc::MAP_SHARED);
        assert_eq!(out.args[4], 7); // fd left alone for file-backed mappings
        assert_eq!(out.args[5], 0x2000);
    }

    #[test]
    fn unsupported_syscall_number_is_reported_not_guessed() {
        let err = to_linux(&req(9999, [0; 6])).unwrap_err();
        assert_eq!(err, TranslateError::Unsupported(9999));
    }

    #[test]
    fn darwin_return_convention_success() {
        assert_eq!(to_darwin_return(3), (3, false));
        assert_eq!(to_darwin_return(0), (0, false));
    }

    #[test]
    fn darwin_return_convention_error() {
        // Linux write() returning -EBADF (-9)
        assert_eq!(to_darwin_return(-9), (9, true));
    }
}
