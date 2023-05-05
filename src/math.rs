//implement own matrix structs and sin cos
use cgmath::*;

//Plan: Explore R3,3 bivector generator basis
//generates 6 shears, 3 pseudo-projections, 3 scales, 3 translation, 3 rotations

//assumes a, b, c normalized
//probably slower than cgmath as lot of load instructions
//rotates then translates
//rigid transformation presering scale
pub fn rigid_mat(
    t: f32, 
    a: f32, 
    b: f32, 
    c: f32, 
    x: f32, 
    y: f32, 
    z: f32
) -> Matrix4<f32> {
    let ab = a * b;
    let ac = a * c;
    let bc = b * c;

    let aa = a * a;
    let bb = b * b;
    let cc = c * c;

    let (sin, cos) = t.sin_cos();
    let one_sub_cos = 1.0 - cos;

    let asin = a * sin;
    let bsin = b * sin;
    let csin = c * sin;

    let ab_one_sub_cos = ab * one_sub_cos;
    let ac_one_sub_cos = ac * one_sub_cos;
    let bc_one_sub_cos = bc * one_sub_cos;

    //[n(n.)] + sin(t)[nx] + cos(t)[1 - n(n.)]
    Matrix4::new(
        (1.0 - aa) * cos + aa,
        ab_one_sub_cos + csin,
        ac_one_sub_cos - bsin,
        0.0,

        ab_one_sub_cos - csin,
        (1.0 - bb) * cos + bb,
        bc_one_sub_cos + asin,
        0.0,

        ac_one_sub_cos + bsin,
        bc_one_sub_cos - asin, 
        (1.0 - cc) * cos + cc,
        0.0,

        x, 
        y, 
        z, 
        1.0,
    )
}


//avoid general matrix inverse algoirthm
//(T * R)^-1 = R^t * T^-1



pub fn inverse_rigid_mat(
    t: f32, 
    a: f32, 
    b: f32, 
    c: f32, 
    x: f32, 
    y: f32, 
    z: f32
) -> Matrix4<f32> {
    let ab = a * b;
    let ac = a * c;
    let bc = b * c;

    let aa = a * a;
    let bb = b * b;
    let cc = c * c;

    let (sin, cos) = t.sin_cos();
    let one_sub_cos = 1.0 - cos;

    let asin = a * sin;
    let bsin = b * sin;
    let csin = c * sin;

    let ab_one_sub_cos = ab * one_sub_cos;
    let ac_one_sub_cos = ac * one_sub_cos;
    let bc_one_sub_cos = bc * one_sub_cos;

    //[n(n.)] + sin(t)[nx] + cos(t)[1 - n(n.)]
    let mut mat = Matrix4::new(
        (1.0 - aa) * cos + aa,
        ab_one_sub_cos - csin,
        ac_one_sub_cos + bsin,
        0.0,

        ab_one_sub_cos + csin,
        (1.0 - bb) * cos + bb,
        bc_one_sub_cos - asin,
        0.0,

        ac_one_sub_cos - bsin,
        bc_one_sub_cos + asin, 
        (1.0 - cc) * cos + cc,
        0.0,

        0.0, 
        0.0, 
        0.0, 
        1.0,
    );
    mat.w.x = -(mat.x.x * x + mat.y.x * y + mat.z.x * z);
    mat.w.y = -(mat.x.y * x + mat.y.y * y + mat.z.y * z);
    mat.w.z = -(mat.x.z * x + mat.y.z * y + mat.z.z * z);

    mat
}


pub fn proj_mat(
    aspect_ratio: f32,
    scale_x: f32,
    scale_y: f32,
    scale_z: f32,
    near_z: f32,
) -> Matrix4<f32> {
    Matrix4::new(
        2.0 * near_z / scale_x, 0.0, 0.0, 0.0,
        0.0, 2.0 * aspect_ratio * near_z / scale_y, 0.0, 0.0,
        0.0, 0.0, (near_z + scale_z) / scale_z, 1.0,
        0.0, 0.0, -(near_z + scale_z) * near_z / scale_z, 0.0,
    )
}