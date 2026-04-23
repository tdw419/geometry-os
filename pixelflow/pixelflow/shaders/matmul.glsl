#version 450
// Matrix multiply via fragment shader.
// Each fragment computes one element of C = A @ W.T
// A = input vector (1 x K), W = weight matrix (M x K)
// Output pixel (x, y) = C[y][x] = sum over k of A[y][k] * W[x][k]
// Note: W is stored row-major, so texture(x, y) = W[y][x]
// We need W[x][k] = texture(k, x), sampled at uv ((k+0.5)/Ww, (x+0.5)/Wh)

uniform sampler2D u_weights;   // W: (M x K) as texture (K wide, M tall)
uniform sampler2D u_input;     // A: (batch x K) as texture (K wide, batch tall)
uniform int u_K;               // shared dimension (in_features)
uniform int u_Ww;              // weight texture width (= K)
uniform int u_Wh;              // weight texture height (= M = out_features)
uniform int u_batch;           // batch size (usually 1)
uniform int u_mode;            // 0=float32, 1=float16, 2=int8

// Quantization params for int8 mode
uniform float u_w_scale;
uniform float u_a_scale;

out float frag_output;

void main() {
    int out_idx = int(gl_FragCoord.x);   // output feature (0..M-1)
    int batch_idx = int(gl_FragCoord.y); // batch index (0..batch-1)
    
    float sum = 0.0;
    
    if (u_mode == 0 || u_mode == 1) {
        // Float mode: direct texture sample + multiply
        for (int k = 0; k < u_K; k++) {
            // Sample W[out_idx][k]: row=out_idx, col=k
            vec2 w_uv = vec2(
                (float(k) + 0.5) / float(u_Ww),
                (float(out_idx) + 0.5) / float(u_Wh)
            );
            float w = texture(u_weights, w_uv).r;
            
            // Sample A[batch_idx][k]: row=batch_idx, col=k
            vec2 a_uv = vec2(
                (float(k) + 0.5) / float(u_K),
                (float(batch_idx) + 0.5) / float(u_batch)
            );
            float a = texture(u_input, a_uv).r;
            
            sum += w * a;
        }
    } else if (u_mode == 2) {
        // Int8 quantized mode: 4 values per RGBA texel
        // Weight texel: each RGBA channel = 1 int8 value
        for (int k4 = 0; k4 < u_K; k4 += 4) {
            // Sample weight block at (k4/4, out_idx)
            vec2 w_uv = vec2(
                (float(k4 / 4) + 0.5) / float(max(1, u_Ww / 4)),
                (float(out_idx) + 0.5) / float(u_Wh)
            );
            vec4 w_raw = texture(u_weights, w_uv);
            // Dequantize: signed int8 stored as uint8: (val - 128) * scale
            vec4 w = (w_raw * 255.0 - 128.0) * u_w_scale;
            
            // Sample input block
            vec2 a_uv = vec2(
                (float(k4 / 4) + 0.5) / float(max(1, u_K / 4)),
                (float(batch_idx) + 0.5) / float(u_batch)
            );
            vec4 a_raw = texture(u_input, a_uv);
            vec4 a = (a_raw * 255.0 - 128.0) * u_a_scale;
            
            sum += dot(w, a);
        }
    }
    
    frag_output = sum;
}
