# graphics/metal-vulkan

Metal → Vulkan API and shader translation layer. Structurally the mirror
image of MoltenVK (Vulkan-over-Metal); this is Metal-over-Vulkan.

- Command/object translation: `MTLDevice`, `MTLCommandQueue`,
  `MTLRenderPipelineState`, `MTLTexture`, etc. → Vulkan equivalents.
- Shader translation: MSL → SPIR-V via
  [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross) (dependency,
  not reimplemented).
- Synchronization mapping: Metal fences/events/heaps → Vulkan
  semaphores/fences/memory heaps.

Status: not started. See [Phase 4](../../docs/ROADMAP.md#phase-4--metal--vulkan-translation)
and [Architecture §4](../../docs/ARCHITECTURE.md#4-graphics-translation-graphicsmetal-vulkan).
