#version 450
// Fused matmul + bias add
// output[j, b] = sum_k(input[k, b] * weight[k, j]) + bias[j]
uniform sampler2D u_w;
uniform sampler2D u_x;
uniform sampler2D u_b;  // bias: (out_features x 1)
uniform int u_K;
uniform int u_Ww;
uniform int u_Wh;
uniform int u_B;
out float f;

void main() {
    int j = int(gl_FragCoord.x);
    int b = int(gl_FragCoord.y);
    float s = 0.0;
    for (int k = 0; k < u_K; k++) {
        float w = texture(u_w, vec2((float(k)+0.5)/float(u_Ww), (float(j)+0.5)/float(u_Wh))).r;
        float a = texture(u_x, vec2((float(k)+0.5)/float(u_K), (float(b)+0.5)/float(u_B))).r;
        s += w * a;
    }
    f = s + texelFetch(u_b, ivec2(j, 0), 0).r;
}
