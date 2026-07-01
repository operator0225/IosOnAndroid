# runtime

The Android host app: an `Activity` hosting a Vulkan-backed
`Surface`/`SurfaceView`, a process/service running the loaded binary and
shim stack, and glue translating Android input/lifecycle events into the
event model the framework shims expect.

Status: not started. See [Phase 6](../docs/ROADMAP.md#phase-6--uikit-subset--android-host-integration)
and [Architecture §6](../docs/ARCHITECTURE.md#6-android-host-runtime-runtime).
