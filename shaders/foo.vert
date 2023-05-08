#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 vPos;
layout(location = 1) in vec3 vColor;

layout(location = 2) in mat4x3 iModel;

layout(binding = 0) uniform UniformBufferObject {
    mat4 viewProj;
};

layout(location = 0) out vec3 fragColor;

void main() {
    gl_Position = viewProj * vec4(iModel * vec4(vPos, 1.0), 1.0);
    fragColor = vColor;
}