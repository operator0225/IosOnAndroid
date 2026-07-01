# runtime-shim

dyld-compatible symbol resolver, a clean-room Objective-C object/dispatch
model, ARC refcounting, and a minimal `libSystem` subset backed by
Android's bionic libc.

- `registry.rs` — `Registry`/`resolve()`: matches a `loader::MachOImage`'s
  imports against a table of shim symbol addresses, reporting anything
  unresolved instead of guessing.
- `sync.rs` — `FutexMutex`, a real futex(2)-backed mutex. Arch-independent,
  verified with real concurrent `std::thread`s on host.
- `objc.rs` — `Class`/`Sel`/`Imp`/`Object` and `objc_msg_send`: the
  *dispatch logic* of Objective-C messaging (superclass-chain method
  lookup, override shadowing, ivar offset layout, and nil-messaging
  semantics), independent of any particular calling convention.
- `arc.rs` — `RefCounted`: retain/release/dealloc-signal bookkeeping, with
  over-release reported rather than left as undefined behavior.
- `libsystem.rs` — `_malloc`/`_free`/`_memcpy`/`_strlen` trampolines to
  bionic, registered into `Registry` under their Darwin symbol names.

`cargo test -p iosonandroid-runtime-shim` (23 tests).

Not yet implemented: the real ARM64 `objc_msgSend` calling-convention
trampoline (needs ARM64 execution to verify — same caveat as
`syscall-shim::ptrace`), parsing actual classes out of a Mach-O's
`__objc_classlist`/`__objc_methname` sections, the rest of libSystem's
symbol surface, and actual Darwin thread creation (`bsdthread_create`).

Status: Phase 3, core data models done + unit tested. See
[Phase 2-3](../docs/ROADMAP.md) and
[Architecture §3](../docs/ARCHITECTURE.md#3-runtime-shim-runtime-shim).
