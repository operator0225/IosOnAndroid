//! Metal → Vulkan translation: Metal-inspired resource/pipeline
//! descriptors, translated to real Vulkan calls via `ash`.
//!
//! See `../../docs/ARCHITECTURE.md` §4 and `../../docs/ROADMAP.md` Phase 4
//! for scope and what's not implemented yet (MSL → SPIR-V shader
//! translation in particular — this module consumes SPIR-V, it doesn't
//! produce it from Metal Shading Language source).

pub mod backend;
pub mod shaders;
pub mod translate;

pub use backend::{BackendError, MetalBuffer, MetalComputePipeline, MetalDevice};
pub use translate::{find_memory_type, storage_mode_to_memory_properties, StorageMode};

#[cfg(test)]
mod end_to_end_tests;
