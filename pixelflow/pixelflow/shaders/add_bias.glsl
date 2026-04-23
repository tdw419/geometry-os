#version 450
// Add bias: output = input + bias
// input is (K x batch), bias is (K x 1) broadcast across batch
uniform sampler2D u_input;
uniform sampler2D u_bias;
out float frag_output;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    int k = coord.x;
    float x = texelFetch(u_input, coord, 0).r;
    float b = texelFetch(u_bias, ivec2(k, 0), 0).r;
    frag_output = x + b;
}
