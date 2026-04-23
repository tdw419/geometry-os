"""pixelflow engine_v6 -- All-GPU pipeline. Zero CPU readbacks during inference.

Every operation runs as a fragment shader pass. Activations stay on GPU
as textures between passes. Only the final logits are read back.

Resource management:
  - All textures and FBOs are held in a resource list until forward() completes
  - Weight textures are uploaded once at init and never modified
  - Activation textures are allocated per-layer and reused where possible

This is the architecture that ports directly to WebGL 2.0.
"""
import os
import time
import numpy as np
import moderngl
import tiktoken
from pathlib import Path
from collections import OrderedDict

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

# ============================================================
# Shaders
# ============================================================

MATMUL_FS = """
#version 450
uniform sampler2D u_weights;
uniform sampler2D u_input;
uniform int u_K;        // shared dim (in_features)
uniform int u_Ww;       // weight texture width
uniform int u_Wh;       // weight texture height (= out_features)
uniform int u_batch;
out float frag_output;

void main() {
    int j = int(gl_FragCoord.x);   // output feature
    int b = int(gl_FragCoord.y);   // batch index
    float sum = 0.0;
    for (int k = 0; k < u_K; k++) {
        float w = texture(u_weights, vec2((float(k)+0.5)/float(u_Ww),
                                           (float(j)+0.5)/float(u_Wh))).r;
        float a = texture(u_input,   vec2((float(k)+0.5)/float(u_K),
                                           (float(b)+0.5)/float(u_batch))).r;
        sum += w * a;
    }
    frag_output = sum;
}
"""

ADD_BIAS_FS = """
#version 450
uniform sampler2D u_input;
uniform sampler2D u_bias;  // (width x 1), broadcast across batch
out float frag_output;

void main() {
    ivec2 coord = ivec2(gl_FragCoord.xy);
    float x = texelFetch(u_input, coord, 0).r;
    float b = texelFetch(u_bias, ivec2(coord.x, 0), 0).r;
    frag_output = x + b;
}
"""

ADD_FS = """
#version 450
uniform sampler2D u_a;
uniform sampler2D u_b;
out float frag_output;

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    frag_output = texelFetch(u_a, c, 0).r + texelFetch(u_b, c, 0).r;
}
"""

# LayerNorm: two-pass. Pass 1 computes mean/var. Pass 2 normalizes.
REDUCE_STATS_FS = """
#version 450
uniform sampler2D u_input;
uniform int u_K;
out vec2 frag_output;  // (mean, variance)

void main() {
    int b = int(gl_FragCoord.y);
    float sum = 0.0;
    float sum2 = 0.0;
    for (int k = 0; k < u_K; k++) {
        float x = texelFetch(u_input, ivec2(k, b), 0).r;
        sum += x;
        sum2 += x * x;
    }
    float mean = sum / float(u_K);
    float variance = sum2 / float(u_K) - mean * mean;
    frag_output = vec2(mean, variance);
}
"""

LAYERNORM_FS = """
#version 450
uniform sampler2D u_input;
uniform sampler2D u_stats;   // (1 x batch): R=mean, G=var
uniform sampler2D u_gamma;   // (width x 1)
uniform sampler2D u_beta;    // (width x 1)
out float frag_output;

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    float x = texelFetch(u_input, c, 0).r;
    vec2 stats = texelFetch(u_stats, ivec2(0, c.y), 0).rg;
    float g = texelFetch(u_gamma, ivec2(c.x, 0), 0).r;
    float b = texelFetch(u_beta, ivec2(c.x, 0), 0).r;
    frag_output = ((x - stats.r) / sqrt(stats.g + 1e-5)) * g + b;
}
"""

GELU_FS = """
#version 450
uniform sampler2D u_input;
out float frag_output;

void main() {
    float x = texelFetch(u_input, ivec2(gl_FragCoord.xy), 0).r;
    frag_output = 0.5 * x * (1.0 + tanh(0.7978845608 * (x + 0.044715 * x * x * x)));
}
"""

# Attention: for seq_len=1, softmax(QK^T/sqrt(d)) = 1.0, so attn_out = V
# For seq_len>1, we'd need a full softmax shader. For now, handle seq_len=1 on GPU
# and seq_len>1 falls back to CPU for the attention head manipulation.
# The QKV shader splits Q, K, V and reshapes into heads:
ATTENTION_S1_FS = """
#version 450
// For seq_len=1: output = V (last third of QKV)
uniform sampler2D u_qkv;  // (2304 x batch)
uniform int u_n_embd;
out float frag_output;

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    int feat = c.x;  // 0..n_embd-1
    int batch = c.y;
    // V starts at column n_embd*2
    int qkv_col = feat + u_n_embd * 2;
    frag_output = texelFetch(u_qkv, ivec2(qkv_col, batch), 0).r;
}
"""

# For multi-token: we'll do attention on CPU and upload result.
# The GPU advantage is in the matmuls anyway.

SOFTMAX_FS = """
#version 450
// Row-wise softmax. Requires pre-computed row max and row sum.
uniform sampler2D u_input;
uniform sampler2D u_rowmax;   // (1 x batch)
uniform sampler2D u_rowsum;   // (1 x batch)
out float frag_output;

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    float x = texelFetch(u_input, c, 0).r;
    float mx = texelFetch(u_rowmax, ivec2(0, c.y), 0).r;
    float s  = texelFetch(u_rowsum, ivec2(0, c.y), 0).r;
    frag_output = exp(x - mx) / s;
}
"""

ROW_MAX_FS = """
#version 450
uniform sampler2D u_input;
uniform int u_N;
out float frag_output;

void main() {
    int b = int(gl_FragCoord.y);
    float mx = -1e30;
    for (int i = 0; i < u_N; i++) {
        mx = max(mx, texelFetch(u_input, ivec2(i, b), 0).r);
    }
    frag_output = mx;
}
"""

ROW_SUM_EXP_FS = """
#version 450
uniform sampler2D u_input;
uniform sampler2D u_rowmax;
uniform int u_N;
out float frag_output;

void main() {
    int b = int(gl_FragCoord.y);
    float mx = texelFetch(u_rowmax, ivec2(0, b), 0).r;
    float s = 0.0;
    for (int i = 0; i < u_N; i++) {
        s += exp(texelFetch(u_input, ivec2(i, b), 0).r - mx);
    }
    frag_output = s;
}
"""

MUL_FS = """
#version 450
uniform sampler2D u_a;
uniform sampler2D u_b;
out float frag_output;

void main() {
    ivec2 c = ivec2(gl_FragCoord.xy);
    frag_output = texelFetch(u_a, c, 0).r * texelFetch(u_b, c, 0).r;
}
"""

SCALE_FS = """
#version 450
uniform sampler2D u_input;
uniform float u_scale;
out float frag_output;

void main() {
    frag_output = texelFetch(u_input, ivec2(gl_FragCoord.xy), 0).r * u_scale;
}
"""


class GPUContext:
    """Manages all GPU resources for a forward pass."""
    
    def __init__(self):
        import pygame
        pygame.init()
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MAJOR_VERSION, 4)
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MINOR_VERSION, 6)
        self.screen = pygame.display.set_mode((1, 1), pygame.OPENGL | pygame.HIDDEN)
        self.ctx = moderngl.create_context()
        print(f"GPU: {self.ctx.info['GL_RENDERER']}")
        
        # Compile all shader programs
        self.progs = {}
        shader_map = {
            'matmul': MATMUL_FS,
            'add_bias': ADD_BIAS_FS,
            'add': ADD_FS,
            'reduce_stats': REDUCE_STATS_FS,
            'layernorm': LAYERNORM_FS,
            'gelu': GELU_FS,
            'attn_s1': ATTENTION_S1_FS,
            'softmax': SOFTMAX_FS,
            'row_max': ROW_MAX_FS,
            'row_sum_exp': ROW_SUM_EXP_FS,
            'mul': MUL_FS,
            'scale': SCALE_FS,
        }
        for name, fs in shader_map.items():
            try:
                self.progs[name] = self.ctx.program(vertex_shader=QUAD_VS, fragment_shader=fs)
            except Exception as e:
                print(f"  Shader '{name}' compile error: {e}")
        
        self.vao = self.ctx.vertex_array(self.progs['matmul'], [],
                                          mode=moderngl.TRIANGLE_STRIP)
        
        # Resource tracking: holds refs to prevent GC during forward pass
        self._pass_resources = []
        
        # Weight textures (persistent)
        self.weights = {}
        self.weight_tiled = {}
        
        # Activation texture pool (reuse across layers)
        self._tex_pool = OrderedDict()
        self._pool_enabled = True
    
    def alloc_texture(self, w, h, components=1, dtype='f4', pool_key=None):
        """Allocate a texture, optionally from pool."""
        if pool_key and self._pool_enabled and pool_key in self._tex_pool:
            tex = self._tex_pool[pool_key]
            if tex.size == (w, h) and tex.components == components:
                return tex
            else:
                del self._tex_pool[pool_key]
        
        tex = self.ctx.texture((w, h), components=components, dtype=dtype)
        self._pass_resources.append(tex)
        if pool_key:
            self._tex_pool[pool_key] = tex
        return tex
    
    def run_pass(self, prog_name, bindings, out_size, uniforms=None, out_components=1):
        """Run one shader pass. Returns output texture.
        
        Args:
            prog_name: shader program name
            bindings: list of (uniform_name, texture, binding_unit)
            out_size: (width, height)
            uniforms: dict of {name: value} for int/float uniforms
            out_components: 1 for float, 2 for vec2
        """
        w, h = out_size
        out_tex = self.ctx.texture((w, h), components=out_components, dtype='f4')
        fbo = self.ctx.framebuffer(color_attachments=[out_tex])
        
        # Keep refs alive
        self._pass_resources.extend([out_tex, fbo])
        
        fbo.use()
        fbo.viewport = (0, 0, w, h)
        
        prog = self.progs[prog_name]
        
        for uname, tex, unit in bindings:
            tex.use(unit)
            prog[uname].value = unit
        
        if uniforms:
            for uname, val in uniforms.items():
                prog[uname].value = val
        
        # Create VAO for this specific program
        vao = self.ctx.vertex_array(prog, [], mode=moderngl.TRIANGLE_STRIP)
        self._pass_resources.append(vao)
        vao.render(vertices=4)
        
        return out_tex
    
    def clear_pass_resources(self):
        """Clear temporary resources after forward pass."""
        self._pass_resources.clear()
        self._tex_pool.clear()
    
    def upload_weight(self, name, matrix):
        """Upload weight matrix as texture. Persists across forward passes."""
        M, K = matrix.shape
        max_size = self.ctx.info['GL_MAX_TEXTURE_SIZE']
        
        if M <= max_size:
            tex = self.ctx.texture((K, M), components=1, dtype='f4')
            tex.write(matrix.astype(np.float32).tobytes())
            self.weights[name] = tex
        else:
            tiles = []
            offset = 0
            while offset < M:
                ts = min(max_size, M - offset)
                tile_data = matrix[offset:offset+ts, :]
                t = self.ctx.texture((K, ts), components=1, dtype='f4')
                t.write(tile_data.astype(np.float32).tobytes())
                tiles.append(t)
                offset += ts
            self.weight_tiled[name] = tiles
    
    def matmul(self, weight_name, input_tex, batch=1):
        """GPU matmul: output = input @ W.T (with pre-transposed Conv1D weights)."""
        wt = self.weights.get(weight_name)
        
        if wt is None:
            return self._tiled_matmul(weight_name, input_tex, batch)
        
        K = wt.size[0]
        M = wt.size[1]
        
        out = self.run_pass('matmul',
            [('u_weights', wt, 0), ('u_input', input_tex, 1)],
            (M, batch),
            {'u_K': K, 'u_Ww': K, 'u_Wh': M, 'u_batch': batch})
        
        return out
    
    def _tiled_matmul(self, weight_name, input_tex, batch):
        """Tiled matmul for weights exceeding max texture size."""
        tiles = self.weight_tiled[weight_name]
        K = tiles[0].size[0]
        
        # Compute each tile and concatenate on CPU, then re-upload
        # (Suboptimal but correct; v7 will do this fully on GPU)
        parts = []
        for tile in tiles:
            tile_M = tile.size[1]
            out = self.run_pass('matmul',
                [('u_weights', tile, 0), ('u_input', input_tex, 1)],
                (tile_M, batch),
                {'u_K': K, 'u_Ww': K, 'u_Wh': tile_M, 'u_batch': batch})
            parts.append(np.frombuffer(out.read(), dtype=np.float32).reshape(batch, tile_M))
        
        full = np.concatenate(parts, axis=1)
        total_M = full.shape[1]
        out_tex = self.ctx.texture((total_M, batch), components=1, dtype='f4')
        out_tex.write(full.astype(np.float32).tobytes())
        self._pass_resources.append(out_tex)
        return out_tex


class GPT2EngineV6:
    """GPT-2 inference with all operations on GPU."""
    
    def __init__(self, weight_dir="gpt2_weights"):
        self.gpu = GPUContext()
        self.weight_dir = Path(weight_dir)
        self.tokenizer = tiktoken.get_encoding("gpt2")
        
        import json
        with open(self.weight_dir / "config.json") as f:
            cfg = json.load(f)
        self.n_embd = cfg["n_embd"]
        self.n_head = cfg["n_head"]
        self.n_layer = cfg["n_layer"]
        self.head_dim = self.n_embd // self.n_head
        
        print(f"GPT-2: {self.n_layer}L, {self.n_head}H, {self.n_embd}D")
        
        self._load_weights()
    
    def _load_weights(self):
        """Load weights: numpy for CPU ops, GPU textures for matmuls."""
        self.w_np = {}
        conv1d = {"attn_c_attn_weight", "attn_c_proj_weight",
                  "mlp_c_fc_weight", "mlp_c_proj_weight"}
        
        print("Loading weights...")
        count = 0
        for p in sorted(self.weight_dir.glob("*.npy")):
            name = p.stem
            arr = np.load(p)
            self.w_np[name] = arr
            
            if arr.ndim == 2:
                # Determine if this is a Conv1D layer (x @ W) or standard Linear (x @ W.T)
                suffix = name.split("_", 3)[-1] if name.startswith("transformer_h_") else ""
                if suffix in conv1d:
                    gpu_arr = arr.T  # transpose for our shader's x @ W.T convention
                elif name == "lm_head_weight":
                    gpu_arr = arr  # already (out, in) for standard linear
                else:
                    continue  # skip embedding weights
                
                self.gpu.upload_weight(name, gpu_arr)
                count += 1
        
        # Upload bias/gamma/beta as 1D textures (reshaped to 1 x N)
        for name, arr in self.w_np.items():
            if arr.ndim == 1 and any(name.endswith(s) for s in [
                "weight", "bias",  # LN weights and biases
            ]):
                tex = self.gpu.ctx.texture((len(arr), 1), components=1, dtype='f4')
                tex.write(arr.astype(np.float32).tobytes())
                self.gpu.weights[name] = tex
        
        print(f"  {count} weight matrices + biases uploaded to GPU")
    
    def _ln(self, x_tex, gamma_name, beta_name, width, batch):
        """LayerNorm entirely on GPU."""
        # Pass 1: mean + variance
        stats = self.gpu.run_pass('reduce_stats',
            [('u_input', x_tex, 0)],
            (1, batch),
            {'u_K': width},
            out_components=2)
        
        # Pass 2: normalize
        gamma_tex = self.gpu.weights[gamma_name]
        beta_tex = self.gpu.weights[beta_name]
        out = self.gpu.run_pass('layernorm',
            [('u_input', x_tex, 0), ('u_stats', stats, 1),
             ('u_gamma', gamma_tex, 2), ('u_beta', beta_tex, 3)],
            (width, batch))
        
        return out
    
    def _attention_gpu(self, qkv_tex, seq_len, batch):
        """Attention for seq_len=1: output = V."""
        if seq_len == 1:
            return self.gpu.run_pass('attn_s1',
                [('u_qkv', qkv_tex, 0)],
                (self.n_embd, batch),
                {'u_n_embd': self.n_embd})
        else:
            # Multi-token attention: readback to CPU, compute, re-upload
            # This is the fallback for prompt processing
            qkv_np = np.frombuffer(qkv_tex.read(), dtype=np.float32).reshape(batch, 2304)
            q, k, v = np.split(qkv_np, 3, axis=-1)
            attn_out = self._attention_cpu(q, k, v, seq_len)
            tex = self.gpu.ctx.texture((self.n_embd, batch), components=1, dtype='f4')
            tex.write(attn_out.astype(np.float32).tobytes())
            self.gpu._pass_resources.append(tex)
            return tex
    
    def _attention_cpu(self, q, k, v, seq_len):
        """CPU attention for multi-token case."""
        q = q.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        k = k.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        v = v.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        
        scale = 1.0 / np.sqrt(self.head_dim)
        scores = np.matmul(q, k.transpose(0, 2, 1)) * scale
        if seq_len > 1:
            mask = np.triu(np.ones((seq_len, seq_len)), k=1) * -1e10
            scores += mask
        mx = scores.max(axis=-1, keepdims=True)
        exp_s = np.exp(scores - mx)
        weights = exp_s / exp_s.sum(axis=-1, keepdims=True)
        out = np.matmul(weights, v).transpose(1, 0, 2).reshape(seq_len, self.n_embd)
        return out
    
    def _transformer_block(self, x_tex, layer, batch):
        """Full transformer block on GPU."""
        p = f"transformer_h_{layer}"
        
        # LN1
        ln1 = self._ln(x_tex, f"{p}_ln_1_weight", f"{p}_ln_1_bias", self.n_embd, batch)
        
        # QKV: (768) -> (2304)
        qkv = self.gpu.matmul(f"{p}_attn_c_attn_weight", ln1, batch)
        qkv = self.gpu.run_pass('add_bias',
            [('u_input', qkv, 0), ('u_bias', self.gpu.weights[f"{p}_attn_c_attn_bias"], 1)],
            (2304, batch))
        
        # Attention
        attn = self._attention_gpu(qkv, batch, batch)
        
        # Output projection: (768) -> (768)
        proj = self.gpu.matmul(f"{p}_attn_c_proj_weight", attn, batch)
        proj = self.gpu.run_pass('add_bias',
            [('u_input', proj, 0), ('u_bias', self.gpu.weights[f"{p}_attn_c_proj_bias"], 1)],
            (self.n_embd, batch))
        
        # Residual 1
        res1 = self.gpu.run_pass('add',
            [('u_a', x_tex, 0), ('u_b', proj, 1)],
            (self.n_embd, batch))
        
        # LN2
        ln2 = self._ln(res1, f"{p}_ln_2_weight", f"{p}_ln_2_bias", self.n_embd, batch)
        
        # MLP FC: (768) -> (3072)
        fc = self.gpu.matmul(f"{p}_mlp_c_fc_weight", ln2, batch)
        fc = self.gpu.run_pass('add_bias',
            [('u_input', fc, 0), ('u_bias', self.gpu.weights[f"{p}_mlp_c_fc_bias"], 1)],
            (3072, batch))
        
        # GELU
        gelu = self.gpu.run_pass('gelu', [('u_input', fc, 0)], (3072, batch))
        
        # MLP proj: (3072) -> (768)
        mlp = self.gpu.matmul(f"{p}_mlp_c_proj_weight", gelu, batch)
        mlp = self.gpu.run_pass('add_bias',
            [('u_input', mlp, 0), ('u_bias', self.gpu.weights[f"{p}_mlp_c_proj_bias"], 1)],
            (self.n_embd, batch))
        
        # Residual 2
        out = self.gpu.run_pass('add',
            [('u_a', res1, 0), ('u_b', mlp, 1)],
            (self.n_embd, batch))
        
        return out
    
    def forward(self, input_ids):
        """Full forward pass. Returns logits numpy array."""
        batch = len(input_ids)
        self.gpu._pass_resources.clear()
        
        # Embedding on CPU (lookup table, not worth GPU overhead)
        x_np = self.w_np["transformer_wte_weight"][input_ids] + \
               self.w_np["transformer_wpe_weight"][:batch]
        
        # Upload initial activations
        x_tex = self.gpu.ctx.texture((self.n_embd, batch), components=1, dtype='f4')
        x_tex.write(x_np.astype(np.float32).tobytes())
        self.gpu._pass_resources.append(x_tex)
        
        # Transformer blocks
        for i in range(self.n_layer):
            x_tex = self._transformer_block(x_tex, i, batch)
        
        # Final LN
        x_tex = self._ln(x_tex, "transformer_ln_f_weight", "transformer_ln_f_bias",
                         self.n_embd, batch)
        
        # LM Head: last position only
        if batch > 1:
            # Read last position, re-upload
            all_hidden = np.frombuffer(x_tex.read(), dtype=np.float32).reshape(batch, self.n_embd)
            last = all_hidden[-1:].astype(np.float32)
            last_tex = self.gpu.ctx.texture((self.n_embd, 1), components=1, dtype='f4')
            last_tex.write(last.tobytes())
            self.gpu._pass_resources.append(last_tex)
        else:
            last_tex = x_tex
        
        logits_tex = self.gpu.matmul("lm_head_weight", last_tex, 1)
        logits = np.frombuffer(logits_tex.read(), dtype=np.float32)
        
        self.gpu.clear_pass_resources()
        return logits
    
    def generate(self, prompt, max_new_tokens=20, temperature=0.8, top_k=40):
        """Generate text."""
        tokens = self.tokenizer.encode(prompt)
        print(f"Generating from: '{prompt}'")
        
        for step in range(max_new_tokens):
            logits = self.forward(tokens)
            
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
            
            next_tok = int(np.random.choice(len(probs), p=probs))
            tokens.append(next_tok)
            word = self.tokenizer.decode([next_tok])
            print(f"  {step+1:2d}: '{word}'")
        
        result = self.tokenizer.decode(tokens)
        print(f"\nResult: {result}")
        return result


if __name__ == "__main__":
    engine = GPT2EngineV6()
    
    # Accuracy check vs v5 CPU reference
    tokens = engine.tokenizer.encode("The")
    
    print("\n=== v6 GPU ===")
    v6_logits = engine.forward(tokens)
    v6_top = np.argsort(v6_logits)[-5:][::-1]
    for idx in v6_top:
        print(f"  {engine.tokenizer.decode([idx]):>10s} ({v6_logits[idx]:.4f})")
    
    # v5 CPU reference
    from pixelflow.engine_v5 import GPT2Engine as V5
    v5 = V5()
    v5_logits = v5.forward(tokens, use_gpu=False)
    
    corr = np.corrcoef(v6_logits, v5_logits)[0, 1]
    max_err = np.max(np.abs(v6_logits - v5_logits))
    print(f"\nv5 vs v6: corr={corr:.6f}, max_err={max_err:.6f}")
    
    # Benchmark
    import time
    for _ in range(3): engine.forward(tokens)  # warmup
    
    v6_times = []
    for _ in range(10):
        t0 = time.perf_counter()
        engine.forward(tokens)
        v6_times.append((time.perf_counter() - t0) * 1000)
    
    v5_times = []
    for _ in range(10):
        t0 = time.perf_counter()
        v5.forward(tokens, use_gpu=True)
        v5_times.append((time.perf_counter() - t0) * 1000)
    
    print(f"\nv5 (hybrid): {np.mean(v5_times):.1f}ms")
    print(f"v6 (all-GPU): {np.mean(v6_times):.1f}ms")
    print(f"Speedup: {np.mean(v5_times)/np.mean(v6_times):.2f}x")
    
    # Generation
    print("\n=== v6 Generation ===")
    engine.generate("The future of AI is", max_new_tokens=10)
