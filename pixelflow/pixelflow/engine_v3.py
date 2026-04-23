import os
import numpy as np
import moderngl
import tiktoken
from pathlib import Path
from .engine_v2 import ShaderInference, QUAD_VS

class GPT2Inference:
    def __init__(self, weight_dir="gpt2_weights", nvidia=True):
        self.engine = ShaderInference(nvidia=nvidia)
        self.weight_dir = Path(weight_dir)
        self.tokenizer = tiktoken.get_encoding("gpt2")
        
        # Load config
        import json
        with open(self.weight_dir / "config.json") as f:
            self.config = json.load(f)
            
        self._load_all_weights()
        self._load_extra_shaders()
        
    def _load_extra_shaders(self):
        shader_dir = Path(__file__).parent / "shaders"
        for name in ["layernorm", "gelu", "softmax", "reduce_stats"]:
            src = (shader_dir / f"{name}.glsl").read_text()
            self.engine.programs[name] = self.engine.ctx.program(
                vertex_shader=QUAD_VS, fragment_shader=src
            )

    def _load_all_weights(self):
        print("Uploading weights to GPU textures...")
        for p in self.weight_dir.glob("*.npy"):
            name = p.stem
            arr = np.load(p)
            # Some weights are 1D (bias/layernorm), we upload as (1, N) textures
            if arr.ndim == 1:
                arr = arr.reshape(1, -1)
            self.engine.upload_weights(name, arr)

    def forward_mlp(self, x_arr, layer_idx=0):
        """Run the MLP block for a specific layer: GELU(x @ W_fc + b_fc) @ W_proj + b_proj."""
        # x_arr is (seq_len, n_embd)
        seq_len, n_embd = x_arr.shape
        
        # 1. Upload input
        in_tex = self.engine.ctx.texture((n_embd, seq_len), components=1, dtype="f4")
        in_tex.write(x_arr.astype(np.float32).tobytes())
        
        # 2. First Linear (c_fc): 768 -> 3072
        w_fc_name = f"transformer_h_{layer_idx}_mlp_c_fc_weight"
        h = self.engine.linear(w_fc_name, in_tex)
        
        # 3. GELU
        h_gelu_tex = self.engine.ctx.texture((3072, seq_len), components=1, dtype="f4")
        fbo = self.engine.ctx.framebuffer(color_attachments=[h_gelu_tex])
        fbo.use()
        fbo.viewport = (0, 0, 3072, seq_len)
        h.use(0)
        self.engine.programs["gelu"]["u_input"].value = 0
        self.engine.ctx.vertex_array(self.engine.programs["gelu"], [], mode=moderngl.TRIANGLE_STRIP).render(vertices=4)
        
        # 4. Second Linear (c_proj): 3072 -> 768
        w_proj_name = f"transformer_h_{layer_idx}_mlp_c_proj_weight"
        out = self.engine.linear(w_proj_name, h_gelu_tex)
        
        return out

    def generate_demo(self, prompt="The future of AI is", max_new_tokens=5):
        print(f"Generating from prompt: '{prompt}'")
        tokens = self.tokenizer.encode(prompt)
        
        # Load embeddings for the whole prompt
        wte_arr = np.load(self.weight_dir / "transformer_wte_weight.npy")
        wpe_arr = np.load(self.weight_dir / "transformer_wpe_weight.npy")
        
        for _ in range(max_new_tokens):
            seq_len = len(tokens)
            x = wte_arr[tokens] + wpe_arr[range(seq_len)]
            
            # Run a single MLP block as a demonstration of the pixel pipeline
            # In a real model, we'd run all layers and the full attention mechanism
            out_tex = self.forward_mlp(x, layer_idx=0)
            
            # LM Head: 768 -> 50257
            logits_tex = self.engine.linear("lm_head_weight", out_tex)
            
            # Read back last token logits
            # For simplicity, we just read the last row
            raw_logits = np.frombuffer(logits_tex.read(), dtype=np.float32)
            M_out, batch_out = logits_tex.size
            logits = raw_logits.reshape(batch_out, M_out)[-1]
            
            next_token = int(np.argmax(logits))
            tokens.append(next_token)
            
            word = self.tokenizer.decode([next_token])
            print(f"Next token: {next_token} ('{word}')")
            
        return self.tokenizer.decode(tokens)

if __name__ == "__main__":
    # Test init
    gpt2 = GPT2Inference()
    print("GPT-2 GPU Engine initialized.")
