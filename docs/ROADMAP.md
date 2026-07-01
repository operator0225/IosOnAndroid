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

- [ ] Mach-O header/load-command/segment parser.
- [ ] Segment mapping with correct page protections.
- [ ] Darwin syscall trap table for a minimal syscall subset
      (`write`, `exit`, `mmap`, `mprotect`).
- [ ] Success criterion: a trivial iOS-compiled binary prints to logcat and
      exits cleanly on-device.

## Phase 2 — Dynamic linking & threading

- [ ] dyld-compatible symbol resolution against our own shim libraries.
- [ ] pthread/futex-backed threading primitives satisfying Darwin's thread
      API surface.
- [ ] Mach IPC primitives (ports, messages) reimplemented in userspace.
- [ ] Success criterion: a multi-threaded binary using dynamic libraries
      (still no UIKit/Foundation) runs correctly.

## Phase 3 — Objective-C runtime & minimal libSystem

- [ ] Object layout, `objc_msgSend`, class/method/ivar tables.
- [ ] ARC-compatible reference counting.
- [ ] `libSystem` subset over bionic.
- [ ] Success criterion: a binary with basic Objective-C classes (no UIKit)
      runs and produces correct output.

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
