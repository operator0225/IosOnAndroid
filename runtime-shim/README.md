# runtime-shim

dyld-compatible symbol resolver, clean-room Objective-C runtime
(`objc_msgSend`, class/method tables, ARC), and a minimal `libSystem`
subset backed by Android's bionic libc.

- `registry.rs` — `Registry`/`resolve()`: matches a `loader::MachOImage`'s
  imports against a table of shim symbol addresses, reporting anything
  unresolved instead of guessing. `cargo test -p iosonandroid-runtime-shim`.
- `sync.rs` — `FutexMutex`, a real futex(2)-backed mutex (the standard
  Drepper two/three-state algorithm), one piece of the Darwin
  threading-primitive surface. Arch-independent, verified with real
  concurrent `std::thread`s on host.

Not yet implemented: the Objective-C runtime (`objc_msgSend`, class/method
tables, ARC), the `libSystem` subset itself (the registry has no shim
symbols registered in it yet — that's what Phase 3 populates it with), and
actual Darwin thread creation (`bsdthread_create`).

Status: Phase 2, symbol resolution + one threading primitive done + unit
tested. See [Phase 2-3](../docs/ROADMAP.md) and
[Architecture §3](../docs/ARCHITECTURE.md#3-runtime-shim-runtime-shim).
