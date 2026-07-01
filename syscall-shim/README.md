# syscall-shim

Intercepts Darwin/XNU syscalls issued by a loaded Mach-O binary and
re-implements their observable behavior on top of Linux/Android syscalls.
This is the layer that stands in for "the kernel" from the binary's point
of view — a userspace translator over the real Android kernel, not a
second kernel or a copy of XNU.

- `darwin.rs` — Darwin BSD syscall numbers and `mmap`/`mprotect` flag/prot
  bit constants (public ABI values, see `../docs/LEGAL.md`).
- `translate.rs` — pure Darwin-syscall → Linux-syscall translation (no
  side effects, no ptrace needed to test it; `cargo test -p
  iosonandroid-syscall-shim`). Covers `exit`, `write`, `mmap` (including
  correct `MAP_ANON` bit translation — Darwin's `0x1000` vs. Linux's
  `0x20`, a real mismatch a naive pass-through would get wrong),
  `mprotect`.
- `ptrace.rs` (aarch64-only) — the interception loop: catches a traced
  guest at its syscall-entry trap, rewrites registers to the translated
  Linux syscall so the *real* kernel executes it in the guest's own
  address space/fd table, then fixes up the return value into Darwin's
  carry-flag convention. **Cross-compiles clean for
  `aarch64-linux-android` but is unverified on real hardware** — this
  repo's dev environment is x86_64 with no ARM64 execution available.

Status: Phase 1, translation logic done + unit tested; ptrace loop written
but device-untested. See [Phase 1-2](../docs/ROADMAP.md) and
[Architecture §2](../docs/ARCHITECTURE.md#2-syscall-shim-syscall-shim).
