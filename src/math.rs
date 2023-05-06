//implement own matrix structs and sin cos
use cgmath::*;

//Plan: Explore R3,3 bivector generator basis
//generates 6 shears, 3 pseudo-projections, 3 scales, 3 translation, 3 rotations

//row_major
#[derive(Clone, Copy)]
pub struct Mat {
    r0c0: f32, r0c1: f32, r0c2: f32, r0c3: f32,
    r1c0: f32, r1c1: f32, r1c2: f32, r1c3: f32,
    r2c0: f32, r2c1: f32, r2c2: f32, r2c3: f32,
    r3c0: f32, r3c1: f32, r3c2: f32, r3c3: f32,
}

pub fn project(
    trans: TransformMat,
    aspect_ratio: f32,
    near_z: f32,
    far_z: f32,
) -> Mat {
    let two_near_z = 2.0 * near_z;
    
    let proj_r0c0 = two_near_z / aspect_ratio;
    let proj_r1c1 = two_near_z * near_z;
    let proj_r2c2 = far_z / (far_z - near_z);

    Mat { 
        r0c0: proj_r0c0 * trans.r0c0, 
        r0c1: proj_r0c0 * trans.r0c1, 
        r0c2: proj_r0c0 * trans.r0c2, 
        r0c3: proj_r0c0 * trans.r0c3, 
        
        r1c0: proj_r1c1 * trans.r1c0, 
        r1c1: proj_r1c1 * trans.r1c1, 
        r1c2: proj_r1c1 * trans.r1c2, 
        r1c3: proj_r1c1 * trans.r1c3, 
        
        r2c0: proj_r2c2 * trans.r2c0, 
        r2c1: proj_r2c2 * trans.r2c1, 
        r2c2: proj_r2c2 * trans.r2c2, 
        r2c3: proj_r2c2 * (trans.r2c3 - near_z), 

        r3c0: trans.r2c0, 
        r3c1: trans.r2c1, 
        r3c2: trans.r2c2, 
        r3c3: trans.r2c3,
    }
}

//row_major
#[derive(Clone, Copy)]
pub struct TransformMat {
    r0c0: f32, r0c1: f32, r0c2: f32, r0c3: f32,
    r1c0: f32, r1c1: f32, r1c2: f32, r1c3: f32,
    r2c0: f32, r2c1: f32, r2c2: f32, r2c3: f32,

}

impl std::ops::Mul for TransformMat {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            r0c0: self.r0c0 * rhs.r0c0 + self.r0c1 * rhs.r1c0 + self.r0c2 * rhs.r2c0,
            r1c0: self.r1c0 * rhs.r0c0 + self.r1c1 * rhs.r1c0 + self.r1c2 * rhs.r2c0,
            r2c0: self.r2c0 * rhs.r0c0 + self.r2c1 * rhs.r1c0 + self.r2c2 * rhs.r2c0,

            r0c1: self.r0c0 * rhs.r0c1 + self.r0c1 * rhs.r1c1 + self.r0c2 * rhs.r2c1,
            r1c1: self.r1c0 * rhs.r0c1 + self.r1c1 * rhs.r1c1 + self.r1c2 * rhs.r2c1,
            r2c1: self.r2c0 * rhs.r0c1 + self.r2c1 * rhs.r1c1 + self.r2c2 * rhs.r2c1,
            
            r0c2: self.r0c0 * rhs.r0c2 + self.r0c1 * rhs.r1c2 + self.r0c2 * rhs.r2c2,
            r1c2: self.r1c0 * rhs.r0c2 + self.r1c1 * rhs.r1c2 + self.r1c2 * rhs.r2c2,
            r2c2: self.r2c0 * rhs.r0c2 + self.r2c1 * rhs.r1c2 + self.r2c2 * rhs.r2c2,
            
            r0c3: self.r0c0 * rhs.r0c3 + self.r0c1 * rhs.r1c3 + self.r0c2 * rhs.r2c3 + self.r0c3,
            r1c3: self.r1c0 * rhs.r0c3 + self.r1c1 * rhs.r1c3 + self.r1c2 * rhs.r2c3 + self.r1c3,
            r2c3: self.r2c0 * rhs.r0c3 + self.r2c1 * rhs.r1c3 + self.r2c2 * rhs.r2c3 + self.r2c3,
        }
    }
}

impl TransformMat {
    pub fn identity() -> Self {
        TransformMat {
            r0c0: 1.0, r0c1: 0.0, r0c2: 0.0, r0c3: 0.0,
            r1c0: 0.0, r1c1: 1.0, r1c2: 0.0, r1c3: 0.0,
            r2c0: 0.0, r2c1: 0.0, r2c2: 1.0, r2c3: 0.0,
        }
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.r0c0 *= x;
        self.r0c1 *= x;
        self.r0c2 *= x;

        self.r1c0 *= y;
        self.r1c1 *= y;
        self.r1c2 *= y;

        self.r2c0 *= z;
        self.r2c1 *= z;
        self.r2c2 *= z;
        self
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.r0c3 += x;
        self.r1c3 += y;
        self.r2c3 += z;
        self
    }

    pub fn rotate(&mut self, angle: f32, yx: f32, zy: f32, xz: f32) -> &mut Self {
        let xz_zy = xz * zy;
        let zy_yx = zy * yx;
        let yx_xz  = yx * xz;
    
        let zy_zy  = zy * zy;
        let xz_xz = xz * xz;
        let yx_yx = yx * yx;
    
        let (sin, cos) = angle.sin_cos();
        let one_sub_cos = 1.0 - cos;
    
        let zy_sin = zy * sin;
        let xz_sin = xz * sin;
        let yx_sin = yx * sin;
    
        let xz_zy_one_sub_cos = xz_zy * one_sub_cos;
        let zy_yx_one_sub_cos = zy_yx * one_sub_cos;
        let yx_xz_one_sub_cos = yx_xz * one_sub_cos;


        let r0c0 = (1.0 - zy_zy) * cos + zy_zy;
        let r1c0 = xz_zy_one_sub_cos + yx_sin;
        let r2c0 = zy_yx_one_sub_cos - xz_sin;

        let r0c1 = xz_zy_one_sub_cos - yx_sin;
        let r1c1 = (1.0 - xz_xz) * cos + xz_xz;
        let r2c1 = yx_xz_one_sub_cos + zy_sin;

        let r0c2 = zy_yx_one_sub_cos + xz_sin;
        let r1c2 = yx_xz_one_sub_cos - zy_sin; 
        let r2c2 = (1.0 - yx_yx) * cos + yx_yx;


        let self_r0c0 = self.r0c0;
        let self_r1c0 = self.r1c0;
        let self_r2c0 = self.r2c0;

        let self_r0c1 = self.r0c1;
        let self_r1c1 = self.r1c1;
        let self_r2c1 = self.r2c1;

        let self_r0c2 = self.r0c2;
        let self_r1c2 = self.r1c2;
        let self_r2c2 = self.r2c2;

        let self_r0c3 = self.r0c3;
        let self_r1c3 = self.r1c3;
        let self_r2c3 = self.r2c3;


        self.r0c0 = r0c0 * self_r0c0 + r0c1 * self_r1c0 + r0c2 * self_r2c0;
        self.r1c0 = r1c0 * self_r0c0 + r1c1 * self_r1c0 + r1c2 * self_r2c0;
        self.r2c0 = r2c0 * self_r0c0 + r2c1 * self_r1c0 + r2c2 * self_r2c0;

        self.r0c1 = r0c0 * self_r0c1 + r0c1 * self_r1c1 + r0c2 * self_r2c1;
        self.r1c1 = r1c0 * self_r0c1 + r1c1 * self_r1c1 + r1c2 * self_r2c1;
        self.r2c1 = r2c0 * self_r0c1 + r2c1 * self_r1c1 + r2c2 * self_r2c1;

        self.r0c2 = r0c0 * self_r0c2 + r0c1 * self_r1c2 + r0c2 * self_r2c2;
        self.r1c2 = r1c0 * self_r0c2 + r1c1 * self_r1c2 + r1c2 * self_r2c2;
        self.r2c2 = r2c0 * self_r0c2 + r2c1 * self_r1c2 + r2c2 * self_r2c2;

        self.r0c3 = r0c0 * self_r0c3 + r0c1 * self_r1c3 + r0c2 * self_r2c3;
        self.r1c3 = r1c0 * self_r0c3 + r1c1 * self_r1c3 + r1c2 * self_r2c3;
        self.r2c3 = r2c0 * self_r0c3 + r2c1 * self_r1c3 + r2c2 * self_r2c3;

        self
    }

    pub fn as_mat4(&self) -> Matrix4<f32> {
        Matrix4::new(
            self.r0c0, self.r1c0, self.r2c0, 0.0,
            self.r0c1, self.r1c1, self.r2c1, 0.0,
            self.r0c2, self.r1c2, self.r2c2, 0.0,
            self.r0c3, self.r1c3, self.r2c3, 1.0,
        )
    }
}

pub struct Bivector {
    pub yx: f32,
    pub zy: f32,
    pub xz: f32,
}

pub struct Vector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn wedge(&self, rhs: &Vector) -> Bivector {
        Bivector { 
            yx: self.x * rhs.y - self.y * rhs.x, 
            zy: self.y * rhs.z - self.z * rhs.y, 
            xz: self.z * rhs.x - self.x * rhs.z,
        }
    }
}

impl Bivector {
    pub fn new(yx: f32, zy: f32, xz: f32) -> Self {
        Self { yx, zy, xz }
    }

    pub fn commute(&self, rhs: &Bivector) -> Bivector {
        Bivector { 
            yx: self.zy * rhs.xz - self.xz * rhs.zy, 
            zy: self.xz * rhs.yx - self.yx * rhs.xz, 
            xz: self.yx * rhs.zy - self.zy * rhs.yx,
        }
    }
}