"""Encode model weights as GPU textures for fragment shader inference.

Strategies:
  - FLOAT16: 2 pixels per float32 weight (R=high16, G=low16 packed into RGBA)
  - Q8:     1 pixel per 4 int8 weights (RGBA channels)
  - Q4:     1 pixel per 8 nibbles (packed into RGBA)
  - BC4:    Hardware compressed, 1 block per 4x4 texels, single channel
  - BC5:    Hardware compressed, 2 channels, for paired weights
"""

import numpy as np
import moderngl
import struct


def encode_float32_to_rgba_f16(weights: np.ndarray) -> tuple[np.ndarray, tuple[int, int]]:
    """Encode float32 weights as RGBA16F texture data (2 pixels per weight).
    
    Each float32 is split into two float16 values (high and low halves),
    then packed into RGBA16F pixel format.
    
    Args:
        weights: 1D float32 array of model weights
        
    Returns:
        (texture_data, (width, height)) -- uint16 array for texture upload
    """
    # Pad to multiple of 4 (RGBA groups)
    n = len(weights)
    padded = np.zeros(n + (4 - n % 4) % 4, dtype=np.float32)
    padded[:n] = weights
    
    # Convert to float16 pairs for lossy, or use fp32 splitting
    # For now: direct float16 with some precision loss
    f16 = padded.astype(np.float16)
    
    # Pack as RG16F (each pixel = 2 float16 values)
    # ModernGL RG16F = 2 channels x 16 bits
    data = np.array(f16, dtype=np.uint16)
    
    # Reshape to 2D texture
    total_pixels = len(data) // 2  # 2 channels per pixel
    width = min(total_pixels, 4096)
    height = max(1, (total_pixels + width - 1) // width)
    
    # Pad to fill the texture
    needed = width * height * 2
    padded_data = np.zeros(needed, dtype=np.uint16)
    padded_data[:len(data)] = data
    
    return padded_data, (width, height)


def encode_q8_to_rgba(weights_int8: np.ndarray) -> tuple[np.ndarray, tuple[int, int]]:
    """Encode int8 quantized weights as RGBA8 texture data (4 weights per pixel).
    
    Args:
        weights_int8: int8 array of quantized weights (values -128 to 127)
        
    Returns:
        (texture_data, (width, height)) -- uint8 array for texture upload
    """
    n = len(weights_int8)
    # Pad to multiple of 4
    padded = np.zeros(n + (4 - n % 4) % 4, dtype=np.int8)
    padded[:n] = weights_int8
    
    # Reinterpret as uint8 for GPU
    data = padded.view(np.uint8)
    
    # Reshape to 2D texture (4 channels per pixel = RGBA)
    total_pixels = len(data) // 4
    width = min(total_pixels, 4096)
    height = max(1, (total_pixels + width - 1) // width)
    
    needed = width * height * 4
    padded_data = np.zeros(needed, dtype=np.uint8)
    padded_data[:len(data)] = data
    
    return padded_data, (width, height)


def encode_q4_to_rg8(weights_nibbles: np.ndarray) -> tuple[np.ndarray, tuple[int, int]]:
    """Encode 4-bit quantized weights as RG8 texture (4 nibbles per pixel).
    
    Each pixel packs 4 nibbles: R=(hi0,lo0), G=(hi1,lo1)
    
    Args:
        weights_nibbles: uint8 array where each value is 0-15 (4-bit)
        
    Returns:
        (texture_data, (width, height)) -- uint8 array
    """
    n = len(weights_nibbles)
    # Pad to multiple of 4
    padded = np.zeros(n + (4 - n % 4) % 4, dtype=np.uint8)
    padded[:n] = weights_nibbles
    
    # Pack 2 nibbles per byte: (n0 << 4) | n1
    pairs = (padded[0::2] << 4) | padded[1::2]
    
    # Reshape: 2 channels (RG) per pixel, 2 bytes per pixel
    total_pixels = len(pairs) // 2
    width = min(total_pixels, 4096)
    height = max(1, (total_pixels + width - 1) // width)
    
    needed = width * height * 2
    padded_data = np.zeros(needed, dtype=np.uint8)
    padded_data[:len(pairs)] = pairs
    
    return padded_data, (width, height)


def weight_matrix_to_texture(
    ctx: moderngl.Context,
    matrix: np.ndarray,
    mode: str = "float32"
) -> moderngl.Texture:
    """Upload a weight matrix as a GPU texture.
    
    Args:
        ctx: ModernGL context
        matrix: 2D numpy array (out_features x in_features)
        mode: "float32", "float16", "q8", or "q4"
        
    Returns:
        ModernGL Texture object
    """
    flat = matrix.flatten()
    
    if mode == "float32":
        # Use R32F texture -- simplest, 1 float per pixel
        width = min(len(flat), 4096)
        height = max(1, (len(flat) + width - 1) // width)
        padded = np.zeros(width * height, dtype=np.float32)
        padded[:len(flat)] = flat
        tex = ctx.texture((width, height), components=1, dtype="f4")
        tex.write(padded.tobytes())
        
    elif mode == "float16":
        # Use R16F texture
        f16 = flat.astype(np.float16)
        width = min(len(f16), 4096)
        height = max(1, (len(f16) + width - 1) // width)
        padded = np.zeros(width * height, dtype=np.float16)
        padded[:len(f16)] = f16
        tex = ctx.texture((width, height), components=1, dtype="f2")
        tex.write(padded.tobytes())
        
    elif mode == "q8":
        # 4 int8 values per RGBA pixel
        q_weights = quantize_to_int8(flat)
        data, (w, h) = encode_q8_to_rgba(q_weights)
        tex = ctx.texture((w, h), components=4, dtype="u1")
        tex.write(data.tobytes())
        
    elif mode == "q4":
        q_weights = quantize_to_int4(flat)
        data, (w, h) = encode_q4_to_rg8(q_weights)
        tex = ctx.texture((w, h), components=2, dtype="u1")
        tex.write(data.tobytes())
        
    else:
        raise ValueError(f"Unknown mode: {mode}")
    
    return tex


def quantize_to_int8(weights: np.ndarray, symmetric: bool = True) -> tuple:
    """Quantize float32 weights to int8.
    
    Returns:
        (int8_weights, scale, zero_point) -- need these for dequantization
    """
    wmin = weights.min()
    wmax = weights.max()
    
    if symmetric:
        # Symmetric quantization: zero_point = 0, range = max(|w|)
        amax = max(abs(wmin), abs(wmax))
        scale = amax / 127.0
        zero_point = 0
        q = np.clip(np.round(weights / scale), -128, 127).astype(np.int8)
    else:
        # Asymmetric quantization
        scale = (wmax - wmin) / 255.0
        zero_point = int(np.round(-wmin / scale))
        q = np.clip(np.round(weights / scale + zero_point), 0, 255).astype(np.uint8)
    
    return q, scale, zero_point


def quantize_to_int4(weights: np.ndarray) -> tuple:
    """Quantize float32 weights to 4-bit (0-15 range).
    
    Returns:
        (uint4_weights, scale, zero_point) -- values 0-15
    """
    wmin = weights.min()
    wmax = weights.max()
    scale = (wmax - wmin) / 15.0
    zero_point = 0  # for symmetric-ish
    q = np.clip(np.round((weights - wmin) / scale), 0, 15).astype(np.uint8)
    return q, scale, wmin  # wmin is the offset for dequant


def dequantize_int8(q_weights: np.ndarray, scale: float, zero_point: int = 0) -> np.ndarray:
    """Dequantize int8 weights back to float32."""
    return (q_weights.astype(np.float32) - zero_point) * scale


def dequantize_int4(q_weights: np.ndarray, scale: float, offset: float) -> np.ndarray:
    """Dequantize 4-bit weights back to float32."""
    return q_weights.astype(np.float32) * scale + offset
