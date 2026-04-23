#version 450
// Softmax over a row.
// Strategy: row-max and row-sum are passed in as uniforms or textures.
// For GPT-2 attention, we usually do this after Q*K.

uniform sampler2D u_input;  // (seq_len x seq_len)
uniform sampler2D u_max;    // (1 x seq_len)
uniform sampler2D u_sum;    // (1 x seq_len)
out float frag_output;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    float x = texelFetch(u_input, coord, 0).r;
    float row_max = texelFetch(u_max, ivec2(0, coord.y), 0).r;
    float row_sum = texelFetch(u_sum, ivec2(0, coord.y), 0).r;
    
    frag_output = exp(x - row_max) / row_sum;
}
