//! Darwin/XNU BSD syscall numbers and `mmap`/`mprotect` flag bits.
//!
//! These are public ABI constants — the same numbers any interoperability
//! tool (or `strace`-equivalent) would need — documented in Apple's own
//! open-source (APSL) `xnu` headers (`bsd/kern/syscalls.master`,
//! `bsd/sys/mman.h`). No XNU implementation code is used; see
//! `../../docs/LEGAL.md`.

/// Terminate the calling thread (BSD `exit`).
pub const SYS_EXIT: i64 = 1;
pub const SYS_READ: i64 = 3;
pub const SYS_WRITE: i64 = 4;
pub const SYS_OPEN: i64 = 5;
pub const SYS_CLOSE: i64 = 6;
pub const SYS_MUNMAP: i64 = 73;
pub const SYS_MPROTECT: i64 = 74;
pub const SYS_MMAP: i64 = 197;

/// `vm_prot_t` bits — numerically identical to Linux's `PROT_*`, but kept
/// as separate constants so that congruence is asserted (see
/// `translate::tests`) rather than silently assumed.
pub const PROT_READ: u32 = 0x01;
pub const PROT_WRITE: u32 = 0x02;
pub const PROT_EXEC: u32 = 0x04;

/// `mmap` flags from Darwin's `<sys/mman.h>`. Several of these have
/// *different* bit values than Linux's `MAP_*` flags of the same name
/// (`MAP_ANON` in particular: `0x1000` on Darwin vs. `0x20` on Linux) —
/// this is exactly the kind of mismatch a naive pass-through would get
/// wrong. See `translate::to_linux`.
pub const MAP_SHARED: u32 = 0x0001;
pub const MAP_PRIVATE: u32 = 0x0002;
pub const MAP_FIXED: u32 = 0x0010;
pub const MAP_ANON: u32 = 0x1000;
