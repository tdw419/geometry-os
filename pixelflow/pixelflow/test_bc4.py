"""BC4 compressed texture test: encode weights, upload, sample, measure accuracy."""
import os, ctypes, time
import numpy as np
os.environ['__NV_PRIME_RENDER_OFFLOAD'] = '1'
os.environ['__GLX_VENDOR_LIBRARY_NAME'] = 'nvidia'

import pygame
pygame.init()
pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MAJOR_VERSION, 4)
pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MINOR_VERSION, 6)
screen = pygame.display.set_mode((1, 1), pygame.OPENGL | pygame.HIDDEN)

from OpenGL import GL
print(f"GPU: {GL.glGetString(GL.GL_RENDERER).decode()}")

GL_COMPRESSED_RED_RGTC1 = 0x8DBB

def encode_bc4_block(values_4x4):
    """Encode a 4x4 block of float32 [0,1] values into BC4 unsigned (8 bytes)."""
    v_uint8 = np.clip(values_4x4.flatten() * 255.0, 0, 255).astype(int)
    
    r0 = int(v_uint8.max())  # endpoint 0 (max)
    r1 = int(v_uint8.min())  # endpoint 1 (min)
    if r0 == r1:
        r0 = min(r0 + 1, 255)
    
    # Palette: 8 values interpolated between r0 and r1
    if r0 > r1:
        palette = [
            r0, r1,
            (6*r0 + 1*r1) // 7,
            (5*r0 + 2*r1) // 7,
            (4*r0 + 3*r1) // 7,
            (3*r0 + 4*r1) // 7,
            (2*r0 + 5*r1) // 7,
            (1*r0 + 6*r1) // 7,
        ]
    else:
        palette = [r0, r1, 0, 0, 0, 0, 0, 255]
    
    # Find nearest palette index for each pixel
    indices = []
    for v in v_uint8:
        best = 0
        best_d = abs(v - palette[0])
        for i in range(1, 8):
            d = abs(v - palette[i])
            if d < best_d:
                best_d = d
                best = i
        indices.append(best)
    
    # Pack: r0(1) + r1(1) + 16 indices * 3 bits = 48 bits = 6 bytes
    packed = bytearray(8)
    packed[0] = r0
    packed[1] = r1
    
    all_bits = 0
    for i, idx in enumerate(indices):
        all_bits |= (idx << (i * 3))
    
    for i in range(6):
        packed[2 + i] = (all_bits >> (i * 8)) & 0xFF
    
    return bytes(packed)

def encode_bc4_texture(data_2d):
    """Encode float32 [0,1] 2D array as BC4 compressed data."""
    H, W = data_2d.shape
    W4 = ((W + 3) // 4) * 4
    H4 = ((H + 3) // 4) * 4
    padded = np.zeros((H4, W4), dtype=np.float32)
    padded[:H, :W] = data_2d
    
    blocks = bytearray()
    for by in range(0, H4, 4):
        for bx in range(0, W4, 4):
            blocks.extend(encode_bc4_block(padded[by:by+4, bx:bx+4]))
    
    return bytes(blocks), W4, H4

def compile_shader(src, shader_type):
    s = GL.glCreateShader(shader_type)
    GL.glShaderSource(s, [src])
    GL.glCompileShader(s)
    if GL.glGetShaderiv(s, GL.GL_COMPILE_STATUS) != GL.GL_TRUE:
        print(f"Shader error: {GL.glGetShaderInfoLog(s).decode()}")
        return None
    return s

# Load real weights
print("\n=== BC4 Compression Test ===")
w = np.load("gpt2_weights/transformer_h_0_attn_c_proj_weight.npy")  # (768, 768)
wt = w.T

w_min, w_max = wt.min(), wt.max()
w_std = wt.std()
print(f"Weight: {wt.shape}, range=[{w_min:.4f}, {w_max:.4f}], std={w_std:.4f}")

# Scale to [0, 1]
w_scaled = (wt - w_min) / (w_max - w_min)

# Encode BC4
t0 = time.perf_counter()
compressed, W4, H4 = encode_bc4_texture(w_scaled)
t_enc = time.perf_counter() - t0

original_bytes = wt.nbytes
compressed_bytes = len(compressed)
print(f"\nOriginal: {original_bytes:,} bytes (F32)")
print(f"BC4:      {compressed_bytes:,} bytes ({t_enc:.3f}s to encode)")
print(f"Ratio:    {original_bytes / compressed_bytes:.1f}x")

# Upload compressed texture via raw GL
tex_id = GL.glGenTextures(1)
GL.glBindTexture(GL.GL_TEXTURE_2D, tex_id)

try:
    GL.glCompressedTexImage2D(
        GL.GL_TEXTURE_2D, 0, GL_COMPRESSED_RED_RGTC1,
        W4, H4, 0, compressed_bytes, compressed
    )
    print("Compressed texture uploaded OK!")
except Exception as e:
    print(f"Upload failed: {e}")
    pygame.quit()
    exit(1)

GL.glTexParameteri(GL.GL_TEXTURE_2D, GL.GL_TEXTURE_MIN_FILTER, GL.GL_NEAREST)
GL.glTexParameteri(GL.GL_TEXTURE_2D, GL.GL_TEXTURE_MAG_FILTER, GL.GL_NEAREST)

# Create FBO + render to sample the BC4 texture
fbo = GL.glGenFramebuffers(1)
GL.glBindFramebuffer(GL.GL_FRAMEBUFFER, fbo)

out_tex = GL.glGenTextures(1)
GL.glBindTexture(GL.GL_TEXTURE_2D, out_tex)
GL.glTexImage2D(GL.GL_TEXTURE_2D, 0, GL.GL_R32F, W4, H4, 0, GL.GL_RED, GL.GL_FLOAT, None)
GL.glFramebufferTexture2D(GL.GL_FRAMEBUFFER, GL.GL_COLOR_ATTACHMENT0, GL.GL_TEXTURE_2D, out_tex, 0)

vs = compile_shader(b"""#version 450
void main() {
    float x = -1.0 + float(gl_VertexID % 2) * 2.0;
    float y = -1.0 + float(gl_VertexID / 2) * 2.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}""", GL.GL_VERTEX_SHADER)

fs = compile_shader(b"""#version 450
uniform sampler2D u_tex;
uniform vec2 u_size;
out float frag_output;
void main() {
    vec2 uv = gl_FragCoord.xy / u_size;
    frag_output = texture(u_tex, uv).r;
}""", GL.GL_FRAGMENT_SHADER)

prog = GL.glCreateProgram()
GL.glAttachShader(prog, vs)
GL.glAttachShader(prog, fs)
GL.glLinkProgram(prog)
GL.glUseProgram(prog)

GL.glUniform1i(GL.glGetUniformLocation(prog, b"u_tex"), 0)
GL.glUniform2f(GL.glGetUniformLocation(prog, b"u_size"), float(W4), float(H4))
GL.glActiveTexture(GL.GL_TEXTURE0)
GL.glBindTexture(GL.GL_TEXTURE_2D, tex_id)

GL.glViewport(0, 0, W4, H4)
GL.glDrawArrays(GL.GL_TRIANGLE_STRIP, 0, 4)

# Read back decoded values
result = (ctypes.c_float * (W4 * H4))()
GL.glReadPixels(0, 0, W4, H4, GL.GL_RED, GL.GL_FLOAT, result)
decoded_scaled = np.array(result, dtype=np.float32).reshape(H4, W4)

# Unscale from [0,1] to original range
decoded = decoded_scaled * (w_max - w_min) + w_min

# Accuracy metrics
err = np.abs(decoded[:768, :768] - wt)
print(f"\n=== BC4 Accuracy ===")
print(f"  Mean error: {err.mean():.6f}")
print(f"  Max error:  {err.max():.6f}")
print(f"  RMSE:       {np.sqrt((err**2).mean()):.6f}")
print(f"  Weight std: {w_std:.6f}")
print(f"  Error/std:  {err.mean()/w_std*100:.2f}%")
corr = np.corrcoef(decoded[:768,:768].flatten(), wt.flatten())[0, 1]
print(f"  Correlation: {corr:.6f}")

# Now test: does inference with BC4-quantized weights still produce coherent output?
# Replace one weight in engine_v5 with BC4-decoded version and compare
print(f"\n=== Inference Impact ===")
# Run engine_v5 CPU mode with original vs BC4-decoded weights
import sys
sys.path.insert(0, '.')
from pixelflow.engine_v5 import GPT2Engine

engine = GPT2Engine()
tokens = engine.tokenizer.encode("The")

# Original logits
orig_logits = engine.forward(tokens, use_gpu=False)

# Replace the weight with BC4-decoded version
orig_w = engine.weights["transformer_h_0_attn_c_proj_weight"].copy()
engine.weights["transformer_h_0_attn_c_proj_weight"] = decoded[:768, :768].T  # un-transpose

bc4_logits = engine.forward(tokens, use_gpu=False)

# Compare
logit_diff = np.abs(bc4_logits - orig_logits)
top_orig = np.argsort(orig_logits)[-5:][::-1]
top_bc4 = np.argsort(bc4_logits)[-5:][::-1]

print(f"  Original top 5: {[engine.tokenizer.decode([t]) for t in top_orig]}")
print(f"  BC4     top 5: {[engine.tokenizer.decode([t]) for t in top_bc4]}")
print(f"  Logit MAE: {logit_diff.mean():.4f}")
print(f"  Logit max diff: {logit_diff.max():.4f}")

# Restore
engine.weights["transformer_h_0_attn_c_proj_weight"] = orig_w

# Cleanup GL
GL.glBindFramebuffer(GL.GL_FRAMEBUFFER, 0)
GL.glDeleteFramebuffers(1, [fbo])
GL.glDeleteTextures([tex_id, out_tex])
GL.glDeleteProgram(prog)

pygame.quit()
