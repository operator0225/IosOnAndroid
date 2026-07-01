# IosOnAndroid

A research project exploring iOS app compatibility on Android via a
**userspace binary-compatibility layer**, not a virtual machine and not a
kernel emulator.

## Why this approach, not "full iOS emulation"

A phone like a Galaxy S25+ and an iPhone both run **ARM64**. That single
fact changes the whole design space:

- We do **not** need to emulate the ARM64 instruction set (unlike x86-on-ARM
  projects). Native code can run at native speed.
- What differs between iOS and Android is everything *above* the CPU: the
  Mach-O binary format, the Darwin/XNU system call ABI, the dyld dynamic
  linker, the Objective-C/Swift runtime, and the entire framework stack
  (Foundation, UIKit, Metal, ...).
- So the real engineering problem is a **compatibility shim**, the same
  class of project as [Darling](https://www.darlinghq.org/) (macOS binaries
  on Linux), [Wine](https://www.winehq.org/) (Windows binaries on Linux), or
  [Proton](https://github.com/ValveSoftware/Proton). None of those projects
  emulate a kernel or ship the target OS's proprietary kernel/frameworks —
  they reimplement the ABI surface a userspace binary depends on.

"완벽한 커널 에뮬레이션" (perfect kernel emulation) and running Apple's
actual XNU kernel/IPSW firmware is explicitly **out of scope** — see
[docs/LEGAL.md](docs/LEGAL.md) for why.

## What this project is

A layered translation stack:

1. **Loader** — parses and loads ARM64 Mach-O binaries.
2. **Syscall shim** — traps Darwin/XNU syscalls issued by the loaded binary
   and re-implements their behavior on top of Linux/Android syscalls.
3. **Runtime shim** — a clean-room, from-scratch implementation of enough of
   `libSystem`, the Objective-C runtime, and dyld's dynamic-linking contract
   for real binaries to start up.
4. **Graphics translation** — a Metal → Vulkan call-and-shader translation
   layer, so rendering commands issued by an iOS binary become Vulkan
   commands Android's GPU driver actually understands.
5. **Framework shims** — incremental, clean-room reimplementations of the
   framework surface (Foundation first, then a UIKit subset) that apps
   actually call.
6. **Android host runtime** — the Android-side app/process that hosts a
   loaded binary, presents its UI via a Vulkan-backed `Surface`, and bridges
   input/lifecycle events.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the detailed design and
[docs/ROADMAP.md](docs/ROADMAP.md) for the phased build plan. Status:
**Phase 3 — Objective-C runtime & minimal libSystem**, with Phases 1-2
mostly but not fully done. Implemented and unit tested
(`cargo test --workspace`, 50 tests): the Mach-O parser (segments, entry
point, symbol table, dylib names), Darwin→Linux syscall translation,
dyld-style symbol resolution against a shim registry, a futex-backed
mutex, an Objective-C class/method/ivar/dispatch model (including
correct nil-messaging semantics), ARC retain/release bookkeeping, and a
four-function `libSystem` subset over bionic. **Not yet verified on real
hardware**: the aarch64 ptrace guest-interception loop and the real
ARM64 `objc_msgSend` calling-convention trampoline both build clean for
`aarch64-linux-android` but this development environment is x86_64 with
no ARM64 execution available — see [docs/ROADMAP.md](docs/ROADMAP.md)
for the honest per-item status.

## Building

Rust workspace; `loader`, `syscall-shim`, and `runtime-shim` are plain
library crates, `tools/mkfixture` is a small CLI. Requires the
`aarch64-linux-android` rustup target to cross-check against the real
target:

```
cargo test --workspace                              # host tests
cargo check --workspace --target aarch64-linux-android  # target type-check
```

## Non-goals

- Running Apple's own OS, kernel, firmware, or system apps.
- Bit-for-bit "perfect" compatibility on day one. Compatibility is built
  incrementally, framework surface by framework surface.
- x86/x86_64 iOS simulator binaries — ARM64-only, matching real iPhone
  hardware and the ARM64 host.

## License

MIT — see [LICENSE](LICENSE). This covers original code written in this
repository only; it does not grant any rights to Apple's copyrighted
material, and none of Apple's code is included here (see
[docs/LEGAL.md](docs/LEGAL.md)).
