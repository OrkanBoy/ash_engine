use std::mem::size_of;

use ash::vk;

//TODO: update descriptor set managing system
#[derive(Clone, Copy, Default)]
pub struct PerFrameUBO {
    pub proj_view: crate::math::Mat,
}

// Textures, need multiple descriptors for each texture samplers
// use different descriptor sets for difference frequency resources
// descriptor 0 is most global

// Descriptor Set 0
//   Binding 0: ProjectionView

// Descriptor Set 1
//   Binding 0: 
//      Specular texture
//      Diffuse texture
//      Normal/Height texture

pub fn new_descriptor_pool(
    device: &ash::Device,
) -> vk::DescriptorPool {
    const MAX_TEXTURE_COUNT: u32 = 20;

    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
            descriptor_count: 1,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: MAX_TEXTURE_COUNT,
        },
    ];

    let info = vk::DescriptorPoolCreateInfo::builder()
        .max_sets(2)
        .pool_sizes(&pool_sizes) // TODO: configurable
        .build();

    unsafe {
        device
            .create_descriptor_pool(&info, None)
            .expect("Failed to create descriptor pool")
    }
}

pub fn new_descriptor_set_layouts(
    device: &ash::Device,
    texture_descriptor_count: u32,
) -> (vk::DescriptorSetLayout, vk::DescriptorSetLayout) {
    let ubo_set_layout_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .build();

    let textures_set_layout_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(texture_descriptor_count)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        .build();

    let ubo_set_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
        .bindings(&[ubo_set_layout_binding])
        .build();
    let textures_set_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
        .bindings(&[textures_set_layout_binding])
        .build();

    unsafe {
        let ubo_set_layout = device
            .create_descriptor_set_layout(&ubo_set_layout_info, None)
            .unwrap();
        let textures_set_layout = device
            .create_descriptor_set_layout(&textures_set_layout_info, None)
            .unwrap();

        (ubo_set_layout, textures_set_layout)
    }
}

pub fn new_per_frame_ubo_set(
    device: &ash::Device,
    pool: vk::DescriptorPool,
    ubo_set_layout: vk::DescriptorSetLayout,
    per_frame_uniform_buffer: &super::buffer::Buffer,
) -> vk::DescriptorSet {
    let set = unsafe {
        let alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(pool)
            .set_layouts(&[ubo_set_layout])
            .build();
        device
            .allocate_descriptor_sets(&alloc_info).unwrap()[0]
    };

    let write = {
        let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(per_frame_uniform_buffer.handle)
            .offset(0)
            .range(per_frame_uniform_buffer.size)
            .build();

        vk::WriteDescriptorSet::builder()
            .dst_set(set)
            .dst_array_element(0)
            .dst_binding(0)
            .buffer_info(&[buffer_info])
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .build()
    };

    unsafe {
        device.update_descriptor_sets(&[write], &[])
    }

    set
}

pub fn new_texture_descriptor_update_template(
    device: &ash::Device,
    texture_descriptor_count: u32,
    pipeline_layout: vk::PipelineLayout,
    set_layout: vk::DescriptorSetLayout,
) -> vk::DescriptorUpdateTemplate {
    let textures_update_entry = vk::DescriptorUpdateTemplateEntry::builder()
        .dst_binding(0)
        .dst_array_element(0)
        .descriptor_count(texture_descriptor_count)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .offset(0)
        .stride(size_of::<vk::DescriptorImageInfo>())
        .build();

    let info = vk::DescriptorUpdateTemplateCreateInfo::builder()
        .flags(vk::DescriptorUpdateTemplateCreateFlags::empty())
        .descriptor_set_layout(set_layout)
        .descriptor_update_entries(&[textures_update_entry])
        .template_type(vk::DescriptorUpdateTemplateType::DESCRIPTOR_SET)
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .pipeline_layout(pipeline_layout)
        .build();

    unsafe { device.create_descriptor_update_template(&info, None).unwrap() }
}

pub fn update_textures_descriptor_set(
    device: &ash::Device,

    template: vk::DescriptorUpdateTemplate,

    set: vk::DescriptorSet,
    samplers: &[vk::Sampler],
    image_views: &[vk::ImageView],
) {
    assert!(samplers.len() == image_views.len());

    let image_infos = (0..samplers.len()).map(|i|
        vk::DescriptorImageInfo {
            sampler: samplers[i],
            image_view: image_views[i],
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }
    ).collect::<Vec<_>>();

    unsafe { device.update_descriptor_set_with_template(
        set, 
        template, 
        image_infos.as_ptr() as *const std::ffi::c_void,
    )};
}
