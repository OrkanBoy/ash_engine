use ash::vk;

use crate::math;

#[derive(Clone, Copy)]
pub struct VertexData {
    pub pos: [f32; 3],
    pub color: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct InstanceData {
    pub model: math::ModelMat,
}

pub const VERTEX_BINDING: u32 = 0;
pub const INSTANCE_BINDING: u32 = 1;

pub fn get_binding_descs() -> [vk::VertexInputBindingDescription; 2] {
    [
        vk::VertexInputBindingDescription::builder()
            .binding(VERTEX_BINDING)
            .stride(std::mem::size_of::<VertexData>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build(),
        vk::VertexInputBindingDescription::builder()
            .binding(INSTANCE_BINDING)
            .stride(std::mem::size_of::<InstanceData>() as u32)
            .input_rate(vk::VertexInputRate::INSTANCE)
            .build(),
    ]
}

pub fn get_attrib_descs() -> [vk::VertexInputAttributeDescription; 6] {
    let pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(VERTEX_BINDING)
        .location(0)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(0)
        .build();
    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(VERTEX_BINDING)
        .location(1)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(12)
        .build();

    let m0 = vk::VertexInputAttributeDescription::builder()
        .binding(INSTANCE_BINDING)
        .location(2)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(0 * 3 * 4)
        .build();
    let m1 = vk::VertexInputAttributeDescription::builder()
        .binding(INSTANCE_BINDING)
        .location(3)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(1 * 3 * 4)
        .build();
    let m2 = vk::VertexInputAttributeDescription::builder()
        .binding(INSTANCE_BINDING)
        .location(4)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(2 * 3 * 4)
        .build();
    let m3 = vk::VertexInputAttributeDescription::builder()
        .binding(INSTANCE_BINDING)
        .location(5)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(3 * 3 * 4)
        .build();
    [pos_desc, color_desc, m0, m1, m2, m3]
}