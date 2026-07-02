//! Runs actual GPU (or software-rasterizer) work through the translation
//! layer: allocate a Metal-shaped buffer, fill it from the CPU side,
//! dispatch a real compute shader against it via a real Vulkan device,
//! and check the GPU's own output. This is the strongest verification
//! this crate can offer short of a physical Android device — genuinely
//! exercising `translate.rs` + `backend.rs` together, not just type
//! checking them.
//!
//! Requires a loadable Vulkan implementation. On a machine with a real
//! GPU driver this just works; on a driver-less CI host, point
//! `VK_ICD_FILENAMES` at a software implementation (e.g. SwiftShader,
//! Lavapipe) to get the same coverage without hardware. Either way, if
//! no Vulkan device is available at all, these tests skip themselves
//! (print + return) rather than failing the whole suite — the important
//! invariant is "if a device exists, our translation must be correct on
//! it", not "a device must always exist here".

use crate::{shaders, MetalDevice, StorageMode};

fn device_or_skip(test_name: &str) -> Option<MetalDevice> {
    match MetalDevice::new() {
        Ok(device) => Some(device),
        Err(e) => {
            eprintln!(
                "skipping {test_name}: no usable Vulkan device/loader available ({e:?}). \
                 Set VK_ICD_FILENAMES to a software implementation to run this test."
            );
            None
        }
    }
}

#[test]
fn doubles_buffer_contents_on_a_real_vulkan_device() {
    let Some(device) = device_or_skip("doubles_buffer_contents_on_a_real_vulkan_device") else {
        return;
    };
    eprintln!("running against device: {}", device.name());

    const ELEMENT_COUNT: usize = 256;
    let byte_len = (ELEMENT_COUNT * std::mem::size_of::<u32>()) as u64;

    let buffer = device
        .new_buffer(byte_len, StorageMode::Shared)
        .expect("create buffer");

    let input: Vec<u32> = (0..ELEMENT_COUNT as u32).collect();
    let input_bytes: Vec<u8> = input.iter().flat_map(|v| v.to_ne_bytes()).collect();
    buffer.write(&input_bytes).expect("write input");

    let pipeline = device
        .new_compute_pipeline(shaders::DOUBLE_SPIRV, shaders::DOUBLE_ENTRY_POINT)
        .expect("create compute pipeline");

    let group_count = (ELEMENT_COUNT as u32).div_ceil(shaders::DOUBLE_LOCAL_SIZE_X);
    device
        .dispatch_and_wait(&pipeline, &buffer, group_count)
        .expect("dispatch");

    let mut output_bytes = vec![0u8; byte_len as usize];
    buffer.read(&mut output_bytes).expect("read output");
    let output: Vec<u32> = output_bytes
        .chunks_exact(4)
        .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
        .collect();

    for (i, &v) in output.iter().enumerate() {
        assert_eq!(v, (i as u32) * 2, "element {i} was not doubled by the GPU");
    }

    pipeline.destroy(&device);
    buffer.destroy(&device);
}

#[test]
fn private_storage_buffer_is_not_host_readable() {
    // Proves storage_mode_to_memory_properties (translate.rs) actually
    // changes device behavior, not just the value it returns: a Private
    // buffer must come back with no mapped pointer, so write()/read()
    // fail cleanly instead of segfaulting through a null/bogus pointer.
    let Some(device) = device_or_skip("private_storage_buffer_is_not_host_readable") else {
        return;
    };

    let buffer = device
        .new_buffer(256, StorageMode::Private)
        .expect("create private buffer");

    let data = [0u8; 4];
    let mut out = [0u8; 4];
    assert!(matches!(
        buffer.write(&data),
        Err(crate::BackendError::BufferNotHostVisible)
    ));
    assert!(matches!(
        buffer.read(&mut out),
        Err(crate::BackendError::BufferNotHostVisible)
    ));

    buffer.destroy(&device);
}
