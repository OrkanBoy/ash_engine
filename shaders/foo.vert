#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 vPos;
// layout(location = 1) in vec3 vColor;
// layout(location = 2) in vec2 vTexCoord;

// layout(location = 3) in mat4x3 iModel;

layout(set = 0, binding = 0) uniform UniformBufferObject {
    mat4 projView;
} global_ubo;

// layout(location = 0) out vec3 fragColor;
// layout(location = 1) out vec2 fragTexCoord;

void main() {
    gl_Position = global_ubo.projView * vec4(vPos, 1.0);
    // fragColor = vColor;
    // fragTexCoord = vTexCoord;
}