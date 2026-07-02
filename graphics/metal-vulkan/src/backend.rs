//! A real Vulkan backend (via `ash`) for the handful of Metal concepts
//! this crate translates: a device, GPU-visible buffers, and compute
//! pipelines. Naming follows Metal's shape (`MetalDevice`, `MetalBuffer`,
//! `MetalComputePipeline`) because that's the architectural role these
//! types play — see `../../docs/ARCHITECTURE.md` §4 — even though every
//! field inside is a real `ash`/Vulkan object, not a reimplementation.
//!
//! Scope: compute only (buffers + compute pipelines + dispatch). Render
//! pipelines, textures, and samplers are not covered yet — see
//! `../../docs/ROADMAP.md` Phase 4.

use std::ffi::CStr;

use ash::vk;

use crate::translate::{find_memory_type, storage_mode_to_memory_properties, StorageMode};

#[derive(Debug)]
pub enum BackendError {
    Loading(String),
    Vulkan(vk::Result),
    NoComputeCapableDevice,
    NoSuitableMemoryType,
    BufferNotHostVisible,
}

impl From<vk::Result> for BackendError {
    fn from(e: vk::Result) -> Self {
        BackendError::Vulkan(e)
    }
}

pub struct MetalDevice {
    _entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
}

impl MetalDevice {
    /// Equivalent to `MTLCreateSystemDefaultDevice()`: picks the first
    /// physical device exposing a compute-capable queue family and sets
    /// up a device/queue/command pool against it.
    pub fn new() -> Result<Self, BackendError> {
        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| BackendError::Loading(e.to_string()))?;

        let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_0);
        let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&instance_info, None) }?;

        let physical_devices = unsafe { instance.enumerate_physical_devices() }?;
        let mut chosen: Option<(vk::PhysicalDevice, u32)> = None;
        for pd in &physical_devices {
            let queue_families =
                unsafe { instance.get_physical_device_queue_family_properties(*pd) };
            if let Some(idx) = queue_families
                .iter()
                .position(|q| q.queue_flags.contains(vk::QueueFlags::COMPUTE))
            {
                chosen = Some((*pd, idx as u32));
                break;
            }
        }
        let Some((physical_device, queue_family_index)) = chosen else {
            unsafe { instance.destroy_instance(None) };
            return Err(BackendError::NoComputeCapableDevice);
        };

        let queue_priorities = [1.0f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);
        let queue_infos = [queue_info];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_infos);
        let device = unsafe { instance.create_device(physical_device, &device_info, None) }?;
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&pool_info, None) }?;

        Ok(MetalDevice {
            _entry: entry,
            instance,
            physical_device,
            device,
            queue,
            queue_family_index,
            memory_properties,
            command_pool,
        })
    }

    /// Equivalent to `MTLDevice.name`.
    pub fn name(&self) -> String {
        let props = unsafe {
            self.instance
                .get_physical_device_properties(self.physical_device)
        };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) };
        name.to_string_lossy().into_owned()
    }

    /// Equivalent to `-[MTLDevice newBufferWithLength:options:]`.
    pub fn new_buffer(
        &self,
        size: u64,
        storage_mode: StorageMode,
    ) -> Result<MetalBuffer, BackendError> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { self.device.create_buffer(&buffer_info, None) }?;
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let required_properties = storage_mode_to_memory_properties(storage_mode);
        let memory_type_index = find_memory_type(
            &self.memory_properties,
            requirements.memory_type_bits,
            required_properties,
        )
        .ok_or(BackendError::NoSuitableMemoryType)?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type_index);
        let memory = unsafe { self.device.allocate_memory(&alloc_info, None) }?;
        unsafe { self.device.bind_buffer_memory(buffer, memory, 0) }?;

        let mapped_ptr = if required_properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
            let ptr = unsafe {
                self.device
                    .map_memory(memory, 0, requirements.size, vk::MemoryMapFlags::empty())
            }?;
            Some(ptr as *mut u8)
        } else {
            None
        };

        Ok(MetalBuffer {
            buffer,
            memory,
            size,
            mapped_ptr,
        })
    }

    /// Equivalent to `-[MTLDevice newComputePipelineStateWithFunction:]`,
    /// given SPIR-V (this crate does not compile MSL — see module docs).
    pub fn new_compute_pipeline(
        &self,
        spirv: &[u32],
        entry_point: &str,
    ) -> Result<MetalComputePipeline, BackendError> {
        let shader_info = vk::ShaderModuleCreateInfo::default().code(spirv);
        let shader_module = unsafe { self.device.create_shader_module(&shader_info, None) }?;

        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            unsafe { self.device.create_descriptor_set_layout(&layout_info, None) }?;

        let set_layouts = [descriptor_set_layout];
        let pipeline_layout_info =
            vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);
        let pipeline_layout = unsafe {
            self.device
                .create_pipeline_layout(&pipeline_layout_info, None)
        }?;

        let entry_name = std::ffi::CString::new(entry_point).expect("entry point has no NUL");
        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(&entry_name);
        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage_info)
            .layout(pipeline_layout);
        let pipelines = unsafe {
            self.device
                .create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
        }
        .map_err(|(_, e)| e)?;
        let pipeline = pipelines[0];

        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1);
        let descriptor_pool = unsafe { self.device.create_descriptor_pool(&pool_info, None) }?;

        Ok(MetalComputePipeline {
            shader_module,
            descriptor_set_layout,
            pipeline_layout,
            pipeline,
            descriptor_pool,
        })
    }

    /// Binds `buffer` to `pipeline`'s binding 0, dispatches
    /// `group_count_x` workgroups, and blocks until the GPU is done —
    /// equivalent to encoding + committing a `MTLComputeCommandEncoder`
    /// and waiting on it, simplified (no async/multi-encoder pipelining)
    /// since this crate exists to prove translation correctness, not to
    /// be a production scheduler.
    pub fn dispatch_and_wait(
        &self,
        pipeline: &MetalComputePipeline,
        buffer: &MetalBuffer,
        group_count_x: u32,
    ) -> Result<(), BackendError> {
        let set_layouts = [pipeline.descriptor_set_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pipeline.descriptor_pool)
            .set_layouts(&set_layouts);
        let descriptor_sets = unsafe { self.device.allocate_descriptor_sets(&alloc_info) }?;
        let descriptor_set = descriptor_sets[0];

        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(buffer.buffer)
            .offset(0)
            .range(buffer.size)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_info);
        unsafe { self.device.update_descriptor_sets(&[write], &[]) };

        let cmd_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd_buffers = unsafe { self.device.allocate_command_buffers(&cmd_alloc_info) }?;
        let cmd = cmd_buffers[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device.begin_command_buffer(cmd, &begin_info)?;
            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline.pipeline);
            self.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                pipeline.pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );
            self.device.cmd_dispatch(cmd, group_count_x, 1, 1);
            self.device.end_command_buffer(cmd)?;
        }

        let cmd_buffers_submit = [cmd];
        let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_buffers_submit);
        unsafe {
            self.device
                .queue_submit(self.queue, &[submit_info], vk::Fence::null())?;
            self.device.queue_wait_idle(self.queue)?;
            self.device
                .free_command_buffers(self.command_pool, &cmd_buffers);
        }

        Ok(())
    }

    pub fn queue_family_index(&self) -> u32 {
        self.queue_family_index
    }
}

impl Drop for MetalDevice {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

pub struct MetalBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: u64,
    mapped_ptr: Option<*mut u8>,
}

impl MetalBuffer {
    /// Equivalent to `-[MTLBuffer contents]` plus a manual `memcpy` in —
    /// only valid for a `Shared`-storage-mode buffer.
    pub fn write(&self, data: &[u8]) -> Result<(), BackendError> {
        let ptr = self.mapped_ptr.ok_or(BackendError::BufferNotHostVisible)?;
        assert!(data.len() as u64 <= self.size, "write overruns buffer");
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len()) };
        Ok(())
    }

    pub fn read(&self, out: &mut [u8]) -> Result<(), BackendError> {
        let ptr = self.mapped_ptr.ok_or(BackendError::BufferNotHostVisible)?;
        assert!(out.len() as u64 <= self.size, "read overruns buffer");
        unsafe { std::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), out.len()) };
        Ok(())
    }

    /// Frees the underlying Vulkan buffer/memory. Not a `Drop` impl
    /// because destruction must happen before the owning `MetalDevice`
    /// does, and this crate doesn't yet track that lifetime relationship
    /// at the type level — see module docs on scope.
    pub fn destroy(self, device: &MetalDevice) {
        unsafe {
            if self.mapped_ptr.is_some() {
                device.device.unmap_memory(self.memory);
            }
            device.device.destroy_buffer(self.buffer, None);
            device.device.free_memory(self.memory, None);
        }
    }
}

pub struct MetalComputePipeline {
    shader_module: vk::ShaderModule,
    descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
}

impl MetalComputePipeline {
    pub fn destroy(self, device: &MetalDevice) {
        unsafe {
            device
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            device.device.destroy_pipeline(self.pipeline, None);
            device
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            device
                .device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            device
                .device
                .destroy_shader_module(self.shader_module, None);
        }
    }
}
