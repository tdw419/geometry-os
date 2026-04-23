"""pixelflow engine_v5 -- GPT-2 inference with GPU matmuls only.

All non-linear ops (LayerNorm, GELU, softmax, attention) run on CPU.
Only the large matrix multiplies (Linear layers) run on GPU via fragment shaders.
"""
import os
import time
import numpy as np
import moderngl
import tiktoken
from pathlib import Path

os.environ['__NV_PRIME_RENDER_OFFLOAD'] = '1'
os.environ['__GLX_VENDOR_LIBRARY_NAME'] = 'nvidia'

QUAD_VS = """
#version 450
void main() {
    float x = -1.0 + float(gl_VertexID % 2) * 2.0;
    float y = -1.0 + float(gl_VertexID / 2) * 2.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}
"""

MATMUL_FS = """
#version 450
uniform sampler2D u_weights;
uniform sampler2D u_input;
uniform int u_K;
uniform int u_Ww;
uniform int u_Wh;
uniform int u_batch;
out float frag_output;

void main() {
    int out_idx = int(gl_FragCoord.x);
    int batch_idx = int(gl_FragCoord.y);
    
    float sum = 0.0;
    for (int k = 0; k < u_K; k++) {
        vec2 w_uv = vec2(
            (float(k) + 0.5) / float(u_Ww),
            (float(out_idx) + 0.5) / float(u_Wh)
        );
        float w = texture(u_weights, w_uv).r;
        
        vec2 a_uv = vec2(
            (float(k) + 0.5) / float(u_K),
            (float(batch_idx) + 0.5) / float(u_batch)
        );
        float a = texture(u_input, a_uv).r;
        
        sum += w * a;
    }
    frag_output = sum;
}
"""


class GPT2Engine:
    """GPT-2 inference with GPU-accelerated matmuls."""
    
    def __init__(self, weight_dir="gpt2_weights"):
        import pygame
        pygame.init()
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MAJOR_VERSION, 4)
        pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MINOR_VERSION, 6)
        screen = pygame.display.set_mode((1, 1), pygame.OPENGL | pygame.HIDDEN)
        self.screen = screen
        
        self.ctx = moderngl.create_context()
        print(f"GPU: {self.ctx.info['GL_RENDERER']}")
        
        self.weight_dir = Path(weight_dir)
        self.tokenizer = tiktoken.get_encoding("gpt2")
        
        # Load config
        import json
        with open(self.weight_dir / "config.json") as f:
            self.config = json.load(f)
        
        self.n_embd = self.config["n_embd"]
        self.n_head = self.config["n_head"]
        self.n_layer = self.config["n_layer"]
        self.head_dim = self.n_embd // self.n_head
        
        print(f"GPT-2: {self.n_layer} layers, {self.n_head} heads, {self.n_embd} dim")
        
        # Compile shader
        self.prog = self.ctx.program(vertex_shader=QUAD_VS, fragment_shader=MATMUL_FS)
        self.vao = self.ctx.vertex_array(self.prog, [], mode=moderngl.TRIANGLE_STRIP)
        
        # Load all weights into numpy (CPU-side)
        self.weights = {}
        self._load_weights()
        
        # Upload large weight matrices to GPU textures
        self.gpu_weights = {}
        self._upload_gpu_weights()
    
    def _load_weights(self):
        """Load all weights as numpy arrays."""
        print("Loading weights...")
        for p in sorted(self.weight_dir.glob("*.npy")):
            name = p.stem
            arr = np.load(p)
            self.weights[name] = arr
        print(f"Loaded {len(self.weights)} weight arrays")
    
    def _upload_gpu_weights(self):
        """Upload linear layer weights to GPU textures."""
        # GPT-2 Conv1D layers: forward = x @ W + b
        # Our shader computes x @ W.T, so we need to transpose
        conv1d_suffixes = [
            "attn_c_attn_weight", "attn_c_proj_weight",
            "mlp_c_fc_weight", "mlp_c_proj_weight",
        ]
        # Also upload lm_head (standard linear, no transpose)
        
        count = 0
        for name, arr in self.weights.items():
            if arr.ndim != 2:
                continue
            if not any(name.endswith(s) for s in conv1d_suffixes) and name != "lm_head_weight":
                continue
            
            # Transpose Conv1D weights
            if name != "lm_head_weight":
                arr = arr.T  # (in, out) -> (out, in) for x @ W.T = x @ W
            
            M, K = arr.shape
            max_size = self.ctx.info['GL_MAX_TEXTURE_SIZE']
            
            if M <= max_size:
                tex = self.ctx.texture((K, M), components=1, dtype="f4")
                tex.write(arr.astype(np.float32).tobytes())
                self.gpu_weights[name] = tex
            else:
                # Tiled for large outputs (lm_head: 50257)
                tiles = []
                offset = 0
                while offset < M:
                    tile_size = min(max_size, M - offset)
                    tile = arr[offset:offset+tile_size, :]
                    t = self.ctx.texture((K, tile_size), components=1, dtype="f4")
                    t.write(tile.astype(np.float32).tobytes())
                    tiles.append(t)
                    offset += tile_size
                self.gpu_weights[name] = tiles
            
            count += 1
        
        print(f"Uploaded {count} weight matrices to GPU")
    
    def gpu_matmul(self, weight_name, x):
        """Matrix multiply: output = x @ W using GPU shader.
        
        Args:
            weight_name: key in self.gpu_weights
            x: numpy array (batch, K) where K = in_features
        
        Returns:
            numpy array (batch, M) where M = out_features
        """
        if x.ndim == 1:
            x = x.reshape(1, -1)
        batch, K = x.shape
        
        wt = self.gpu_weights[weight_name]
        
        if isinstance(wt, list):
            # Tiled
            results = []
            for tile in wt:
                tile_M = tile.size[1]
                # Upload input
                x_tex = self.ctx.texture((K, batch), components=1, dtype="f4")
                x_tex.write(x.astype(np.float32).tobytes())
                # Output
                out_tex = self.ctx.texture((tile_M, batch), components=1, dtype="f4")
                fbo = self.ctx.framebuffer(color_attachments=[out_tex])
                fbo.use()
                fbo.viewport = (0, 0, tile_M, batch)
                tile.use(0)
                x_tex.use(1)
                self.prog["u_weights"].value = 0
                self.prog["u_input"].value = 1
                self.prog["u_K"].value = K
                self.prog["u_Ww"].value = tile.size[0]
                self.prog["u_Wh"].value = tile.size[1]
                self.prog["u_batch"].value = batch
                self.vao.render(vertices=4)
                result = np.frombuffer(out_tex.read(), dtype=np.float32).reshape(batch, tile_M)
                results.append(result)
                fbo.release()
                out_tex.release()
                x_tex.release()
            return np.concatenate(results, axis=1)
        else:
            M = wt.size[1]
            # Upload input
            x_tex = self.ctx.texture((K, batch), components=1, dtype="f4")
            x_tex.write(x.astype(np.float32).tobytes())
            # Output
            out_tex = self.ctx.texture((M, batch), components=1, dtype="f4")
            fbo = self.ctx.framebuffer(color_attachments=[out_tex])
            fbo.use()
            fbo.viewport = (0, 0, M, batch)
            wt.use(0)
            x_tex.use(1)
            self.prog["u_weights"].value = 0
            self.prog["u_input"].value = 1
            self.prog["u_K"].value = K
            self.prog["u_Ww"].value = wt.size[0]
            self.prog["u_Wh"].value = wt.size[1]
            self.prog["u_batch"].value = batch
            self.vao.render(vertices=4)
            result = np.frombuffer(out_tex.read(), dtype=np.float32).reshape(batch, M)
            fbo.release()
            out_tex.release()
            x_tex.release()
            return result
    
    def cpu_matmul(self, weight_name, x):
        """Matrix multiply on CPU (fallback)."""
        if x.ndim == 1:
            x = x.reshape(1, -1)
        w = self.weights[weight_name]
        if w.ndim == 1:
            return x  # skip for 1D
        # Conv1D layers: x @ W (no transpose)
        # Standard Linear (lm_head): x @ W.T
        conv1d_suffixes = [
            "attn_c_attn_weight", "attn_c_proj_weight",
            "mlp_c_fc_weight", "mlp_c_proj_weight",
        ]
        if any(weight_name.endswith(s) for s in conv1d_suffixes):
            return x @ w  # Conv1D: x @ W
        else:
            return x @ w.T  # Linear: x @ W.T
    
    def layer_norm(self, x, weight, bias, eps=1e-5):
        """LayerNorm on CPU."""
        mean = x.mean(axis=-1, keepdims=True)
        var = x.var(axis=-1, keepdims=True)
        return (x - mean) / np.sqrt(var + eps) * weight + bias
    
    def gelu(self, x):
        """GELU activation (tanh approximation) on CPU."""
        c1 = 0.7978845608
        c2 = 0.044715
        return 0.5 * x * (1.0 + np.tanh(c1 * (x + c2 * x**3)))
    
    def attention(self, q, k, v):
        """Multi-head scaled dot-product attention on CPU."""
        seq_len = q.shape[0]
        scale = 1.0 / np.sqrt(self.head_dim)
        
        # Reshape to (n_head, seq_len, head_dim)
        q = q.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        k = k.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        v = v.reshape(seq_len, self.n_head, self.head_dim).transpose(1, 0, 2)
        
        # Scaled dot-product
        scores = np.matmul(q, k.transpose(0, 2, 1)) * scale
        
        # Causal mask
        if seq_len > 1:
            mask = np.triu(np.ones((seq_len, seq_len)), k=1) * -1e10
            scores += mask
        
        # Softmax
        scores_max = scores.max(axis=-1, keepdims=True)
        exp_scores = np.exp(scores - scores_max)
        attn_weights = exp_scores / exp_scores.sum(axis=-1, keepdims=True)
        
        # Weighted sum
        out = np.matmul(attn_weights, v)  # (n_head, seq_len, head_dim)
        return out.transpose(1, 0, 2).reshape(seq_len, self.n_embd)
    
    def transformer_block(self, x, layer_idx, use_gpu=True):
        """Single transformer block.
        
        x: (seq_len, n_embd)
        """
        p = f"transformer_h_{layer_idx}"
        matmul = self.gpu_matmul if use_gpu else self.cpu_matmul
        
        # LN1
        ln1_out = self.layer_norm(
            x, self.weights[f"{p}_ln_1_weight"], self.weights[f"{p}_ln_1_bias"])
        
        # QKV projection: (seq_len, 768) -> (seq_len, 2304)
        qkv = matmul(f"{p}_attn_c_attn_weight", ln1_out)
        qkv = qkv + self.weights[f"{p}_attn_c_attn_bias"]
        
        # Split Q, K, V
        q, k, v = np.split(qkv, 3, axis=-1)
        
        # Attention
        attn_out = self.attention(q, k, v)
        
        # Output projection
        proj = matmul(f"{p}_attn_c_proj_weight", attn_out)
        proj = proj + self.weights[f"{p}_attn_c_proj_bias"]
        
        # Residual 1
        x = x + proj
        
        # LN2
        ln2_out = self.layer_norm(
            x, self.weights[f"{p}_ln_2_weight"], self.weights[f"{p}_ln_2_bias"])
        
        # MLP FC
        fc = matmul(f"{p}_mlp_c_fc_weight", ln2_out)
        fc = fc + self.weights[f"{p}_mlp_c_fc_bias"]
        
        # GELU
        fc = self.gelu(fc)
        
        # MLP projection
        mlp = matmul(f"{p}_mlp_c_proj_weight", fc)
        mlp = mlp + self.weights[f"{p}_mlp_c_proj_bias"]
        
        # Residual 2
        x = x + mlp
        
        return x
    
    def forward(self, input_ids, use_gpu=True):
        """Full GPT-2 forward pass.
        
        Args:
            input_ids: list of token IDs
            use_gpu: if True, use GPU for matmuls; else CPU
            
        Returns:
            logits: numpy array (vocab_size,)
        """
        seq_len = len(input_ids)
        
        # Embeddings
        x = self.weights["transformer_wte_weight"][input_ids] + \
            self.weights["transformer_wpe_weight"][:seq_len]
        
        # Transformer blocks
        for i in range(self.n_layer):
            x = self.transformer_block(x, i, use_gpu=use_gpu)
        
        # Final LayerNorm
        x = self.layer_norm(
            x, self.weights["transformer_ln_f_weight"], self.weights["transformer_ln_f_bias"])
        
        # LM Head (last position)
        last_hidden = x[-1:]  # (1, 768)
        matmul = self.gpu_matmul if use_gpu else self.cpu_matmul
        logits = matmul("lm_head_weight", last_hidden).flatten()
        
        return logits
    
    def generate(self, prompt, max_new_tokens=20, temperature=0.8, top_k=40, use_gpu=True):
        """Generate text from prompt."""
        tokens = self.tokenizer.encode(prompt)
        print(f"Generating from: '{prompt}'")
        
        for step in range(max_new_tokens):
            logits = self.forward(tokens, use_gpu=use_gpu)
            
            # Temperature
            if temperature > 0:
                logits = logits / temperature
            
            # Top-k filtering
            if top_k > 0:
                top_k_idx = np.argsort(logits)[-top_k:]
                mask = np.full_like(logits, -np.inf)
                mask[top_k_idx] = logits[top_k_idx]
                logits = mask
            
            # Sample
            logits_max = logits.max()
            exp_logits = np.exp(logits - logits_max)
            probs = exp_logits / exp_logits.sum()
            
            next_token = np.random.choice(len(probs), p=probs)
            tokens.append(int(next_token))
            
            word = self.tokenizer.decode([int(next_token)])
            print(f"  Step {step+1}: '{word}' (id={next_token})")
        
        result = self.tokenizer.decode(tokens)
        print(f"\nResult: {result}")
        return result


if __name__ == "__main__":
    import sys
    
    engine = GPT2Engine()
    
    # Compare GPU vs CPU
    tokens = engine.tokenizer.encode("The")
    
    print("\n=== CPU Reference ===")
    cpu_logits = engine.forward(tokens, use_gpu=False)
    cpu_top5 = np.argsort(cpu_logits)[-5:][::-1]
    for idx in cpu_top5:
        print(f"  {engine.tokenizer.decode([idx]):>10s} ({cpu_logits[idx]:.4f})")
    
    print("\n=== GPU Shader ===")
    gpu_logits = engine.forward(tokens, use_gpu=True)
    gpu_logits = np.nan_to_num(gpu_logits, nan=0.0)
    gpu_top5 = np.argsort(gpu_logits)[-5:][::-1]
    for idx in gpu_top5:
        print(f"  {engine.tokenizer.decode([idx]):>10s} ({gpu_logits[idx]:.4f})")
    
    # Correlation
    valid = ~np.isnan(gpu_logits) & ~np.isinf(gpu_logits)
    if valid.sum() > 100:
        corr = np.corrcoef(gpu_logits[valid], cpu_logits[valid])[0, 1]
        max_err = np.max(np.abs(gpu_logits[valid] - cpu_logits[valid]))
        print(f"\nCorrelation: {corr:.6f}")
        print(f"Max error: {max_err:.4f}")
    
    # Generate
    print("\n=== Generation Test ===")
    engine.generate("The future of AI is", max_new_tokens=10, use_gpu=True)
