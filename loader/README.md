# loader

ARM64 Mach-O binary loader.

Parses Mach-O headers, load commands, and segments; maps segments into the
host process with correct page protections; resolves the entry point and
produces the list of unresolved external symbols for `runtime-shim/` to
satisfy.

Status: not started. See [Phase 1](../docs/ROADMAP.md#phase-1--mach-o-loader--hello-syscall)
and [Architecture §1](../docs/ARCHITECTURE.md#1-loader-loader).
