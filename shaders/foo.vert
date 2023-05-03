#version 450
#extension GL_ARB_separate_shader_objects : enable
//#include "pga3d.glsl"

layout(location = 0) in vec3 vPos;
layout(location = 1) in vec3 vColor;

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
};

layout(location = 0) out vec3 fragColor;

void main() {
    gl_Position = proj * view * model * vec4(vPos, 1.0);
    fragColor = vColor;
}