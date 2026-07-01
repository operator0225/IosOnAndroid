# Legal & Compliance Notes

This document exists because "iOS emulation" projects have real legal
precedent that shapes what this project can responsibly build. Read it
before contributing code.

## What happened to Corellium, and why it matters here

Corellium built ARM virtualization that ran **actual iOS firmware**
(Apple's real kernel, real system frameworks, extracted from real IPSW
files) inside a VM. Apple sued them for copyright infringement and DMCA
anti-circumvention. The case (settled in 2021 after years of litigation)
turned specifically on the fact that Corellium's product booted Apple's
own copyrighted OS images. That is the scenario this project must not
recreate.

## The line this project draws

| Do | Don't |
|---|---|
| Reimplement the *behavior* of a Darwin syscall from public documentation/observed behavior (clean-room) | Copy, decompile, or extract code from Apple's XNU kernel source or binaries |
| Reimplement a subset of Foundation/UIKit's *public API surface* from Apple's published documentation | Bundle or redistribute Apple's `.framework` binaries, `dyld_shared_cache`, or any file extracted from an IPSW/real device |
| Load and run a Mach-O binary the *user* legitimately built (their own app, compiled with their own Xcode/toolchain, under their own Apple ID/entitlements) | Ship, download, or require Apple's iOS firmware, iTunes/Xcode-restricted binaries, or jailbreak-sourced OS images as part of this project |
| Translate Metal API calls to Vulkan calls (an API-compatibility shim, same category as MoltenVK/DXVK/Proton) | Emulate or virtualize Apple's actual GPU driver/kernel graphics stack |
| Discuss and link to public specs (Mach-O format, ARM64 ABI, Vulkan/Metal specs) | Distribute Apple-copyrighted headers, SDKs, or documentation text verbatim |

## Practical consequence for the architecture

Because we cannot use Apple's kernel, **there is no "kernel" in this
project's sense** — no privileged component that owns memory
management, scheduling, or device drivers the way XNU does on a real
iPhone. Instead, the syscall shim (docs/ARCHITECTURE.md §2) intercepts the
narrow set of Darwin syscalls a userspace binary issues and answers them
using Linux/Android's own kernel underneath. This is not a simplification
for convenience — it is the only legally sound design, and it's also
exactly how Darling and Wine work.

## What "success" realistically looks like

Running a *user's own, independently built* iOS app (no Apple system
frameworks bundled) with an increasing subset of Foundation/UIKit/Metal
supported. Not: booting an iPhone's actual OS, or running unmodified
App Store binaries that depend on Apple's proprietary frameworks being
present.

## If you're unsure whether something you want to add is in scope

Ask: "Does this require a byte of Apple's copyrighted kernel, firmware, or
framework binary to exist on disk?" If yes, it doesn't belong in this repo.
