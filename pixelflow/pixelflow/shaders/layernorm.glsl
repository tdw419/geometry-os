#version 450
// LayerNorm: output = (x - mean) / sqrt(var + eps) * gamma + beta
// For fragment shaders, we assume mean and variance are pre-computed 
// or computed in a previous pass to avoid a massive loop per pixel.
//
// Strategy: 
// 1. Pass 1: Compute Mean and Variance for the row (1x1 texture per row)
// 2. Pass 2: Apply normalization (this shader)

uniform sampler2D u_input;  // (batch x K)
uniform sampler2D u_stats;  // (batch x 1): R=mean, G=variance
uniform sampler2D u_gamma;  // (1 x K)
uniform sampler2D u_beta;   // (1 x K)
uniform int u_K;

out float frag_output;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    int k = coord.x;
    int batch_idx = coord.y;
    
    // Sample stats for this row
    vec2 stats = texelFetch(u_stats, ivec2(0, batch_idx), 0).rg;
    float mean = stats.r;
    float variance = stats.g;
    
    // Sample input, gamma, and beta
    float x = texelFetch(u_input, coord, 0).r;
    float gamma = texelFetch(u_gamma, ivec2(k, 0), 0).r;
    float beta = texelFetch(u_beta, ivec2(k, 0), 0).r;
    
    // Apply normalization
    float eps = 1e-5;
    frag_output = ((x - mean) / sqrt(variance + eps)) * gamma + beta;
}
