"""Pixelflow inference engine -- GPU fragment shader inference.

Creates an OpenGL context, uploads weight textures, and runs
fragment shader programs for LLM inference operations.
"""

import numpy as np
import moderngl
import time
from pathlib import Path

SHADER_DIR = Path(__file__).parent / "shaders"

# Shared vertex shader for fullscreen quad (uses gl_VertexID, no VBO needed)
QUAD_VS = """
#version 450
void main() {
    float x = -1.0 + float(gl_VertexID % 2) * 2.0;
    float y = -1.0 + float(gl_VertexID / 2) * 2.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}
"""


class PixelEngine:
    """GPU inference engine using fragment shaders and texture-encoded weights."""
    
    def __init__(self, headless: bool = True):
        if headless:
            self.ctx = moderngl.create_standalone_context()
        else:
            raise NotImplementedError("Windowed mode not yet implemented")
        
        self.textures = {}
        self.programs = {}
        self._load_shaders()
    
    def _load_shaders(self):
        """Load all built-in shaders."""
        matmul_src = (SHADER_DIR / "matmul.glsl").read_text()
        self.programs["matmul"] = self.ctx.program(
            vertex_shader=QUAD_VS,
            fragment_shader=matmul_src,
        )
    
    def upload_weight_matrix(self, name: str, matrix: np.ndarray,
                             mode: str = "float32") -> moderngl.Texture:
        """Upload a weight matrix as a GPU texture.
        
        Args:
            name: Texture name
            matrix: 2D numpy array (M x K) where M=out_features, K=in_features
            mode: "float32", "float16", or "q8"
        """
        M, K = matrix.shape
        flat = matrix.astype(np.float32)
        
        if mode == "float32":
            tex = self.ctx.texture((K, M), components=1, dtype="f4")
            tex.write(flat.tobytes())
            
        elif mode == "float16":
            f16 = flat.astype(np.float16)
            tex = self.ctx.texture((K, M), components=1, dtype="f2")
            tex.write(f16.tobytes())
            
        elif mode == "q8":
            from .weight_textures import quantize_to_int8
            q, scale, zp = quantize_to_int8(flat.flatten())
            q_matrix = q.reshape(M, K)
            # Pad K to multiple of 4
            K_pad = (K + 3) // 4 * 4
            padded = np.zeros((M, K_pad), dtype=np.int8)
            padded[:, :K] = q_matrix
            data = padded.view(np.uint8)
            tex = self.ctx.texture((K_pad // 4, M), components=4, dtype="u1")
            tex.write(data.tobytes())
            tex.extra = {"scale": scale, "zp": zp, "mode": "q8", "K_pad": K_pad}
        else:
            raise ValueError(f"Unknown mode: {mode}")
        
        self.textures[name] = tex
        return tex
    
    def matmul(self, weight_name: str, input_vec: np.ndarray,
               mode: str = "float32") -> np.ndarray:
        """Compute output = input @ weight.T via fragment shader.
        
        Args:
            weight_name: Name of uploaded weight texture (M x K)
            input_vec: Input array, 1D (K,) or 2D (batch, K)
            mode: Must match the weight texture mode
            
        Returns:
            Output array (batch, M)
        """
        if input_vec.ndim == 1:
            input_vec = input_vec.reshape(1, -1)
        
        batch, K = input_vec.shape
        weight_tex = self.textures[weight_name]
        
        if mode == "q8":
            M = weight_tex.size[1]
            K_pad = weight_tex.extra["K_pad"]
            w_scale = weight_tex.extra["scale"]
        else:
            M = weight_tex.size[1]  # height = out_features
            K_pad = K
            w_scale = 1.0
        
        # Upload input as texture
        input_tex = self.ctx.texture((K, batch), components=1, dtype="f4")
        input_tex.write(input_vec.astype(np.float32).tobytes())
        
        # Create output FBO
        out_tex = self.ctx.texture((M, batch), components=1, dtype="f4")
        fbo = self.ctx.framebuffer(color_attachments=[out_tex])
        fbo.use()
        fbo.viewport = (0, 0, M, batch)
        
        # Bind textures
        weight_tex.use(0)
        input_tex.use(1)
        
        # Set uniforms
        prog = self.programs["matmul"]
        prog["u_weights"].value = 0
        prog["u_input"].value = 1
        prog["u_K"].value = K
        prog["u_Ww"].value = weight_tex.size[0]  # texture width
        prog["u_Wh"].value = weight_tex.size[1]  # texture height (= M)
        prog["u_batch"].value = batch
        prog["u_w_scale"].value = w_scale
        prog["u_a_scale"].value = 1.0  # input not quantized
        
        if mode in ("float32", "float16"):
            prog["u_mode"].value = 0
        elif mode == "q8":
            prog["u_mode"].value = 2
        
        # Render
        vao = self.ctx.vertex_array(prog, [], mode=moderngl.TRIANGLE_STRIP)
        vao.render(vertices=4)
        
        # Read back
        output = np.frombuffer(out_tex.read(), dtype=np.float32)
        return output.reshape(batch, M)
    
    def benchmark_matmul(self, M: int, K: int, mode: str = "float32",
                         warmup: int = 5, runs: int = 50) -> dict:
        """Benchmark matmul vs numpy reference."""
        rng = np.random.default_rng(42)
        W = rng.standard_normal((M, K)).astype(np.float32) * 0.1
        x = rng.standard_normal(K).astype(np.float32) * 0.1
        
        self.upload_weight_matrix("bench_W", W, mode=mode)
        
        # Warmup
        for _ in range(warmup):
            self.matmul("bench_W", x, mode=mode)
        
        # GPU runs
        gpu_times = []
        for _ in range(runs):
            t0 = time.perf_counter()
            result = self.matmul("bench_W", x, mode=mode)
            gpu_times.append((time.perf_counter() - t0) * 1000)
        
        # Numpy reference
        expected = x @ W.T
        max_err = np.max(np.abs(result.flatten() - expected))
        mean_err = np.mean(np.abs(result.flatten() - expected))
        
        # Numpy benchmark
        np_times = []
        for _ in range(runs):
            t0 = time.perf_counter()
            _ = x @ W.T
            np_times.append((time.perf_counter() - t0) * 1000)
        
        return {
            "shape": f"({M}, {K})",
            "mode": mode,
            "gpu_ms": np.mean(gpu_times),
            "gpu_std": np.std(gpu_times),
            "numpy_ms": np.mean(np_times),
            "numpy_std": np.std(np_times),
            "speedup": np.mean(np_times) / np.mean(gpu_times),
            "max_error": max_err,
            "mean_error": mean_err,
        }
    
    def cleanup(self):
        """Release all GPU resources."""
        for tex in self.textures.values():
            tex.release()
        for prog in self.programs.values():
            prog.release()
        self.textures.clear()
        self.programs.clear()
        self.ctx.release()
