#version 330 core

layout(location = 0) in vec2 pos;

uniform vec4 matrix = vec4(0.0f, 0.0f, 1.0f, 1.0f); // [x, y, w, h]

void main() {
    gl_Position = vec4(pos.xy * matrix.zw + matrix.xy, 0.0, 1.0);
}
