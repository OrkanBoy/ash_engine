#version 450
#extension GL_ARB_separate_shader_objects : enable
//#include "pga3d.glsl"

layout(location = 0) in vec3 vPos;
layout(location = 1) in vec3 vColor;

layout(binding = 0, row_major) uniform UniformBufferObject {
    mat4 viewProj;
};

layout(push_constant, row_major) uniform Push {
    mat4x3 model;
};

layout(location = 0) out vec3 fragColor;

void main() {
    gl_Position = viewProj * vec4(model * vec4(vPos, 1.0), 1.0);
    fragColor = vColor;
}