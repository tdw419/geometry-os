# pixelflow

GPU fragment shader inference engine for LLMs. Stores model weights as GPU textures, performs inference via fragment shaders.

## What It Does

- Encodes weight matrices as OpenGL textures (float32, float16, int8)
- Matrix multiply via fragment shader (each pixel computes one output element)
- Pre-allocated multi-pass pipeline for chaining layers
- Supports NVIDIA and Mesa GPUs

## Benchmark Results (RTX 5090 Laptop)

| Layer | Pixelflow | PyTorch CUDA | Ratio |
|---|---|---|---|
| 768x768 (attn Q/K/V) | 0.14ms | 0.012ms | 11.7x |
| 768x2304 (MLP concat) | 0.31ms | 0.015ms | 20.5x |
| 2304x768 (MLP proj) | 0.14ms | 0.014ms | 10.0x |
| 768x50257 (lm_head) | 1.39ms | 0.29ms | **4.8x** |

Numerical accuracy: zero error on small layers, <0.001 max error on large layers.

## Key Findings

1. **Fragment shader matmul is 5-20x slower than CUDA tensor cores** on the same GPU. Expected -- tensor cores are purpose-built hardware.

2. **The gap narrows with size**: 11.7x at 768x768, 4.8x at 768x50257. Fragment shaders scale well with texture size.

3. **WebGL is the real opportunity**: Fragment shaders run on ANY GPU, in ANY browser, without WebGPU. A 124M param model at Q4 = 62MB textures. Target: 3-5 tok/s on any device.

4. **Hardware texture compression thesis**: BC4/BC5 provide 4:1 compression with dedicated decompression silicon. Storing Q4 weights in BC4 textures gives "free" dequantization. This is architecturally novel vs llama.cpp.

## Architecture

```
pixelflow/
  engine.py          -- v1 engine (headless moderngl)
  engine_v2.py       -- v2 engine (pygame+NVIDIA, pre-allocated pipeline)
  weight_textures.py -- Quantization (int8, int4) and texture encoding
  benchmark.py       -- CLI benchmark tool
  shaders/
    matmul.glsl      -- Matrix multiply fragment shader
tests/
  test_matmul.py     -- Correctness and accuracy tests
```

## Usage

```python
from pixelflow.engine_v2 import ShaderInference
import numpy as np

engine = ShaderInference(nvidia=True)
engine.upload_weights("layer1", np.random.randn(768, 768).astype(np.float32))
output = engine.linear("layer1", np.random.randn(768).astype(np.float32))
engine.cleanup()
```

## GPT-2 Pixel Inference Demo

The engine now supports a functional (though incomplete) GPT-2 inference pipeline:
- [x] Weight export from HuggingFace `gpt2`
- [x] Multi-pass fragment shader orchestration
- [x] MLP Block: `Linear -> GELU -> Linear` running natively on GPU
- [x] LM Head: Vocabulary projection running natively on GPU
- [x] Tokenizer integration via `tiktoken`

### Running the Demo
```bash
cd pixelflow
# 1. Export weights (requires torch/transformers)
python3 pixelflow/export_weights.py
# 2. Run generation demo (RTX 5090)
__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia python3 -m pixelflow.engine_v3
```

### Current Opcode Set (GLSL)
- `matmul.glsl`: Standard matrix-vector multiply
- `layernorm.glsl`: Row-wise normalization
- `gelu.glsl`: Gaussian Error Linear Unit activation
- `softmax.glsl`: Exponent-based normalization
- `reduce_stats.glsl`: One-pass mean/variance calculation
