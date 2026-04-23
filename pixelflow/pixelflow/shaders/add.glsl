#version 450
// Element-wise add: output = a + b
uniform sampler2D u_a;
uniform sampler2D u_b;
out float frag_output;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    float a = texelFetch(u_a, coord, 0).r;
    float b = texelFetch(u_b, coord, 0).r;
    frag_output = a + b;
}
