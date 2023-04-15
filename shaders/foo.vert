#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 vPos;
layout(location = 1) in vec3 vColor;

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
};

layout(location = 0) out vec3 fragColor;

struct Rotor {
    float unit, e01, e02, e03, e12, e23, e32, e0123;
};

void mul(Rotor a, Rotor b) {

}

void main() {
    gl_Position = view * model * vec4(vPos, 0.0, 1.0);
    fragColor = vColor;
}