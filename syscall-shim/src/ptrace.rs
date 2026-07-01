//! ptrace-based Darwin syscall interception for a traced ARM64 guest.
//!
//! **Build-gated to `aarch64`, and functionally unverified in this
//! repository's development environment**, which is x86_64 (see
//! `../../docs/ROADMAP.md` Phase 1 notes) — there is no ARM64 hardware or
//! emulator available here to actually run a traced guest against. The
//! logic below follows the standard ptrace syscall-virtualization pattern
//! (the same one gVisor's ptrace platform and User-Mode Linux use): catch
//! the guest at the kernel-syscall-entry trap, rewrite its registers to
//! request the *translated* Linux syscall instead, let the real kernel
//! execute it in the guest's own context, then fix up the return value on
//! the matching exit-trap. It has not been exercised on real aarch64
//! hardware yet; treat it as reviewed-but-unverified until it has.
//!
//! Why intercept raw `svc` traps at all, instead of only replacing
//! `libSystem` symbols at link time (`runtime-shim`, a later phase)? A
//! binary can still issue a bare `svc #0x80` directly (as our own
//! Phase 1 test fixture does — see `tools/mkfixture`), and even once
//! `libSystem` is shimmed, *its own* implementation is exactly what would
//! issue the raw trap. This loop is the floor everything else sits on.

#![cfg(target_arch = "aarch64")]

use crate::translate::{self, LinuxSyscall, SyscallRequest, TranslateError};
use std::io;

/// Mirrors the kernel's `struct user_pt_regs` (`NT_PRSTATUS` on arm64):
/// 31 general registers, `sp`, `pc`, `pstate`. This is the Linux ptrace
/// ABI, not anything Apple-derived.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct UserPtRegs {
    pub regs: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u64,
}

const PSTATE_CARRY_BIT: u64 = 1 << 29;

impl UserPtRegs {
    /// `x16` — the Darwin convention register for the syscall number.
    pub fn syscall_number(&self) -> i64 {
        self.regs[16] as i64
    }

    pub fn syscall_args(&self) -> [u64; 6] {
        [
            self.regs[0],
            self.regs[1],
            self.regs[2],
            self.regs[3],
            self.regs[4],
            self.regs[5],
        ]
    }

    /// Rewrites this register set in place to request `linux_call` via
    /// Linux's own convention (syscall number in `x8`, args in `x0..x5`)
    /// instead of whatever Darwin syscall was originally requested.
    pub fn rewrite_for_linux(&mut self, linux_call: &LinuxSyscall) {
        self.regs[8] = linux_call.number as u64;
        self.regs[0..6].copy_from_slice(&linux_call.args);
    }

    /// Applies Darwin's carry-flag return convention after the real
    /// (Linux) syscall has run and its raw `x0` result has been read back.
    pub fn apply_darwin_return(&mut self, linux_x0: i64) {
        let (value, carry) = translate::to_darwin_return(linux_x0);
        self.regs[0] = value;
        if carry {
            self.pstate |= PSTATE_CARRY_BIT;
        } else {
            self.pstate &= !PSTATE_CARRY_BIT;
        }
    }
}

pub type Pid = libc::pid_t;

/// ELF core note type for general-purpose registers. Standard across
/// Linux/Android (`<linux/elf.h>`); the `libc` crate doesn't expose it on
/// every target, so it's inlined here.
const NT_PRSTATUS: i32 = 1;

fn getregset(pid: Pid) -> io::Result<UserPtRegs> {
    let mut regs = UserPtRegs::default();
    let mut iov = libc::iovec {
        iov_base: &mut regs as *mut _ as *mut libc::c_void,
        iov_len: std::mem::size_of::<UserPtRegs>(),
    };
    let ret = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGSET,
            pid,
            NT_PRSTATUS as *mut libc::c_void,
            &mut iov as *mut _ as *mut libc::c_void,
        )
    };
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(regs)
}

fn setregset(pid: Pid, regs: &UserPtRegs) -> io::Result<()> {
    let mut regs = *regs;
    let mut iov = libc::iovec {
        iov_base: &mut regs as *mut _ as *mut libc::c_void,
        iov_len: std::mem::size_of::<UserPtRegs>(),
    };
    let ret = unsafe {
        libc::ptrace(
            libc::PTRACE_SETREGSET,
            pid,
            NT_PRSTATUS as *mut libc::c_void,
            &mut iov as *mut _ as *mut libc::c_void,
        )
    };
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// One iteration: assumes `pid` is stopped at a syscall-entry trap
/// (reached via repeated `PTRACE_SYSCALL`). Reads the Darwin syscall
/// request, translates it, rewrites registers so the kernel runs the
/// Linux equivalent in the guest's own context, single-steps past the
/// exit trap, and fixes up the return value/carry flag.
///
/// Returns the original Darwin request (for logging/policy decisions by
/// the caller) plus the translation outcome.
pub unsafe fn intercept_one(pid: Pid) -> io::Result<Result<SyscallRequest, TranslateError>> {
    let mut regs = getregset(pid)?;
    let request = SyscallRequest {
        number: regs.syscall_number(),
        args: regs.syscall_args(),
    };

    let translated = match translate::to_linux(&request) {
        Ok(call) => call,
        Err(e) => return Ok(Err(e)),
    };

    regs.rewrite_for_linux(&translated);
    setregset(pid, &regs)?;

    if libc::ptrace(libc::PTRACE_SYSCALL, pid, 0, 0) == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut status = 0;
    if libc::waitpid(pid, &mut status, 0) == -1 {
        return Err(io::Error::last_os_error());
    }

    let mut post = getregset(pid)?;
    let linux_x0 = post.regs[0] as i64;
    post.apply_darwin_return(linux_x0);
    setregset(pid, &post)?;

    Ok(Ok(request))
}
