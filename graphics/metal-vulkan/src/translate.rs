//! Pure Metal-concept → Vulkan-concept translation: no device, no
//! instance, nothing GPU-touching. Like `syscall-shim::translate`, this is
//! deliberately side-effect-free so it's testable on any host regardless
//! of whether a Vulkan driver (real or software) is available — see
//! `backend.rs` for where the actual `ash` calls live.

use ash::vk;

/// A simplified stand-in for Metal's `MTLStorageMode`. Real Metal also has
/// `Managed` (macOS-only — doesn't exist on iOS, so out of scope for an
/// iOS-targeting layer) and `Memoryless` (a tile-memory optimization, not
/// needed for correctness); only the two modes that matter for getting
/// data on/off the GPU at all are covered here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    /// CPU and GPU see the same coherent memory — what's needed to
    /// `memcpy` data in from the CPU side (or read GPU output back) without
    /// explicit staging/synchronization.
    Shared,
    /// GPU-only memory: fastest for the GPU, not CPU-accessible at all.
    Private,
}

/// Metal's storage mode is really asking a memory-visibility/coherency
/// question that Vulkan answers with `VkMemoryPropertyFlags` — but the two
/// APIs don't share bit values or even a shared concept boundary (Metal's
/// "storage mode" bundles what Vulkan splits across memory property flags
/// *and* usage flags), so this is a real translation, not a re-labeling.
pub fn storage_mode_to_memory_properties(mode: StorageMode) -> vk::MemoryPropertyFlags {
    match mode {
        StorageMode::Shared => {
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        }
        StorageMode::Private => vk::MemoryPropertyFlags::DEVICE_LOCAL,
    }
}

/// Finds a memory type index satisfying `type_bits` (from
/// `VkMemoryRequirements::memory_type_bits`) and containing all of
/// `required_properties`. This is ordinary Vulkan program boilerplate —
/// but it's also pure lookup logic over data the driver handed us, so
/// it's fully testable against synthetic `PhysicalDeviceMemoryProperties`
/// without a real device or instance.
pub fn find_memory_type(
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    required_properties: vk::MemoryPropertyFlags,
) -> Option<u32> {
    for i in 0..memory_properties.memory_type_count {
        let type_supported = type_bits & (1 << i) != 0;
        let properties_supported = memory_properties.memory_types[i as usize]
            .property_flags
            .contains(required_properties);
        if type_supported && properties_supported {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_storage_is_host_visible_and_coherent() {
        let flags = storage_mode_to_memory_properties(StorageMode::Shared);
        assert!(flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE));
        assert!(flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT));
        assert!(!flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL));
    }

    #[test]
    fn private_storage_is_device_local_only() {
        let flags = storage_mode_to_memory_properties(StorageMode::Private);
        assert_eq!(flags, vk::MemoryPropertyFlags::DEVICE_LOCAL);
    }

    #[test]
    fn shared_and_private_translate_to_genuinely_different_flags() {
        assert_ne!(
            storage_mode_to_memory_properties(StorageMode::Shared),
            storage_mode_to_memory_properties(StorageMode::Private)
        );
    }

    #[allow(clippy::field_reassign_with_default)] // memory_types is a fixed-size array; imperative indexing below reads clearer than a giant struct literal
    fn synthetic_memory_properties() -> vk::PhysicalDeviceMemoryProperties {
        let mut props = vk::PhysicalDeviceMemoryProperties::default();
        props.memory_type_count = 3;
        // type 0: device-local only
        props.memory_types[0] = vk::MemoryType {
            property_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            heap_index: 0,
        };
        // type 1: host-visible + host-coherent (but not device-local)
        props.memory_types[1] = vk::MemoryType {
            property_flags: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT,
            heap_index: 1,
        };
        // type 2: host-visible + coherent + cached (a "better" shared type)
        props.memory_types[2] = vk::MemoryType {
            property_flags: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT
                | vk::MemoryPropertyFlags::HOST_CACHED,
            heap_index: 1,
        };
        props
    }

    #[test]
    fn finds_matching_memory_type_by_properties() {
        let props = synthetic_memory_properties();
        let all_types_bits = 0b111;
        let idx = find_memory_type(
            &props,
            all_types_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .unwrap();
        assert_eq!(idx, 0);

        let idx = find_memory_type(
            &props,
            all_types_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .unwrap();
        // First match wins, matching real Vulkan advice to prefer the
        // lowest-index qualifying type.
        assert_eq!(idx, 1);
    }

    #[test]
    fn respects_the_type_bits_mask() {
        let props = synthetic_memory_properties();
        // Exclude type 0 (bit 0) even though nothing else is required —
        // simulates a resource whose VkMemoryRequirements says it can
        // only live in types 1 or 2.
        let type_bits_excluding_0 = 0b110;
        let idx = find_memory_type(
            &props,
            type_bits_excluding_0,
            vk::MemoryPropertyFlags::empty(),
        )
        .unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn returns_none_when_no_type_matches() {
        let props = synthetic_memory_properties();
        let idx = find_memory_type(
            &props,
            0b111,
            vk::MemoryPropertyFlags::DEVICE_LOCAL | vk::MemoryPropertyFlags::HOST_VISIBLE,
        );
        assert_eq!(idx, None);
    }
}
