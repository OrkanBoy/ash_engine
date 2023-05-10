use ash::vk;
use crate::math::{self, Rotor};


#[derive(Clone, Copy)]
pub struct Instance {
    pub scale: math::Vector,
    pub rotation: math::Rotor,
    pub translation: math::Vector,

    pub translation_velocity: math::Vector,
    pub rotation_velocity: math::Bivector,
}

impl Instance {
    //Maybe use dual quat for realistic kinematics
    pub fn update_translation_kinematics(
        &mut self,
        translation_acceleration: math::Vector,
        dt: f32,
    ) {
        self.translation_velocity += translation_acceleration * dt;
        self.translation += self.translation_velocity * dt;
    }

    pub fn update_rotation_kinematics(
        &mut self,
        rotation_acceleration: math::Bivector,
        dt: f32,
    ) {
        self.rotation_velocity += rotation_acceleration * dt;
        self.rotation += self.rotation_velocity * dt * self.rotation;
        self.rotation /= self.rotation.norm_sqr().sqrt(); //Find better way, sqrt is expensive
    }
}

#[derive(Debug)]
pub struct BufferSlice {
    pub index: usize,
    pub count: usize,
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

