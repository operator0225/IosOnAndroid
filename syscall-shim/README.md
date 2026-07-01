# syscall-shim

Intercepts Darwin/XNU syscalls issued by a loaded Mach-O binary and
re-implements their observable behavior on top of Linux/Android syscalls.
This is the layer that stands in for "the kernel" from the binary's point
of view — a userspace translator over the real Android kernel, not a
second kernel or a copy of XNU.

Status: not started. See [Phase 1-2](../docs/ROADMAP.md) and
[Architecture §2](../docs/ARCHITECTURE.md#2-syscall-shim-syscall-shim).
