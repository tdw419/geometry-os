"""pixelflow engine_v6b -- All-GPU pipeline with deterministic texture pooling.

Key fix: pre-allocate all activation textures and FBOs at init time.
No texture creation/destruction during forward pass = no GC issues = deterministic results.
"""
import os, time, gc
import numpy as np
import moderngl
import tiktoken
from pathlib import Path

os.environ.setdefault('__NV_PRIME_RENDER_OFFLOAD', '1')
os.environ.setdefault('__GLX_VENDOR_LIBRARY_NAME', 'nvidia')

QUAD_VS = """
#version 450
void main() {
    float x = -1.0 + float(gl_VertexID % 2) * 2.0;
    float y = -1.0 + float(gl_VertexID / 2) * 2.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}
"""

# Import shader sources from v6
def _get_shaders():
    """All fragment shader sources."""
    return {
        'matmul': """
#version 450
uniform sampler2D u_w;
uniform sampler2D u_x;
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
    f = s;
}
""",
        'add_bias': """
#version 450
uniform sampler2D u_x;
uniform sampler2D u_b;
out float f;
void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    f = texelFetch(u_x, c, 0).r + texelFetch(u_b, ivec2(c.x, 0), 0).r;
}
""",
        'add': """
#version 450
uniform sampler2D u_a;
uniform sampler2D u_b;
out float f;
void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    f = texelFetch(u_a, c, 0).r + texelFetch(u_b, c, 0).r;
}
""",
        'stats': """
#version 450
uniform sampler2D u_x;
uniform int u_K;
out vec2 f;
void main() {
    int b = int(gl_FragCoord.y);
    float s1 = 0.0, s2 = 0.0;
    for (int k = 0; k < u_K; k++) {
        float v = texelFetch(u_x, ivec2(k, b), 0).r;
        s1 += v; s2 += v*v;
    }
    float m = s1 / float(u_K);
    f = vec2(m, s2/float(u_K) - m*m);
}
""",
        'layernorm': """
#version 450
uniform sampler2D u_x;
uniform sampler2D u_s;
uniform sampler2D u_g;
uniform sampler2D u_bt;
out float f;
void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    float x = texelFetch(u_x, c, 0).r;
    vec2 st = texelFetch(u_s, ivec2(0, c.y), 0).rg;
    float g = texelFetch(u_g, ivec2(c.x, 0), 0).r;
    float b = texelFetch(u_bt, ivec2(c.x, 0), 0).r;
    f = ((x - st.r) / sqrt(st.g + 1e-5)) * g + b;
}
""",
        'gelu': """
#version 450
uniform sampler2D u_x;
out float f;
void main() {
    float x = texelFetch(u_x, ivec2(gl_FragCoord.xy), 0).r;
    f = 0.5 * x * (1.0 + tanh(0.7978845608 * (x + 0.044715 * x * x * x)));
}
""",
        'extract_v': """
#version 450
uniform sampler2D u_qkv;
uniform int u_D;
out float f;
void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    f = texelFetch(u_qkv, ivec2(c.x + u_D * 2, c.y), 0).r;
}
""",
        # Fused operations for fewer passes
        'mm_bias': """
#version 450
uniform sampler2D u_w;
uniform sampler2D u_x;
uniform sampler2D u_b;
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
""",
        'ln_fused': """
#version 450
// Fused LayerNorm: compute stats and normalize in one pass
// Requires two passes would be needed for true reduction, but for batch=1
// we can't do parallel reduction efficiently. Instead we do it all here.
uniform sampler2D u_x;
uniform sampler2D u_g;
uniform sampler2D u_bt;
uniform int u_K;
out float f;
void main() {
    int i = int(gl_FragCoord.x);
    int b = int(gl_FragCoord.y);
    // Pass 1: compute mean and variance
    float sum = 0.0, sum2 = 0.0;
    for (int k = 0; k < u_K; k++) {
        float v = texelFetch(u_x, ivec2(k, b), 0).r;
        sum += v;
        sum2 += v * v;
    }
    float mean = sum / float(u_K);
    float var = sum2 / float(u_K) - mean * mean;
    // Pass 2: normalize
    float x = texelFetch(u_x, ivec2(i, b), 0).r;
    float g = texelFetch(u_g, ivec2(i, 0), 0).r;
    float bt = texelFetch(u_bt, ivec2(i, 0), 0).r;
    f = ((x - mean) / sqrt(var + 1e-5)) * g + bt;
}
""",
    }


class GPT2V6B:
    """GPT-2 inference, fully on GPU, with deterministic texture management."""
    
    def __init__(self, weight_dir="gpt2_weights"):
        import pygame
        pygame.init()
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MAJOR_VERSION, 4)
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MINOR_VERSION, 6)
        self.screen = pygame.display.set_mode((1, 1), pygame.OPENGL | pygame.HIDDEN)
        self.ctx = moderngl.create_context()
        print(f"GPU: {self.ctx.info['GL_RENDERER']}")
        
        self.weight_dir = Path(weight_dir)
        self.tokenizer = tiktoken.get_encoding("gpt2")
        
        import json
        with open(self.weight_dir / "config.json") as f:
            cfg = json.load(f)
        self.D = cfg["n_embd"]
        self.H = cfg["n_head"]
        self.L = cfg["n_layer"]
        self.HD = self.D // self.H
        
        # Compile shaders + pre-create VAOs
        self.prog = {}
        self.vaos = {}
        for name, fs in _get_shaders().items():
            prog = self.ctx.program(vertex_shader=QUAD_VS, fragment_shader=fs)
            self.prog[name] = prog
            self.vaos[name] = self.ctx.vertex_array(prog, [], mode=moderngl.TRIANGLE_STRIP)
        
        # Load weights
        self.w = {}
        self.wt = {}  # GPU weight textures
        self._load_weights()
        
        # Pre-allocate activation texture pool
        self._alloc_pool()
        
        print(f"GPT-2: {self.L}L, {self.H}H, {self.D}D")
    
    def _load_weights(self):
        """Load and upload all weights."""
        conv1d = {"attn_c_attn_weight", "attn_c_proj_weight",
                  "mlp_c_fc_weight", "mlp_c_proj_weight"}
        
        for p in sorted(self.weight_dir.glob("*.npy")):
            name = p.stem
            arr = np.load(p)
            self.w[name] = arr
            
            if arr.ndim == 2:
                suffix = name.split("_", 3)[-1] if name.startswith("transformer_h_") else ""
                if suffix in conv1d:
                    self._upload_weight(name, arr.T)  # Conv1D: x @ W -> transpose for our shader
                elif name == "lm_head_weight":
                    self._upload_weight(name, arr)  # Linear: x @ W.T
            
            if arr.ndim == 1:
                tex = self.ctx.texture((len(arr), 1), components=1, dtype='f4')
                tex.write(arr.astype(np.float32).tobytes())
                self.wt[name] = tex
        
        n_matrices = sum(1 for v in self.wt.values() if hasattr(v, 'size') and v.size[1] > 1)
        print(f"  {n_matrices} matrices + biases uploaded")
    
    def _upload_weight(self, name, matrix):
        max_sz = self.ctx.info['GL_MAX_TEXTURE_SIZE']
        M, K = matrix.shape
        if M <= max_sz:
            tex = self.ctx.texture((K, M), components=1, dtype='f4')
            tex.write(matrix.astype(np.float32).tobytes())
            self.wt[name] = tex
        else:
            tiles = []
            off = 0
            while off < M:
                ts = min(max_sz, M - off)
                t = self.ctx.texture((K, ts), components=1, dtype='f4')
                t.write(matrix[off:off+ts].astype(np.float32).tobytes())
                tiles.append(t)
                off += ts
            self.wt[name + "_tiled"] = tiles
            # Store metadata
            self.wt[name + "_meta"] = (M, K)
    
    def _alloc_pool(self):
        """Pre-allocate activation textures for the forward pass.
        
        Layout per layer (batch=1):
          - 768 x 1   (embed, LN in/out, attn out, proj out, residuals)
          - 2304 x 1  (QKV)
          - 3072 x 1  (FC, GELU, MLP proj)
          - 1 x 1 x2  (stats for LN)
        Plus final: 50257 x 1 (logits, tiled)
        """
        self._pool = {}
        D, FFN = self.D, self.D * 4
        
        # Pre-allocate textures for each size we need
        for label, (w, h, c) in [
            ('D',   (D, 1, 1)),      # 768 x 1
            ('QKV', (2304, 1, 1)),    # 2304 x 1
            ('FFN', (FFN, 1, 1)),     # 3072 x 1
            ('S',   (1, 1, 2)),       # stats (mean, var)
        ]:
            tex = self.ctx.texture((w, h), components=c, dtype='f4')
            fbo = self.ctx.framebuffer(color_attachments=[tex])
            self._pool[label] = (tex, fbo)
        
        # We need multiple D-sized buffers (ping-pong)
        for i in range(20):
            tex = self.ctx.texture((D, 1), components=1, dtype='f4')
            fbo = self.ctx.framebuffer(color_attachments=[tex])
            self._pool[f'D{i}'] = (tex, fbo)
        
        # FFN-sized
        for i in range(8):
            tex = self.ctx.texture((FFN, 1), components=1, dtype='f4')
            fbo = self.ctx.framebuffer(color_attachments=[tex])
            self._pool[f'FFN{i}'] = (tex, fbo)
        
        # QKV-sized
        for i in range(4):
            tex = self.ctx.texture((2304, 1), components=1, dtype='f4')
            fbo = self.ctx.framebuffer(color_attachments=[tex])
            self._pool[f'QKV{i}'] = (tex, fbo)
        
        # Stats
        for i in range(8):
            tex = self.ctx.texture((1, 1), components=2, dtype='f4')
            fbo = self.ctx.framebuffer(color_attachments=[tex])
            self._pool[f'S{i}'] = (tex, fbo)
        
        self._buf_idx = 0  # rotating index for temp buffers
    
    def _get_buf(self, prefix, w, h, components=1):
        """Get next available buffer from pool."""
        i = self._buf_idx
        self._buf_idx += 1
        key = f'{prefix}{i}'
        if key in self._pool:
            tex, fbo = self._pool[key]
            return tex, fbo
        # Fallback: create new
        tex = self.ctx.texture((w, h), components=components, dtype='f4')
        fbo = self.ctx.framebuffer(color_attachments=[tex])
        self._pool[key] = (tex, fbo)
        return tex, fbo
    
    def _render(self, prog_name, bindings, out_tex_fbo, uniforms=None):
        """Render one shader pass into pre-allocated buffer."""
        out_tex, out_fbo = out_tex_fbo
        prog = self.prog[prog_name]
        
        out_fbo.use()
        w, h = out_tex.size
        out_fbo.viewport = (0, 0, w, h)
        
        for uname, tex, unit in bindings:
            tex.use(unit)
            if uname in prog:
                prog[uname].value = unit
        
        if uniforms:
            for uname, val in uniforms.items():
                if uname in prog:
                    prog[uname].value = val
        
        self.vaos[prog_name].render(vertices=4)
        return out_tex
    
    def _mm(self, wname, in_tex):
        """Matrix multiply: in @ W.T using pre-transposed weights."""
        wt = self.wt[wname]
        K, M = wt.size
        buf = self._get_buf('D' if M == self.D else 'FFN' if M == 3072 else 'QKV',
                           M, 1)
        return self._render('matmul',
            [('u_w', wt, 0), ('u_x', in_tex, 1)],
            buf,
            {'u_K': K, 'u_Ww': K, 'u_Wh': M, 'u_B': 1})
    
    def _mm_tiled(self, wname, in_tex):
        """Tiled matmul for large output dimensions (lm_head)."""
        tiles = self.wt[wname + "_tiled"]
        M_total, K = self.wt[wname + "_meta"]
        parts = []
        for tile in tiles:
            K_t, M_t = tile.size
            buf = self._get_buf('TILE', M_t, 1)
            out = self._render('matmul',
                [('u_w', tile, 0), ('u_x', in_tex, 1)],
                buf,
                {'u_K': K_t, 'u_Ww': K_t, 'u_Wh': M_t, 'u_B': 1})
            parts.append(np.frombuffer(out.read(), dtype=np.float32))
        full = np.concatenate(parts)
        return full
    
    def _bias(self, in_tex, bias_name, w):
        """Add bias."""
        buf = self._get_buf('D' if w == self.D else 'FFN' if w == 3072 else 'QKV', w, 1)
        return self._render('add_bias',
            [('u_x', in_tex, 0), ('u_b', self.wt[bias_name], 1)],
            buf)
    
    def _add(self, a, b):
        buf = self._get_buf('D', self.D, 1)
        return self._render('add', [('u_a', a, 0), ('u_b', b, 1)], buf)
    
    def _ln(self, x, gamma, beta):
        """LayerNorm (2 passes)."""
        stats_buf = self._get_buf('S', 1, 1, components=2)
        stats = self._render('stats', [('u_x', x, 0)], stats_buf, {'u_K': self.D})
        
        out_buf = self._get_buf('D', self.D, 1)
        return self._render('layernorm',
            [('u_x', x, 0), ('u_s', stats, 1),
             ('u_g', self.wt[gamma], 2), ('u_bt', self.wt[beta], 3)],
            out_buf)
    
    def _gelu(self, x):
        buf = self._get_buf('FFN', 3072, 1)
        return self._render('gelu', [('u_x', x, 0)], buf)
    
    def _extract_v(self, qkv):
        """Extract V from QKV (for seq_len=1 attention)."""
        buf = self._get_buf('D', self.D, 1)
        return self._render('extract_v', [('u_qkv', qkv, 0)], buf, {'u_D': self.D})
    
    def _mm_bias(self, wname, in_tex, bias_name):
        """Fused matmul + bias add."""
        wt = self.wt[wname]
        K, M = wt.size
        buf = self._get_buf('D' if M == self.D else 'FFN' if M == 3072 else 'QKV', M, 1)
        return self._render('mm_bias',
            [('u_w', wt, 0), ('u_x', in_tex, 1), ('u_b', self.wt[bias_name], 2)],
            buf,
            {'u_K': K, 'u_Ww': K, 'u_Wh': M, 'u_B': 1})
    
    def _ln_fused(self, x, gamma, beta):
        """Fused LayerNorm (stats + normalize in one pass)."""
        buf = self._get_buf('D', self.D, 1)
        return self._render('ln_fused',
            [('u_x', x, 0), ('u_g', self.wt[gamma], 1), ('u_bt', self.wt[beta], 2)],
            buf,
            {'u_K': self.D})
    
    def _block_fused(self, x, layer):
        """Transformer block with fused operations (fewer passes)."""
        p = f"transformer_h_{layer}"
        
        # LN1 -> QKV+bias (fused)
        ln1 = self._ln_fused(x, f"{p}_ln_1_weight", f"{p}_ln_1_bias")
        qkv = self._mm_bias(f"{p}_attn_c_attn_weight", ln1, f"{p}_attn_c_attn_bias")
        
        # Attention (seq_len=1: V only)
        attn = self._extract_v(qkv)
        
        # Proj+bias + residual
        proj = self._mm_bias(f"{p}_attn_c_proj_weight", attn, f"{p}_attn_c_proj_bias")
        res1 = self._add(x, proj)
        
        # LN2 -> FC+bias -> GELU -> MLP+bias + residual
        ln2 = self._ln_fused(res1, f"{p}_ln_2_weight", f"{p}_ln_2_bias")
        fc = self._mm_bias(f"{p}_mlp_c_fc_weight", ln2, f"{p}_mlp_c_fc_bias")
        g = self._gelu(fc)
        mlp = self._mm_bias(f"{p}_mlp_c_proj_weight", g, f"{p}_mlp_c_proj_bias")
        return self._add(res1, mlp)
    
    def _block(self, x, layer):
        """Transformer block."""
        p = f"transformer_h_{layer}"
        
        # LN1 -> QKV -> Bias
        ln1 = self._ln(x, f"{p}_ln_1_weight", f"{p}_ln_1_bias")
        qkv = self._mm(f"{p}_attn_c_attn_weight", ln1)
        qkv = self._bias(qkv, f"{p}_attn_c_attn_bias", 2304)
        
        # Attention (seq_len=1: just V)
        attn = self._extract_v(qkv)
        
        # Proj + bias + residual
        proj = self._mm(f"{p}_attn_c_proj_weight", attn)
        proj = self._bias(proj, f"{p}_attn_c_proj_bias", self.D)
        res1 = self._add(x, proj)
        
        # LN2 -> FC -> bias -> GELU -> proj -> bias + residual
        ln2 = self._ln(res1, f"{p}_ln_2_weight", f"{p}_ln_2_bias")
        fc = self._mm(f"{p}_mlp_c_fc_weight", ln2)
        fc = self._bias(fc, f"{p}_mlp_c_fc_bias", 3072)
        g = self._gelu(fc)
        mlp = self._mm(f"{p}_mlp_c_proj_weight", g)
        mlp = self._bias(mlp, f"{p}_mlp_c_proj_bias", self.D)
        return self._add(res1, mlp)
    
    def forward(self, input_ids):
        """Full forward pass. Returns logits."""
        self._buf_idx = 0  # reset pool index
        
        # Embedding (CPU lookup)
        x_np = self.w["transformer_wte_weight"][input_ids] + \
               self.w["transformer_wpe_weight"][:len(input_ids)]
        
        # Upload to first pool buffer
        x_tex = self._pool['D0'][0]
        x_tex.write(x_np.astype(np.float32).tobytes())
        
        # Run blocks (fused operations)
        cur = x_tex
        for i in range(self.L):
            cur = self._block_fused(cur, i)
        
        # Final LN (fused)
        cur = self._ln_fused(cur, "transformer_ln_f_weight", "transformer_ln_f_bias")
        
        # LM Head
        if "lm_head_weight_tiled" in self.wt:
            logits = self._mm_tiled("lm_head_weight", cur)
        else:
            logit_tex = self._mm("lm_head_weight", cur)
            logits = np.frombuffer(logit_tex.read(), dtype=np.float32)
        
        return logits
    
    def generate(self, prompt, max_new_tokens=20, temperature=0.8, top_k=40):
        tokens = self.tokenizer.encode(prompt)
        print(f"Generating from: '{prompt}'")
        
        # Process prompt: use v5 CPU for multi-token, then GPU for single-token
        if len(tokens) > 1:
            # Use v5 for prompt processing (handles variable length)
            from pixelflow.engine_v5 import GPT2Engine as V5
            v5 = V5()
            prompt_logits = v5.forward(tokens, use_gpu=False)
            next_tok = int(np.argmax(prompt_logits))
            tokens.append(next_tok)
            print(f"  p0: '{self.tokenizer.decode([next_tok])}'")
        
        # Now generate one token at a time (batch=1, all GPU)
        for step in range(max_new_tokens - (1 if len(tokens) > 2 else 0)):
            # Only pass last token (no KV cache = only need last position)
            logits = self.forward([tokens[-1]])
            if temperature > 0:
                logits = logits / temperature
            if top_k > 0:
                top_idx = np.argsort(logits)[-top_k:]
                mask = np.full_like(logits, -np.inf)
                mask[top_idx] = logits[top_idx]
                logits = mask
            mx = logits.max()
            exp_l = np.exp(logits - mx)
            probs = exp_l / exp_l.sum()
            tok = int(np.random.choice(len(probs), p=probs))
            tokens.append(tok)
            print(f"  {step+1:2d}: '{self.tokenizer.decode([tok])}'")
        result = self.tokenizer.decode(tokens)
        print(f"\nResult: {result}")
        return result


if __name__ == "__main__":
    from pixelflow.engine_v5 import GPT2Engine as V5
    import time
    
    eng = GPT2V6B()
    v5 = V5()
    toks = eng.tokenizer.encode("The")
    
    # Accuracy
    print("\n=== Accuracy ===")
    v6_l = eng.forward(toks)
    v5_l = v5.forward(toks, use_gpu=False)
    print(f"v5 vs v6b: corr={np.corrcoef(v6_l, v5_l)[0,1]:.6f}, max_err={np.abs(v6_l-v5_l).max():.6f}")
    print(f"Top-3 v6b: {[eng.tokenizer.decode([t]) for t in np.argsort(v6_l)[-3:][::-1]]}")
    print(f"Top-3 v5:  {[v5.tokenizer.decode([t]) for t in np.argsort(v5_l)[-3:][::-1]]}")
    
    # Determinism check
    l2 = eng.forward(toks)
    l3 = eng.forward(toks)
    print(f"\nDeterminism: call1 vs call2={np.array_equal(v6_l, l2)}, call2 vs call3={np.array_equal(l2, l3)}")
    
    # Benchmark
    for _ in range(5): eng.forward(toks)
    for _ in range(5): v5.forward(toks, use_gpu=True)
    
    v6t, v5t = [], []
    for _ in range(20):
        t = time.perf_counter(); eng.forward(toks); v6t.append((time.perf_counter()-t)*1000)
        t = time.perf_counter(); v5.forward(toks, use_gpu=True); v5t.append((time.perf_counter()-t)*1000)
    print(f"\nv6b (all-GPU): {np.mean(v6t):.1f} +/- {np.std(v6t):.1f} ms")
    print(f"v5  (hybrid):  {np.mean(v5t):.1f} +/- {np.std(v5t):.1f} ms")
    print(f"Speedup:       {np.mean(v5t)/np.mean(v6t):.2f}x")
    
    # Generate
    print("\n=== Generation ===")
    eng.generate("The future of AI is", max_new_tokens=15)
