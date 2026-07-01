//! Darwin/XNU syscall table and translation to Linux syscalls.
//!
//! See `translate` for the pure, fully host-testable translation logic,
//! and `ptrace` (aarch64-only) for the guest-interception loop that uses
//! it. See `../docs/ARCHITECTURE.md` §2 for how this fits the overall
//! stack, and `../docs/LEGAL.md` for why this is a translation shim over
//! the real Linux/Android kernel rather than an XNU reimplementation.

pub mod darwin;
pub mod translate;

#[cfg(target_arch = "aarch64")]
pub mod ptrace;

pub use translate::{to_darwin_return, LinuxSyscall, SyscallRequest, TranslateError};
