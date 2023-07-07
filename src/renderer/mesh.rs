struct BufferSlice;

struct TexCoord {
    u: f32, v: f32,
}

struct Pos {
    x: f32, y: f32, z: f32,
}

struct Vertex {
    pos: Pos,
    diffuse_tex_coord: TexCoord,
    specular_tex_coord: TexCoord,
    normal_or_height_tex_coord: TexCoord,
}

struct Mesh {
    vertices: BufferSlice,
    indidces: BufferSlice,

    descriptor_set: vk::DescriptorSet,
}