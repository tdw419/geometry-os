#version 450
// Row-wise sum of exp(x - max) for softmax denominator.
// Input: (N x batch), Max: (1 x batch). Output: (1 x batch)
uniform sampler2D u_input;
uniform sampler2D u_max;
uniform int u_N;
out float frag_output;

void main() {
    int batch_idx = int(gl_FragCoord.y);
    float row_max = texelFetch(u_max, ivec2(0, batch_idx), 0).r;
    float sum = 0.0;
    for (int i = 0; i < u_N; i++) {
        float x = texelFetch(u_input, ivec2(i, batch_idx), 0).r;
        sum += exp(x - row_max);
    }
    frag_output = sum;
}
