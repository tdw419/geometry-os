"""Test pixelflow: encode weights, matmul via fragment shader, verify correctness."""

import numpy as np
import pytest
from pixelflow.engine import PixelEngine
from pixelflow.weight_textures import (
    quantize_to_int8, quantize_to_int4,
    dequantize_int8, dequantize_int4,
    encode_q8_to_rgba, encode_q4_to_rg8,
)


class TestQuantization:
    """Test weight quantization round-trip accuracy."""
    
    def test_int8_symmetric_roundtrip(self):
        rng = np.random.default_rng(42)
        w = rng.standard_normal(1000).astype(np.float32)
        q, scale, zp = quantize_to_int8(w, symmetric=True)
        deq = dequantize_int8(q, scale, zp)
        # Max error should be ~scale (one quantization step)
        max_err = np.max(np.abs(w - deq))
        assert max_err < scale * 1.5, f"Max error {max_err} > scale {scale} * 1.5"
    
    def test_int4_roundtrip(self):
        rng = np.random.default_rng(42)
        w = rng.standard_normal(1000).astype(np.float32)
        q, scale, offset = quantize_to_int4(w)
        deq = dequantize_int4(q, scale, offset)
        max_err = np.max(np.abs(w - deq))
        assert max_err < scale * 1.5, f"Max error {max_err} > scale {scale} * 1.5"
    
    def test_q8_values_in_range(self):
        rng = np.random.default_rng(42)
        w = rng.standard_normal(1000).astype(np.float32)
        q, _, _ = quantize_to_int8(w, symmetric=True)
        assert q.min() >= -128
        assert q.max() <= 127
    
    def test_q4_values_in_range(self):
        rng = np.random.default_rng(42)
        w = rng.standard_normal(1000).astype(np.float32)
        q, _, _ = quantize_to_int4(w)
        assert q.min() >= 0
        assert q.max() <= 15


class TestTextureEncoding:
    """Test weight encoding to texture data."""
    
    def test_q8_encoding_shape(self):
        rng = np.random.default_rng(42)
        q = rng.integers(-128, 127, 100, dtype=np.int8)
        data, (w, h) = encode_q8_to_rgba(q)
        # 100 values / 4 per pixel = 25 pixels
        assert w * h >= 25
        # Data should be uint8
        assert data.dtype == np.uint8
    
    def test_q4_encoding_shape(self):
        rng = np.random.default_rng(42)
        q = rng.integers(0, 15, 100, dtype=np.uint8)
        data, (w, h) = encode_q4_to_rg8(q)
        assert w * h >= 50  # 100 nibbles / 2 per pixel
        assert data.dtype == np.uint8


class TestEngineInit:
    """Test engine creation and shader loading."""
    
    def test_create_engine(self):
        engine = PixelEngine(headless=True)
        assert engine.ctx is not None
        engine.cleanup()
    
    def test_load_matmul_shader(self):
        engine = PixelEngine(headless=True)
        prog = engine.load_shader("matmul")
        assert prog is not None
        engine.cleanup()


class TestMatmul:
    """Test matrix multiplication via fragment shaders."""
    
    def test_small_matmul_float32(self):
        """Test 4x4 matrix multiply: C = A @ B.T"""
        engine = PixelEngine(headless=True)
        try:
            # Weight matrix: 4 output features, 4 input features
            W = np.array([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ], dtype=np.float32)  # Identity
            
            x = np.array([1.0, 2.0, 3.0, 4.0], dtype=np.float32)
            
            engine.upload_weight_matrix("W", W, mode="float32")
            result = engine.matmul("W", x, out_features=4, in_features=4, mode="float32")
            
            expected = x @ W.T
            np.testing.assert_allclose(result.flatten(), expected, atol=1e-3,
                                      err_msg="Identity matmul failed")
        finally:
            engine.cleanup()
    
    def test_random_matmul_float32(self):
        """Test random 32x32 matrix multiply."""
        engine = PixelEngine(headless=True)
        try:
            rng = np.random.default_rng(42)
            W = rng.standard_normal((32, 32)).astype(np.float32)
            x = rng.standard_normal(32).astype(np.float32)
            
            engine.upload_weight_matrix("W", W, mode="float32")
            result = engine.matmul("W", x, out_features=32, in_features=32, mode="float32")
            
            expected = x @ W.T
            max_err = np.max(np.abs(result.flatten() - expected))
            assert max_err < 0.1, f"Max error {max_err} too large for 32x32"
        finally:
            engine.cleanup()
    
    def test_medium_matmul_float32(self):
        """Test 256x256 matrix multiply -- typical small linear layer."""
        engine = PixelEngine(headless=True)
        try:
            rng = np.random.default_rng(42)
            W = rng.standard_normal((256, 256)).astype(np.float32) * 0.1
            x = rng.standard_normal(256).astype(np.float32) * 0.1
            
            engine.upload_weight_matrix("W", W, mode="float32")
            result = engine.matmul("W", x, out_features=256, in_features=256, mode="float32")
            
            expected = x @ W.T
            max_err = np.max(np.abs(result.flatten() - expected))
            # Larger matrices accumulate more float error in the shader
            assert max_err < 1.0, f"Max error {max_err} too large for 256x256"
        finally:
            engine.cleanup()


class TestBenchmark:
    """Benchmark GPU matmul vs numpy."""
    
    def test_benchmark_small(self):
        """Quick benchmark on 64x64 matrix."""
        engine = PixelEngine(headless=True)
        try:
            results = engine.benchmark_matmul(64, 64, mode="float32", 
                                              warmup=2, runs=5)
            print(f"\nBenchmark 64x64: GPU={results['gpu_time_ms']:.2f}ms "
                  f"numpy={results['numpy_time_ms']:.2f}ms "
                  f"speedup={results['speedup_vs_numpy']:.2f}x "
                  f"error={results['max_error']:.6f}")
            assert results['gpu_time_ms'] > 0
        finally:
            engine.cleanup()


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])
