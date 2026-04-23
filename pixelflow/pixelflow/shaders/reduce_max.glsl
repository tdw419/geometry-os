#version 450
// Row-wise max reduction for softmax stability.
// Input: (N x batch). Output: (1 x batch) where each pixel = max of row.
uniform sampler2D u_input;
uniform int u_N;
out float frag_output;

void main() {
    int batch_idx = int(gl_FragCoord.y);
    float max_val = -1e30;
    for (int i = 0; i < u_N; i++) {
        float x = texelFetch(u_input, ivec2(i, batch_idx), 0).r;
        max_val = max(max_val, x);
    }
    frag_output = max_val;
}
