# Pixelflow

LLM inference using GPU fragment shaders instead of CUDA/compute kernels. Model weights are stored as GPU textures; inference is a chain of fullscreen render passes where each pixel computes one activation value.

## Status: Working

GPT-2 (124M) inference via fragment shaders produces output identical to PyTorch (correlation 1.000000, max error 0.015).

## Quick Start

```bash
cd pixelflow

# 1. Export weights from HuggingFace
python3 pixelflow/export_weights.py

# 2. Run inference
__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia python3 -m pixelflow.engine_v5
```

## Sample Output

```
Prompt: "Once upon a time"
Output: "Once upon a time it was clear that this was an opportunity for the 
president to make good on his promise to repeal Obamacare. As it turned out, 
that promise was never..."
```

## Architecture

```
Weights (numpy) --upload--> GPU Textures
                              |
Input (tokens) --embed--> Activations (CPU)
                              |
          ┌───────────────────┘
          │  For each transformer layer (x12):
          │    LayerNorm ────── CPU
          │    QKV Project ──── GPU matmul (fragment shader)
          │    Attention ────── CPU (head split, softmax, combine)
          │    Out Project ──── GPU matmul
          │    Residual Add ─── CPU
          │    LayerNorm ────── CPU
          │    FC Project ───── GPU matmul (768 -> 3072)
          │    GELU ─────────── CPU
          │    Proj Back ────── GPU matmul (3072 -> 768)
          │    Residual Add ─── CPU
          └───────────────────┐
                              |
         Final LN ── CPU ── LM Head (GPU matmul, 768 -> 50257)
                              |
                        Token Sampling (CPU)
```

## The Matmul Shader

Each matrix multiply is a single render pass:
- Weight matrix W (M x K) stored as a GPU texture
- Input vector x (1 x K) uploaded as a 1-pixel-tall texture
- Output texture (M x 1) rendered via fullscreen quad
- Each output pixel (x=0..M-1) computes: `sum(weight[k,j] * input[k])` for k=0..K-1

```glsl
for (int k = 0; k < u_K; k++) {
    float w = texture(u_weights, weight_uv(k, out_idx)).r;
    float a = texture(u_input, input_uv(k, batch_idx)).r;
    sum += w * a;
}
frag_output = sum;
```

## Performance (RTX 5090 Laptop)

| Metric | Value |
|--------|-------|
| Full forward pass (seq_len=1) | 16ms |
| vs NumPy CPU | 1.37x faster |
| vs PyTorch CUDA | ~5x slower (expected) |
| Logits correlation | 1.000000 |
| Max element error | 0.015 |

The 16ms includes CPU-GPU round trips for each matmul. A fully-GPU pipeline (no readbacks) would be significantly faster.

## Why Fragment Shaders?

1. **Universal GPU access** -- Every GPU since OpenGL 2.0 can render pixels. No CUDA, no compute shaders, no vendor lock-in.
2. **WebGL potential** -- This architecture translates directly to WebGL 2.0, enabling browser-based LLM inference on any device.
3. **Hardware texture compression** -- BC4/BC5/ASTC formats decompress in dedicated silicon. Storing quantized weights in these formats gives "free" 4:1 bandwidth savings.
4. **Geometry OS integration** -- Weights as textures. Inference as rendering. The screen IS the hard drive.

## Files

- `engine_v5.py` -- Current working engine (GPU matmuls + CPU non-linear ops)
- `engine_v4.py` -- Full GPU pipeline (has texture state corruption bug)
- `engine_v2.py` -- Base shader infrastructure
- `export_weights.py` -- Download GPT-2 weights from HuggingFace
- `shaders/` -- GLSL fragment shaders (matmul, layernorm, gelu, softmax, etc.)
- `webgl/index.html` -- Browser-based matmul prototype

## Next Steps

- [ ] Fix engine_v4's texture state corruption (GPU-only pipeline, no CPU round-trips)
- [ ] BC4/BC5 compressed weight textures (verify "free decompression" thesis)
- [ ] WebGL 2.0 full transformer (browser inference)
- [ ] KV-cache for multi-token generation
- [ ] Larger models (GPT-2 medium, phi-2)
