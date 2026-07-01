# Architecture

Status: design only, no implementation yet. This document is the contract
future code should follow.

## Design premise: same ISA, different ABI

An iPhone (A-series SoC) and a Galaxy S25+ (Snapdragon/Exynos) are both
**ARM64 (AArch64)**. That means an ARM64 instruction stream compiled for
iOS can, in principle, execute directly on the S25+'s CPU with no
instruction-level emulation — no QEMU-style JIT, no performance tax from
translating instructions one at a time.

What makes an iOS binary "not run" on Android isn't the CPU — it's that the
binary expects an environment that doesn't exist on Android:

- **Binary format**: Mach-O, not ELF.
- **Dynamic linker**: dyld, not Android's linker.
- **Syscall ABI**: Darwin/XNU syscalls (different numbers, different
  semantics, Mach IPC primitives like ports) instead of Linux syscalls.
- **Runtime**: the Objective-C runtime (`objc_msgSend` and friends) and
  Swift runtime, largely absent from Android.
- **Frameworks**: Foundation, UIKit, Metal, etc. — these are libraries the
  binary dynamically links against; Android ships none of them.

So the project is: **keep the CPU instructions as-is, replace everything
they talk to.** This is precisely the Darling/Wine/Proton model, not a VM
or hypervisor model.

## Layers

```
┌─────────────────────────────────────────────┐
│ Android host app (Kotlin/Java + NDK glue)    │  runtime/
│  - process lifecycle, Surface, input events  │
├─────────────────────────────────────────────┤
│ Framework shims (Foundation, UIKit subset)   │  frameworks/
├─────────────────────────────────────────────┤
│ Graphics translation (Metal → Vulkan)        │  graphics/metal-vulkan/
├─────────────────────────────────────────────┤
│ Runtime shim (objc runtime, libSystem subset,│  runtime-shim/
│ dyld-compatible dynamic linker)              │
├─────────────────────────────────────────────┤
│ Syscall shim (Darwin/XNU syscall → Linux)    │  syscall-shim/
├─────────────────────────────────────────────┤
│ Mach-O loader                                │  loader/
├─────────────────────────────────────────────┤
│ Android/Linux kernel (unmodified)            │  (host OS, not ours)
├─────────────────────────────────────────────┤
│ ARM64 CPU (native execution, no emulation)   │  (hardware)
└─────────────────────────────────────────────┘
```

### 1. Loader (`loader/`)

Parses Mach-O headers, load commands, segments, and the binary's list of
imported symbols/libraries. Maps segments into the host process's address
space with correct protections. Produces a resolved entry point and a list
of external symbols that must be satisfied by layers below. Comparable in
scope to an ELF loader, just for a different format — this is public,
Apple-documented file format knowledge, not proprietary code.

### 2. Syscall shim (`syscall-shim/`)

Intercepts the small set of raw syscalls a Darwin binary issues (Darwin
uses `svc` trap numbers distinct from Linux's) and re-implements their
observable behavior using Linux/Android equivalents: file I/O maps to
Linux file I/O, `mmap`/`mprotect` map to Linux `mmap`/`mprotect`, threading
primitives map to `pthread`/`futex`, Mach ports/IPC (which have no direct
Linux analog) get reimplemented in userspace on top of pipes/eventfd/futex.

This is the layer that stands in for "the kernel" from the binary's point
of view — but it is a userspace shim translating to the *real* Android
kernel underneath, not a second kernel. There is no XNU anywhere in this
stack (see [LEGAL.md](LEGAL.md)).

### 3. Runtime shim (`runtime-shim/`)

- A `dyld`-compatible resolver: walks the loader's unresolved-symbol list
  and binds them against our shim libraries instead of Apple's.
- A clean-room Objective-C runtime: object layout, `objc_msgSend` dispatch,
  class/method tables, reference counting (ARC support). This is the
  hardest correctness-critical piece — every Objective-C-based framework
  call goes through it.
- A minimal `libSystem` subset: the C library surface (`libc`, `libdispatch`
  basics) Darwin binaries expect, backed by bionic (Android's libc)
  underneath.

### 4. Graphics translation (`graphics/metal-vulkan/`)

Metal and Vulkan are both explicit, low-level GPU APIs (unlike the old
OpenGL ES vs. Metal gap), which makes this the *most* tractable part of
the stack, structurally similar to MoltenVK (which does the reverse:
Vulkan-over-Metal on Apple platforms) or gfx-rs/wgpu's multi-backend model.

- **Command translation**: `MTLDevice`, `MTLCommandQueue`,
  `MTLRenderPipelineState`, `MTLTexture`, etc. each get a Vulkan-object
  counterpart with a translation shim between Metal's call conventions and
  Vulkan's.
- **Shader translation**: Metal Shading Language (MSL) binaries/source →
  SPIR-V. This does not need to be written from scratch — the Khronos-owned
  open-source [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross)
  toolchain already supports MSL as an input dialect; this project should
  depend on/extend it rather than reimplement a shader compiler.
- **Synchronization model mapping**: Metal's fences/events/heaps map onto
  Vulkan's semaphores/fences/memory heaps; the mapping isn't 1:1 and is
  where most subtle bugs will live.

Realistic target: enough of the Metal API surface for a typical app's
render/compute pipelines, not literally every entry point on day one.

### 5. Framework shims (`frameworks/`)

Incremental, clean-room reimplementations of the *public API contracts* of
Foundation (collections, strings, `NSObject`, KVO/KVC, run loops) and a
UIKit subset (view hierarchy, layout, basic controls, event dispatch)
sufficient to host real app UIs. Framework support is added driven by what
real test apps actually call, not built speculatively ahead of demand.

### 6. Android host runtime (`runtime/`)

The actual Android app: an `Activity` hosting a `Surface`/`SurfaceView`
backed by Vulkan, a foreground service or isolated process running the
loaded binary + shim stack, and glue translating Android input/lifecycle
events into the iOS-side event model the framework shims expect.

## Threat/compat model

Each shim layer is versioned against a specific iOS SDK version's *public*
ABI/API surface (documented via Apple's public developer documentation),
not against a specific extracted OS image. Compatibility is inherently
partial and grows one framework/one API at a time — see
[ROADMAP.md](ROADMAP.md).

## Explicitly not part of this architecture

- A hypervisor or CPU/instruction emulator (not needed — same ISA).
- Any component that boots, links against, or embeds Apple's XNU kernel,
  `dyld_shared_cache`, or system framework binaries.
- A general x86_64 iOS Simulator compatibility layer.
