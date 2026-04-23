"""BC4 test: ctypes direct call to glCompressedTexImage2D."""
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

# Get glCompressedTexImage2D via ctypes (bypass PyOpenGL's broken wrapper)
libgl = ctypes.CDLL("libOpenGL.so.0")
_glCompressedTexImage2D = libgl.glCompressedTexImage2D
_glCompressedTexImage2D.restype = None
_glCompressedTexImage2D.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.c_uint,
    ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p]

GL_COMPRESSED_RED_RGTC1 = 0x8DBB

def encode_bc4_block(vals):
    v = np.clip(vals.flatten() * 255, 0, 255).astype(int)
    r0, r1 = int(v.max()), int(v.min())
    if r0 == r1: r0 = min(r0 + 1, 255)
    if r0 > r1:
        pal = [r0, r1, (6*r0+1*r1)//7, (5*r0+2*r1)//7, (4*r0+3*r1)//7,
               (3*r0+4*r1)//7, (2*r0+5*r1)//7, (1*r0+6*r1)//7]
    else:
        pal = [r0, r1, 0, 0, 0, 0, 0, 255]
    indices = [min(range(8), key=lambda i: abs(v[j] - pal[i])) for j in range(16)]
    packed = bytearray(8)
    packed[0], packed[1] = r0, r1
    bits = sum(idx << (i * 3) for i, idx in enumerate(indices))
    for i in range(6): packed[2+i] = (bits >> (i*8)) & 0xFF
    return bytes(packed)

def mksh(src, typ):
    s = GL.glCreateShader(typ)
    GL.glShaderSource(s, [src]); GL.glCompileShader(s)
    if not GL.glGetShaderiv(s, GL.GL_COMPILE_STATUS):
        print(GL.glGetShaderInfoLog(s).decode())
    return s

# Load weight
w = np.load("gpt2_weights/transformer_h_0_attn_c_proj_weight.npy")
wt = w.T
w_min, w_max, w_std = wt.min(), wt.max(), wt.std()
W = H = 768
print(f"Weight: range=[{w_min:.4f}, {w_max:.4f}], std={w_std:.4f}")

# Encode BC4
ws = (wt - w_min) / (w_max - w_min)
comp = bytearray()
for by in range(0, H, 4):
    for bx in range(0, W, 4):
        comp.extend(encode_bc4_block(ws[by:by+4, bx:bx+4]))
comp = bytes(comp)
print(f"F32: {wt.nbytes:,}B -> BC4: {len(comp):,}B ({wt.nbytes//len(comp)}x)")

# Upload compressed texture (raw ctypes)
tex = GL.glGenTextures(1)
GL.glBindTexture(GL.GL_TEXTURE_2D, tex)
buf = (ctypes.c_ubyte * len(comp))(*comp)
_glCompressedTexImage2D(GL.GL_TEXTURE_2D, 0, GL_COMPRESSED_RED_RGTC1, W, H, 0, len(comp), buf)
GL.glTexParameteri(GL.GL_TEXTURE_2D, GL.GL_TEXTURE_MIN_FILTER, GL.GL_NEAREST)
GL.glTexParameteri(GL.GL_TEXTURE_2D, GL.GL_TEXTURE_MAG_FILTER, GL.GL_NEAREST)
print("BC4 texture uploaded!")

# Read back via FBO
fbo = GL.glGenFramebuffers(1)
GL.glBindFramebuffer(GL.GL_FRAMEBUFFER, fbo)
ot = GL.glGenTextures(1)
GL.glBindTexture(GL.GL_TEXTURE_2D, ot)
GL.glTexImage2D(GL.GL_TEXTURE_2D, 0, GL.GL_R32F, W, H, 0, GL.GL_RED, GL.GL_FLOAT, None)
GL.glFramebufferTexture2D(GL.GL_FRAMEBUFFER, GL.GL_COLOR_ATTACHMENT0, GL.GL_TEXTURE_2D, ot, 0)

vs = mksh(b"#version 450\nvoid main(){float x=-1.+float(gl_VertexID%2)*2.;float y=-1.+float(gl_VertexID/2)*2.;gl_Position=vec4(x,y,0,1);}", GL.GL_VERTEX_SHADER)
fs = mksh(b"#version 450\nuniform sampler2D u;uniform vec2 sz;out float o;void main(){o=texture(u,gl_FragCoord.xy/sz).r;}", GL.GL_FRAGMENT_SHADER)
prg = GL.glCreateProgram()
GL.glAttachShader(prg, vs); GL.glAttachShader(prg, fs)
GL.glLinkProgram(prg); GL.glUseProgram(prg)
GL.glUniform1i(GL.glGetUniformLocation(prg, b"u"), 0)
GL.glUniform2f(GL.glGetUniformLocation(prg, b"sz"), float(W), float(H))
GL.glActiveTexture(GL.GL_TEXTURE0)
GL.glBindTexture(GL.GL_TEXTURE_2D, tex)
GL.glViewport(0, 0, W, H)
GL.glDrawArrays(GL.GL_TRIANGLE_STRIP, 0, 4)

res = (ctypes.c_float * (W * H))()
GL.glReadPixels(0, 0, W, H, GL.GL_RED, GL.GL_FLOAT, res)
decoded = np.array(res, dtype=np.float32).reshape(H, W) * (w_max - w_min) + w_min

err = np.abs(decoded - wt)
print(f"\n=== BC4 Hardware Decompression Accuracy ===")
print(f"  Mean error:  {err.mean():.6f}")
print(f"  Max error:   {err.max():.6f}")
print(f"  RMSE:        {np.sqrt((err**2).mean()):.6f}")
print(f"  Error/std:   {err.mean()/w_std*100:.2f}%")
print(f"  Correlation: {np.corrcoef(decoded.flatten(), wt.flatten())[0,1]:.6f}")

# Now benchmark: BC4 texture sampling vs F32 texture sampling
# Time 1000 texture samples
import moderngl
ctx = moderngl.create_context()

# F32 baseline
f32_tex = ctx.texture((W, H), components=1, dtype='f4')
f32_tex.write(wt.astype(np.float32).tobytes())

# Benchmark texture read bandwidth
t0 = time.perf_counter()
for _ in range(10):
    _ = f32_tex.read()
f32_time = time.perf_counter() - t0
f32_bandwidth = (wt.nbytes * 10) / f32_time / 1e9

print(f"\n=== Bandwidth ===")
print(f"  F32 read: {f32_time*100:.1f}ms ({f32_bandwidth:.2f} GB/s)")

# Inference with BC4 layer
from pixelflow.engine_v5 import GPT2Engine
engine = GPT2Engine()
tokens = engine.tokenizer.encode("The")
orig = engine.forward(tokens, use_gpu=False)
ow = engine.weights["transformer_h_0_attn_c_proj_weight"].copy()
engine.weights["transformer_h_0_attn_c_proj_weight"] = decoded.T
bc4 = engine.forward(tokens, use_gpu=False)
print(f"\n=== Inference Impact ===")
print(f"  Original top: {[engine.tokenizer.decode([t]) for t in np.argsort(orig)[-5:][::-1]]}")
print(f"  BC4 top:      {[engine.tokenizer.decode([t]) for t in np.argsort(bc4)[-5:][::-1]]}")
print(f"  Same top-1:   {np.argsort(orig)[-1] == np.argsort(bc4)[-1]}")
print(f"  Logit MAE:    {np.abs(bc4-orig).mean():.4f}")

print("\n=== BC4 Generation ===")
engine.generate("The future of", max_new_tokens=8, use_gpu=False)

GL.glBindFramebuffer(GL.GL_FRAMEBUFFER, 0)
pygame.quit()
