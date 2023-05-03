struct Rotor {
    float _;
    float yx, zy, xz;
    float ix, iy, iz;
    float ixyz;
}

struct Bivector {
    float yx, zy, xz;
    float ix, iy, iz;
};

//48 muls, 40 adds
//pricy... research more!
Rotor mul_rotor(Rotor a, Rotor b) {
    Rotor r;
    r._ = a._ * b._ - a.yx * b.yx - a.zy * b.zy - a.xz * b.xz;
    r.yx = a._ * b.yx + a.yx * b._ + a.zy * b.xz - a.xz * b.zy;
    r.zy = a._ * b.zy - a.yx * b.xz + a.zy * b._ + a.xz * b.yx;
    r.xz = a._ * b.xz + a.yx * b.zy - a.zy * b.yx + a.xz * b._;

    r.ix = a.ix * b._ + a.iy * b.yx - a.iz * b.xz + a.ixyz * b.zy
        + a._ * b.ix - a._yx * b.ix + a._zy * b.ix - a.xz * b.ix;
    r.iy = a.iy * b._ - a.ix * b.yx + a.iz * b.zy + a.ixyz * b.xz
        + a._ * b.iy + a._yx * b.iy + a._zy * b.iy - a.xz * b.iy;
    r.iz = a.ix * b.xz - a.iy * b.zy + a.iz * b._ + a.ixyz * b.yx
        + a._ * b.iz - a._yx * b.iz - a._zy * b.iz - a.xz * b.iz;
    r.ixyz = a.ixyz * b._ - a.iy * b._ - a.ix * b.yx - a.iz * b.yx
        + a._ * b.ixyz + a._yx * b.ixyz + a._zy * b.ixyz + a.xz* b.ixyz;

    return r;
}


Rotor exp_bivector(Bivector b) {
    Rotor r;

    r._ = 
}
Rotor log_rotor(Rotor r);

struct Proj {
    float scale_x;
    float scale_y;
    float scale_z;
    float near_z;
};

vec4 apply_rotor_and_proj(Rotor r, Proj p, vec4 v);