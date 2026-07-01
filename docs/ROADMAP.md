# Roadmap

Each phase should produce something demonstrable and testable before the
next one starts. Phases are not time-boxed here (this is a spare-time/
research-scale project, not a funded team), but ordered by dependency.

## Phase 0 — Architecture & scaffolding (current)

- [x] Define scope and legal boundaries ([LEGAL.md](LEGAL.md)).
- [x] Define layered architecture ([ARCHITECTURE.md](ARCHITECTURE.md)).
- [x] Repo module scaffolding.
- [ ] Pick concrete toolchain: Rust vs. C/C++ for the shim layers, NDK
      version, minimum Android API level (Vulkan 1.1+ requires API 24+;
      realistically target recent Android given S25+ as the reference
      device), CMake/Cargo build wiring into Gradle.
- [ ] Set up a self-built ARM64 Mach-O test corpus: trivial binaries
      compiled with a real Apple toolchain by the developer (not
      redistributed), used only as local test fixtures, never committed to
      the repo.

## Phase 1 — Mach-O loader + "hello syscall"

Goal: load a statically-linked, no-framework-dependency ARM64 Mach-O binary
and get it to execute a handful of raw syscalls (write to stdout, exit)
via the syscall shim.

- [x] Mach-O header/load-command parser (`loader/`): `LC_SEGMENT_64`,
      `LC_MAIN`, `LC_UNIXTHREAD` (classic ARM64 thread state). Bounds-checked
      against malformed/truncated/hostile input, 12 unit tests.
- [x] Darwin → Linux syscall translation table (`syscall-shim/translate.rs`)
      for `exit`, `write`, `mmap` (with correct `MAP_ANON`/flag-bit
      translation — Darwin's and Linux's bit values differ), `mprotect`.
      Pure/host-testable, 9 unit tests, no ptrace or ARM64 execution needed.
- [x] `tools/mkfixture`: a minimal hand-assembled (via `llvm-mc`, not
      Apple's toolchain) ARM64 Mach-O executable, round-trip tested against
      the loader.
- [ ] Segment *mapping into a live process* + the aarch64 ptrace
      syscall-interception loop (`syscall-shim/ptrace.rs`) exist as code,
      cross-compile clean for `aarch64-linux-android`, but are **unverified
      on real hardware** — this development environment is x86_64 with no
      aarch64 emulator available. Needs testing on an actual ARM64 device
      (e.g. the S25+) or aarch64 CI runner before this box can be checked.
- [ ] Success criterion: a trivial iOS-compiled binary prints to logcat and
      exits cleanly on-device. **Not yet met** — blocked on the item above.

## Phase 2 — Dynamic linking & threading

- [x] Mach-O symbol table parsing (`loader`): classic `nlist_64`
      `LC_SYMTAB` → import/export lists, plus `LC_LOAD_DYLIB` dependency
      names. 17 unit tests. Not yet understood: `LC_DYLD_CHAINED_FIXUPS`,
      the newer binding scheme most real modern iOS binaries use instead
      of (or alongside) classic relocations — needed before this handles
      real-world binaries, not just hand-built fixtures.
- [x] dyld-compatible symbol resolution against our own shim libraries
      (`runtime-shim::registry`): resolves an image's imports against a
      registry of shim symbol addresses, honestly reporting anything
      unresolved rather than guessing. Pure/host-testable, 4 unit tests.
      Not yet wired to anything: there's no live process to bind resolved
      addresses *into* yet (needs the Phase 1 mapping step) and no actual
      shim symbols registered (needs Phase 3's libSystem subset).
- [x] A futex-backed mutex (`runtime-shim::sync::FutexMutex`) — one piece
      of the threading-primitive surface, standing in for what Darwin's
      `os_unfair_lock`/`pthread_mutex_t` need once nothing has XNU under
      it. Fully arch-independent, verified with real concurrent threads on
      host (unlike the ptrace loop, this needed no aarch64 hardware to
      test). Not yet done: condition variables, semaphores, and — the
      bigger piece — actually creating Darwin threads at all (`bsdthread_create`
      is unusually kernel-assisted on Darwin, unlike Linux's `clone`-based
      pthreads; translating it is its own chunk of work).
- [ ] Mach IPC primitives (ports, messages) reimplemented in userspace.
- [ ] Success criterion: a multi-threaded binary using dynamic libraries
      (still no UIKit/Foundation) runs correctly. **Not yet met** — needs
      the above thread-creation path plus Phase 1's still-open live-process
      mapping/ptrace verification.

## Phase 3 — Objective-C runtime & minimal libSystem

- [x] Object layout, class/method/ivar tables, and `objc_msgSend`'s
      dispatch *logic* (`runtime-shim::objc`): superclass-chain method
      lookup, override shadowing, `-isKindOfClass:`-style checks, ivar
      offset accumulation across inheritance, and — easy to get wrong —
      Objective-C's "sending a message to `nil` is a legal no-op"
      semantics. 7 unit tests. **Not done**: the real ARM64
      `objc_msgSend` calling-convention trampoline (receiver in `x0`,
      selector in `x1`, tail-call into the resolved `Imp`) — needs real
      ARM64 execution to verify, same caveat as `syscall-shim::ptrace`.
      Also not done: parsing actual classes out of a Mach-O's
      `__objc_classlist`/`__objc_methname` sections — real, well-documented
      format, just not implemented yet.
- [x] ARC-compatible reference counting (`runtime-shim::arc::RefCounted`):
      retain/release/dealloc-signal, with over-release reported instead of
      silently corrupting memory the way real ARC's undefined behavior
      would. Verified single-threaded and with 8 real concurrent threads.
- [x] `libSystem` subset over bionic (`runtime-shim::libsystem`) — a
      deliberately small one: `_malloc`/`_free`/`_memcpy`/`_strlen` shims
      registered into `Registry` under their Darwin symbol names, tested
      by actually calling through the registered function-pointer
      addresses (proves they're real working trampolines to bionic, not
      placeholders). The other few hundred libSystem symbols a typical
      binary imports are not covered — grown incrementally as real test
      binaries demand them, not speculatively.
- [ ] Success criterion: a binary with basic Objective-C classes (no UIKit)
      runs and produces correct output. **Not yet met** — blocked on the
      ARM64 `objc_msgSend` trampoline and the still-open Phase 1 live
      process/ptrace verification above.

## Phase 4 — Metal → Vulkan translation

- [ ] Core object mapping (`MTLDevice`, `MTLCommandQueue`, buffers,
      textures, pipeline states).
- [ ] MSL → SPIR-V via SPIRV-Cross integration.
- [ ] Synchronization primitive mapping.
- [ ] Success criterion: a standalone Metal compute/render sample (no
      UIKit) renders correctly to an Android `Surface`.

## Phase 5 — Foundation subset

- [ ] Core `NSObject`, collections (`NSArray`/`NSDictionary`/`NSString`),
      run loop, KVO/KVC basics.
- [ ] Success criterion: a non-UI command-line-style iOS binary using
      Foundation runs correctly.

## Phase 6 — UIKit subset & Android host integration

- [ ] View hierarchy, Auto Layout subset, basic controls, touch event
      dispatch.
- [ ] Android `Activity`/`Surface` host wired to the shim stack's render
      output and input events.
- [ ] Success criterion: a simple real-world app (developer's own build)
      launches, renders its UI, and responds to touch on an S25+.

## Phase 7 — Breadth, testing, hardening

- [ ] Expand framework coverage driven by real test-app crash logs/missing
      symbols, not speculatively.
- [ ] Automated test harness (corpus of self-built test binaries + CI).
- [ ] Performance profiling of the syscall/graphics shim hot paths.

## Explicitly deferred / unscheduled

- Broad UIKit/SwiftUI parity, Swift runtime support, Metal Performance
  Shaders, ARKit/CoreML and other high-level frameworks — large efforts,
  only worth scoping once Phase 6 proves the core stack works.
