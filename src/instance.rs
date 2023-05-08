use ash::vk;
use crate::math;

#[derive(Clone, Copy)]
pub struct Instance {
    pub scale: math::Vector,
    pub rotation: math::Rotor,
    pub translation: math::Vector,
}

pub struct  UniformData {
    pub proj_view: math::Mat,
}

#[derive(Debug)]
pub struct BufferSlice {
    pub index: usize,
    pub count: usize,
}

pub struct GameObject {
    pub vertex_slice: BufferSlice,
    pub index_slice: BufferSlice,

    pub uniform_buffer: vk::Buffer,
    pub uniform_memory: vk::DeviceMemory,
    pub uniform_instances_size: usize,
}

#[derive(Debug)]
pub struct Particle {
    pub vertex_slice: BufferSlice,
    pub index_slice: BufferSlice,

    pub instance_slice: BufferSlice,
    pub instance_slice_max_count: usize,
}

impl Instance {
    pub fn calc_model_mat(&self) -> math::ModelMat {
        math::ModelMat::from(self.scale, self.rotation, self.translation)
    }
}

impl Particle {
}

