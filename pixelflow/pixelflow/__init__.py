"""pixelflow -- GPU fragment shader inference with texture-encoded weights.

Stores LLM weights as GPU textures and performs inference via fragment shaders.
Thesis: hardware texture compression (BCn/ASTC) gives free weight decompression,
reducing memory bandwidth -- the primary bottleneck for LLM inference.
"""

__version__ = "0.1.0"
