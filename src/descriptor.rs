use ash::vk;
use cgmath::Matrix4;

use crate::buffer;

//TODO: update descriptor set managing system
#[derive(Clone, Copy)]
pub struct UniformBufferObject {
    pub model: Matrix4<f32>,
    pub view: Matrix4<f32>,
    pub proj: Matrix4<f32>
}


pub fn new_descriptor_pool(device: &ash::Device, size: usize) -> vk::DescriptorPool {
    let size = size as u32;

    let pool_size = vk::DescriptorPoolSize::builder()
        .ty(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(size)
        .build();
    let pool_sizes = [pool_size];

    let info = vk::DescriptorPoolCreateInfo::builder()
        .max_sets(size)
        .pool_sizes(&pool_sizes);

    unsafe { device.create_descriptor_pool(&info, None).expect("Failed to create descriptor pool") }
}

pub fn new_descriptor_set_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    let bindings = [
        vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .build(),
    ];

    let info = vk::DescriptorSetLayoutCreateInfo::builder()
        .bindings(&bindings);

    unsafe { device.create_descriptor_set_layout(&info, None).expect("Failed to create descriptor set layouts") }
}

pub fn new_descriptor_set(
    device: &ash::Device,
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    uniform_buffer: &buffer::Buffer<UniformBufferObject>,
) -> vk::DescriptorSet {
    let layouts = [layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::builder()
        .descriptor_pool(pool)
        .set_layouts(&layouts);
    let set = unsafe {
        device.allocate_descriptor_sets(&alloc_info)
            .expect("Failed to allocate descriptor sets")[0]
    };

    let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(uniform_buffer.handle)
            .offset(0)
            .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize)
            .build();
    let buffer_infos = [buffer_info];

    let write = vk::WriteDescriptorSet::builder()
        .dst_set(set)
        .dst_array_element(0)
        .dst_binding(0)
        .buffer_info(&buffer_infos)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .build();
    let writes = [write];

    unsafe { device.update_descriptor_sets(&writes, &[]) }
    set
}