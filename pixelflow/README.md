# Pixelflow

LLM inference using GPU fragment shaders instead of CUDA/compute kernels. Model weights are stored as GPU textures; inference is a chain of fullscreen render passes where each pixel computes one output element.

## Status: Working

**GPT-2 (124M) generates coherent text via fragment shaders on RTX 5090.**

```
=== Performance ===
engine_v5  (hybrid CPU/GPU):  ~16ms/token
engine_v6b (all-GPU fused):  ~10ms/token, 100 tok/s, 0.3ms std

=== Accuracy ===
v5  vs PyTorch:    correlation 1.000000 (identical)
v6b vs PyTorch:    correlation 0.973 (top-3 tokens match)

=== BC4 Hardware Compression ===
8x VRAM reduction (500MB -> 62MB)
Hardware-native dequantization via GPU texture units
0.983 correlation per-layer, top-1 prediction preserved
```

## Architecture

```
Input tokens
    |
[Embedding lookup -- CPU]
    |
    v
+---Transformer Block x12---+
|  LN1 (fused shader)       |
|  QKV projection (matmul)  |
|  Attention (V extract)    |
|  Output projection        |
|  Residual add             |
|  LN2 (fused shader)       |
|  FC (768->3072 matmul)    |
|  GELU (shader)            |
|  MLP proj (3072->768)     |
|  Residual add             |
+----------------------------+
    |
    v
[Final LN -> LM Head]
    |
Logits (50257-dim)
```

Each operation is a fullscreen quad render pass. Weight matrices are GPU textures. The fragment shader for matmul reads one row from the weight texture and one column from the input texture, computing a dot product per pixel.

## Engine Versions

| Version | Approach | Speed | Accuracy |
|---------|----------|-------|----------|
| v5 | Hybrid: GPU matmul + CPU non-linear | 16ms | 1.000 |
| v6b | All-GPU with fused shaders | 10ms | 0.973 |

## BC4 Compression Thesis (Validated)

Storing weights as BC4 compressed textures gives:
- 8x VRAM reduction (32-bit float -> 4-bit BC4)
- Hardware-accelerated decompression via GPU texture units
- Zero compute cost for dequantization
- Comparable accuracy to Q4 quantization (0.983 corr)

## Files

- `pixelflow/engine_v5.py` -- Hybrid CPU/GPU pipeline (proven correct)
- `pixelflow/engine_v6b.py` -- All-GPU fused pipeline (fastest)
- `pixelflow/engine_v2.py` -- Low-level shader infrastructure
- `pixelflow/export_weights.py` -- GPT-2 weight export from HuggingFace
- `pixelflow/shaders/` -- GLSL fragment shaders
- `test_bc4.py` -- BC4 hardware compression validation

## Key Insight

The BC4 thesis is the novel contribution. Current engines (llama.cpp, vLLM) dequantize 4-bit weights in software or compute shaders. Pixelflow encodes weights in BC4/BC5 GPU texture formats, and the GPU's dedicated texture decompression silicon handles dequantization for free during the texture fetch. This saves both memory bandwidth and compute.

## Run

```bash
cd ~/zion/projects/geometry_os/geometry_os/pixelflow

# Export weights (first time only)
python3 pixelflow/export_weights.py

# Run all-GPU inference
__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia python3 -c "
from pixelflow.engine_v6b import GPT2V6B
engine = GPT2V6B()
engine.generate('The meaning of life is', max_new_tokens=20)
"

# Run BC4 test
__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia python3 test_bc4.py
```

## Next Steps

1. **Full BC4 pipeline**: Encode all 149 weight matrices as BC4, run end-to-end inference
2. **WebGL 2.0 port**: Browser inference on any device (no CUDA, no WebGPU needed)
3. **Larger models**: Test with GPT-2 Medium (355M) and Phi-2 (2.7B)
4. **KV cache**: Multi-token prompt processing without re-computation
