#version 450
// Compute mean and variance for LayerNorm in a single pass.
// Output: (1 x batch_size) texture, R=mean, G=variance

uniform sampler2D u_input;
uniform int u_K;

out vec2 frag_output;

void main() {
    int batch_idx = int(gl_FragCoord.y);
    
    float sum = 0.0;
    float sum_sq = 0.0;
    
    for (int k = 0; k < u_K; k++) {
        float x = texelFetch(u_input, ivec2(k, batch_idx), 0).r;
        sum += x;
        sum_sq += x * x;
    }
    
    float mean = sum / float(u_K);
    float variance = (sum_sq / float(u_K)) - (mean * mean);
    
    frag_output = vec2(mean, variance);
}
