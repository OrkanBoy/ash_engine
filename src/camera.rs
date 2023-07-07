use crate::math::*;

pub struct Camera {
    pub translation: Vector,
    
    /// z axis to x axis angle
    pub z_x_angle: f32,
    /// y axis to xz plane angle
    pub y_xz_angle: f32,

    /// screen ratio width to height
    pub aspect_ratio: f32,

    pub near_z: f32,
    pub far_z: f32,

    pub translation_speed: f32,
    pub rotation_speed: f32,
}

impl Camera {
    pub fn calc_proj_view(&self) -> Mat {
        let plane = Vector::new(0.0, -1.0, 0.0).wedge(
            &Vector::new(self.z_x_angle.sin(), 0.0, self.z_x_angle.cos())
        );
        
        ModelMat::identity()
            .translate(-self.translation.x, -self.translation.y, -self.translation.z)
            .rotate(-self.y_xz_angle, plane.yx, plane.zy, plane.xz)
            .rotate(-self.z_x_angle, 0.0, 0.0, 1.0)
            .project(
                self.aspect_ratio,
                self.near_z,
                self.far_z,
            )
    }
}



