# graphics/metal-vulkan

Metal → Vulkan API and shader translation layer. Structurally the mirror
image of MoltenVK (Vulkan-over-Metal); this is Metal-over-Vulkan. Compute
path only so far.

- `translate.rs` — pure, side-effect-free translation: Metal storage
  modes → Vulkan memory property flags, plus the usual
  find-a-matching-memory-type lookup. Testable against synthetic
  `PhysicalDeviceMemoryProperties` on any host, no GPU needed.
- `backend.rs` — `MetalDevice`/`MetalBuffer`/`MetalComputePipeline`: real
  `ash`/Vulkan objects (instance, device, buffers, compute pipelines,
  descriptor sets, dispatch), named after their Metal counterparts because
  that's the architectural role they play.
- `shaders.rs` / `shaders/*.comp` — a hand-written GLSL test shader,
  compiled to SPIR-V with `glslangValidator` and checked with `spirv-val`
  (see the `.comp` file for the exact command), embedded as `u32` words.
  Stands in for what an MSL→SPIR-V translation pass would eventually
  produce — this crate consumes SPIR-V, it doesn't compile MSL yet (see
  below).
- `end_to_end_tests.rs` — allocates a real GPU buffer, writes known values
  from the CPU side, dispatches the embedded shader through the full
  `translate` + `backend` stack via a **real Vulkan device**, and checks
  the GPU's own output. Materially stronger verification than this
  project's ARM64-execution-shaped pieces can get in this dev environment,
  since a software Vulkan implementation runs fine on x86_64.

## Running the GPU-backed tests

They skip themselves cleanly if no Vulkan implementation is loadable. On a
machine with a real GPU driver, `cargo test -p iosonandroid-metal-vulkan`
just works. Without one (e.g. this project's headless dev container), point
`VK_ICD_FILENAMES` at any software Vulkan implementation, for example:

```
VK_ICD_FILENAMES=/path/to/vk_swiftshader_icd.json \
  cargo test -p iosonandroid-metal-vulkan -- --nocapture
```

## Not implemented yet

- MSL → SPIR-V translation itself (the real target dependency is
  [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross), which
  supports MSL as an input dialect — not wired up).
- Render pipelines, textures, samplers — compute only right now.
- Real synchronization primitive mapping (fences/events/heaps); this phase
  blocks on `queue_wait_idle` after every dispatch instead of pipelining.
- Running against a real on-device Vulkan driver (Adreno on the S25+) —
  everything above has only been exercised against a software
  implementation on an x86_64 host.

Status: Phase 4, compute path done + verified against a real (software)
Vulkan device. See
[Phase 4](../../docs/ROADMAP.md#phase-4--metal--vulkan-translation) and
[Architecture §4](../../docs/ARCHITECTURE.md#4-graphics-translation-graphicsmetal-vulkan).
