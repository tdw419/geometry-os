"""Pixelflow inference engine v2 -- optimized multi-pass pipeline.

Key optimizations over v1:
1. Pre-allocated weight textures (uploaded once)
2. Reusable FBOs and intermediate textures
3. Batch render passes without intermediate readback
4. Pygame+NVidia initialization for dedicated GPU
"""

import os
import numpy as np
import moderngl
import time
from pathlib import Path

SHADER_DIR = Path(__file__).parent / "shaders"

QUAD_VS = """
#version 450
void main() {
    float x = -1.0 + float(gl_VertexID % 2) * 2.0;
    float y = -1.0 + float(gl_VertexID / 2) * 2.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}
"""


def create_context(nvidia: bool = True, headless: bool = True):
    """Create OpenGL context, optionally forcing NVIDIA GPU."""
    if nvidia:
        os.environ['__NV_PRIME_RENDER_OFFLOAD'] = '1'
        os.environ['__GLX_VENDOR_LIBRARY_NAME'] = 'nvidia'
    
    import pygame
    pygame.init()
    pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MAJOR_VERSION, 4)
    pygame.display.gl_set_attribute(pygame.GL_CONTEXT_MINOR_VERSION, 6)
    pygame.display.gl_set_attribute(pygame.GL_CONTEXT_PROFILE_MASK, pygame.GL_CONTEXT_PROFILE_CORE)
    
    flags = pygame.OPENGL | pygame.HIDDEN
    screen = pygame.display.set_mode((1, 1), flags)
    
    ctx = moderngl.create_context()
    return ctx, pygame


class ShaderInference:
    """Pre-allocated multi-pass inference pipeline."""
    
    def __init__(self, nvidia: bool = True):
        self.ctx, self._pygame = create_context(nvidia=nvidia)
        self.renderer = self.ctx.info['GL_RENDERER']
        self.textures = {}
        self.programs = {}
        self.fbos = {}
        self._load_shaders()
        self._vao = self.ctx.vertex_array(
            self.programs["matmul"], [], mode=moderngl.TRIANGLE_STRIP
        )
    
    def _load_shaders(self):
        matmul_src = (SHADER_DIR / "matmul.glsl").read_text()
        self.programs["matmul"] = self.ctx.program(
            vertex_shader=QUAD_VS, fragment_shader=matmul_src
        )
    
    def upload_weights(self, name: str, matrix: np.ndarray):
        """Upload weight matrix. Matrix shape: (M, K) = (out_features, in_features).
        
        If M > max_texture_size, splits into tiled chunks.
        """
        M, K = matrix.shape
        max_size = self.ctx.info['GL_MAX_TEXTURE_SIZE']
        
        if M <= max_size:
            tex = self.ctx.texture((K, M), components=1, dtype="f4")
            tex.write(matrix.astype(np.float32).tobytes())
            self.textures[name] = tex
            return tex
        else:
            # Tile: split M into chunks of max_size
            n_tiles = (M + max_size - 1) // max_size
            tiles = []
            for i in range(n_tiles):
                start = i * max_size
                end = min(start + max_size, M)
                chunk = matrix[start:end, :]
                tex = self.ctx.texture((K, end - start), components=1, dtype="f4")
                tex.write(chunk.astype(np.float32).tobytes())
                tiles.append(tex)
                self.textures[f"{name}_tile{i}"] = tex
            self.textures[name] = tiles  # Store list for tiled matmul
            return tiles
    
    def linear(self, weight_name: str, input_data,
               reuse_fbo: str = None) -> moderngl.Texture:
        """Single linear layer: output = input @ W.T.
        
        Handles tiled weights (for M > max_texture_size) by concatenating
        partial results.
        
        Args:
            weight_name: Pre-uploaded weight texture name
            input_data: Numpy array or moderngl Texture
            reuse_fbo: FBO name to reuse (avoids reallocation)
            
        Returns:
            Output texture
        """
        wt = self.textures[weight_name]
        
        # Check if tiled
        if isinstance(wt, list):
            return self._tiled_linear(weight_name, input_data, wt)
        
        # Handle input
        if isinstance(input_data, np.ndarray):
            if input_data.ndim == 1:
                input_data = input_data.reshape(1, -1)
            batch, K = input_data.shape
            it = self.ctx.texture((K, batch), components=1, dtype="f4")
            it.write(input_data.astype(np.float32).tobytes())
        else:
            it = input_data
            K, batch = it.size
            
        M = wt.size[1]
        
        # Reuse or create FBO
        if reuse_fbo and reuse_fbo in self.fbos:
            fbo, ot = self.fbos[reuse_fbo]
            if ot.size != (M, batch):
                fbo.release()
                ot.release()
                ot = self.ctx.texture((M, batch), components=1, dtype="f4")
                fbo = self.ctx.framebuffer(color_attachments=[ot])
                self.fbos[reuse_fbo] = (fbo, ot)
        else:
            ot = self.ctx.texture((M, batch), components=1, dtype="f4")
            fbo = self.ctx.framebuffer(color_attachments=[ot])
            if reuse_fbo:
                self.fbos[reuse_fbo] = (fbo, ot)
        
        fbo.use()
        fbo.viewport = (0, 0, M, batch)
        
        # Bind and set uniforms
        wt.use(0)
        it.use(1)
        prog = self.programs["matmul"]
        prog["u_weights"].value = 0
        prog["u_input"].value = 1
        prog["u_K"].value = K
        prog["u_Ww"].value = wt.size[0]
        prog["u_Wh"].value = wt.size[1]
        prog["u_batch"].value = batch
        prog["u_mode"].value = 0
        prog["u_w_scale"].value = 1.0
        prog["u_a_scale"].value = 1.0
        
        self._vao.render(vertices=4)
        
        return ot
    
    def _tiled_linear(self, weight_name, input_data, tiles):
        """Linear with tiled weight matrix. Concatenates partial outputs."""
        # Handle input
        if isinstance(input_data, np.ndarray):
            if input_data.ndim == 1:
                input_data = input_data.reshape(1, -1)
            batch, K = input_data.shape
            it = self.ctx.texture((K, batch), components=1, dtype="f4")
            it.write(input_data.astype(np.float32).tobytes())
        else:
            it = input_data
            K, batch = it.size
        
        total_M = sum(t.size[1] for t in tiles)
        
        # Process each tile and read back results
        results = []
        for i, wt in enumerate(tiles):
            tile_M = wt.size[1]
            ot = self.ctx.texture((tile_M, batch), components=1, dtype="f4")
            fbo = self.ctx.framebuffer(color_attachments=[ot])
            fbo.use()
            fbo.viewport = (0, 0, tile_M, batch)
            
            wt.use(0)
            it.use(1)
            prog = self.programs["matmul"]
            prog["u_weights"].value = 0
            prog["u_input"].value = 1
            prog["u_K"].value = K
            prog["u_Ww"].value = wt.size[0]
            prog["u_Wh"].value = wt.size[1]
            prog["u_batch"].value = batch
            prog["u_mode"].value = 0
            prog["u_w_scale"].value = 1.0
            prog["u_a_scale"].value = 1.0
            
            self._vao.render(vertices=4)
            
            # Read back this tile's result
            tile_data = np.frombuffer(ot.read(), dtype=np.float32).reshape(batch, tile_M)
            results.append(tile_data)
            fbo.release()
            ot.release()
        
        # Concatenate and upload as single texture
        full_result = np.concatenate(results, axis=1)  # (batch, total_M)
        out_tex = self.ctx.texture((total_M, batch), components=1, dtype="f4")
        out_tex.write(full_result.astype(np.float32).tobytes())
        return out_tex
    def benchmark_layer(self, M: int, K: int, warmup: int = 10, runs: int = 100) -> dict:
        """Benchmark a single linear layer with pre-allocated weights."""
        rng = np.random.default_rng(42)
        W = rng.standard_normal((M, K)).astype(np.float32) * 0.01
        x = rng.standard_normal(K).astype(np.float32) * 0.01
        
        self.upload_weights("bench", W)
        
        # Warmup (includes FBO allocation)
        for i in range(warmup):
            r_tex = self.linear("bench", x, reuse_fbo="bench_fbo")
            # Force sync
            _ = r_tex.read()

        # Timed runs
        times = []
        for i in range(runs):
            t0 = time.perf_counter()
            r_tex = self.linear("bench", x, reuse_fbo="bench_fbo")
            _ = r_tex.read() # Force GPU sync
            times.append((time.perf_counter() - t0) * 1000)

        
        expected = x @ W.T
        r_final = np.frombuffer(r_tex.read(), dtype=np.float32).reshape(1, M)
        max_err = np.max(np.abs(r_final.flatten() - expected))
        
        return {
            "shape": f"({M}, {K})",
            "time_ms": np.mean(times),
            "std_ms": np.std(times),
            "p50_ms": np.percentile(times, 50),
            "p99_ms": np.percentile(times, 99),
            "max_error": max_err,
            "gflops": (2 * M * K) / (np.mean(times) / 1000) / 1e9,
        }
    
    def cleanup(self):
        for tex in self.textures.values():
            tex.release()
        for prog in self.programs.values():
            prog.release()
        for fbo, ot in self.fbos.values():
            fbo.release()
            ot.release()
        self.textures.clear()
        self.programs.clear()
        self.fbos.clear()
        self.ctx.release()
        self._pygame.quit()


def run_full_benchmark():
    """Run the comprehensive benchmark suite."""
    import torch
    
    engine = ShaderInference(nvidia=True)
    print(f"GPU: {engine.renderer}")
    print()
    
    device = torch.device("cuda")
    
    # GPT-2 small layer sizes
    layer_sizes = [
        (768, 768, "GPT-2 attn Q/K/V"),
        (768, 2304, "GPT-2 MLP concat"),
        (2304, 768, "GPT-2 MLP proj"),
        (768, 50257, "GPT-2 lm_head"),
        (50257, 768, "GPT-2 embedding"),
        (4096, 4096, "LLaMA-7B MLP"),
    ]
    
    print(f"{'Layer':>30s} {'PF(ms)':>8s} {'CUDA(ms)':>10s} {'PF/CUDA':>8s} {'PF GFLOPS':>10s} {'Err':>10s}")
    print("-" * 80)
    
    for M, K, label in layer_sizes:
        rng = np.random.default_rng(42)
        W = rng.standard_normal((M, K)).astype(np.float32) * 0.001
        x = rng.standard_normal(K).astype(np.float32) * 0.001
        
        # Pixelflow
        engine.upload_weights("bench", W)
        for _ in range(5):
            engine.linear("bench", x, reuse_fbo="bench_fbo")
        
        pf_times = []
        for _ in range(50):
            t0 = time.perf_counter()
            r = engine.linear("bench", x, reuse_fbo="bench_fbo")
            pf_times.append((time.perf_counter() - t0) * 1000)
        
        # PyTorch CUDA
        Wt = torch.tensor(W, device=device)
        xt = torch.tensor(x, device=device)
        for _ in range(5): _ = xt @ Wt.T
        torch.cuda.synchronize()
        
        pt_times = []
        for _ in range(50):
            torch.cuda.synchronize()
            t0 = time.perf_counter()
            _ = xt @ Wt.T
            torch.cuda.synchronize()
            pt_times.append((time.perf_counter() - t0) * 1000)
        
        expected = x @ W.T
        max_err = np.max(np.abs(r.flatten() - expected))
        
        pf_avg = np.mean(pf_times)
        pt_avg = np.mean(pt_times)
        gflops = (2 * M * K) / (pf_avg / 1000) / 1e9
        
        print(f"{label:>30s} {pf_avg:8.2f} {pt_avg:10.4f} {pf_avg/pt_avg:8.1f}x {gflops:10.1f} {max_err:10.6f}")
    
    engine.cleanup()
    print()
    print("PF = Pixelflow (fragment shader on NVIDIA GPU)")
    print("CUDA = PyTorch (tensor cores on same GPU)")


if __name__ == "__main__":
    run_full_benchmark()
