"""Benchmark pixelflow GPU texture matmul against numpy and PyTorch.

Measures:
1. Pure numpy matmul (CPU)
2. PyTorch CUDA matmul (GPU tensor cores)
3. Pixelflow fragment shader matmul (GPU rasterization pipeline)
4. Memory bandwidth utilization

Usage:
    python -m pixelflow.benchmark [--sizes 64,128,256,512,1024] [--mode float32]
"""

import argparse
import time
import numpy as np
import sys


def bench_numpy(sizes, runs=20):
    """Benchmark numpy matmul for given sizes."""
    print("\n=== NumPy (CPU) ===")
    results = {}
    for n in sizes:
        rng = np.random.default_rng(42)
        W = rng.standard_normal((n, n)).astype(np.float32)
        x = rng.standard_normal(n).astype(np.float32)
        
        # Warmup
        for _ in range(3):
            _ = x @ W.T
        
        times = []
        for _ in range(runs):
            t0 = time.perf_counter()
            _ = x @ W.T
            times.append((time.perf_counter() - t0) * 1000)
        
        avg = np.mean(times)
        std = np.std(times)
        gflops = (2 * n * n) / (avg / 1000) / 1e9
        results[n] = {"time_ms": avg, "std_ms": std, "gflops": gflops}
        print(f"  {n:5d}x{n:<5d}  {avg:8.3f} ± {std:.3f} ms  {gflops:8.2f} GFLOPS")
    
    return results


def bench_pytorch(sizes, runs=20):
    """Benchmark PyTorch CUDA matmul."""
    try:
        import torch
    except ImportError:
        print("\n=== PyTorch (CUDA) -- NOT AVAILABLE ===")
        return {}
    
    if not torch.cuda.is_available():
        print("\n=== PyTorch (CUDA) -- No CUDA device ===")
        return {}
    
    device = torch.device("cuda")
    print(f"\n=== PyTorch ({torch.cuda.get_device_name()}) ===")
    
    results = {}
    for n in sizes:
        W = torch.randn(n, n, device=device, dtype=torch.float32)
        x = torch.randn(n, device=device, dtype=torch.float32)
        
        # Warmup
        for _ in range(5):
            _ = x @ W.T
        torch.cuda.synchronize()
        
        times = []
        for _ in range(runs):
            torch.cuda.synchronize()
            t0 = time.perf_counter()
            _ = x @ W.T
            torch.cuda.synchronize()
            times.append((time.perf_counter() - t0) * 1000)
        
        avg = np.mean(times)
        std = np.std(times)
        gflops = (2 * n * n) / (avg / 1000) / 1e9
        results[n] = {"time_ms": avg, "std_ms": std, "gflops": gflops}
        print(f"  {n:5d}x{n:<5d}  {avg:8.3f} ± {std:.3f} ms  {gflops:8.2f} GFLOPS")
    
    return results


def bench_pixelflow(sizes, mode="float32", runs=20):
    """Benchmark pixelflow fragment shader matmul."""
    from pixelflow.engine import PixelEngine
    
    print(f"\n=== Pixelflow (fragment shader, {mode}) ===")
    
    results = {}
    for n in sizes:
        rng = np.random.default_rng(42)
        W = rng.standard_normal((n, n)).astype(np.float32)
        x = rng.standard_normal(n).astype(np.float32)
        
        engine = PixelEngine(headless=True)
        engine.upload_weight_matrix("W", W, mode=mode)
        
        # Warmup
        for _ in range(3):
            _ = engine.matmul("W", x, out_features=n, in_features=n, mode=mode)
        
        times = []
        for _ in range(runs):
            t0 = time.perf_counter()
            result = engine.matmul("W", x, out_features=n, in_features=n, mode=mode)
            times.append((time.perf_counter() - t0) * 1000)
        
        avg = np.mean(times)
        std = np.std(times)
        gflops = (2 * n * n) / (avg / 1000) / 1e9
        
        # Accuracy
        ref = x @ W.T
        max_err = np.max(np.abs(result.flatten() - ref.flatten()))
        
        results[n] = {
            "time_ms": avg, "std_ms": std, "gflops": gflops,
            "max_error": max_err
        }
        print(f"  {n:5d}x{n:<5d}  {avg:8.3f} ± {std:.3f} ms  {gflops:8.2f} GFLOPS  "
              f"max_err={max_err:.6f}")
        
        engine.cleanup()
    
    return results


def main():
    parser = argparse.ArgumentParser(description="Benchmark pixelflow")
    parser.add_argument("--sizes", default="64,128,256,512,1024",
                       help="Comma-separated matrix sizes")
    parser.add_argument("--mode", default="float32",
                       choices=["float32", "float16", "q8"])
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--skip-pytorch", action="store_true")
    args = parser.parse_args()
    
    sizes = [int(s) for s in args.sizes.split(",")]
    
    print("=" * 60)
    print("PIXELFLOW BENCHMARK")
    print("=" * 60)
    print(f"Sizes: {sizes}")
    print(f"Mode:  {args.mode}")
    print(f"Runs:  {args.runs}")
    
    numpy_results = bench_numpy(sizes, args.runs)
    
    if not args.skip_pytorch:
        pytorch_results = bench_pytorch(sizes, args.runs)
    
    pixelflow_results = bench_pixelflow(sizes, args.mode, args.runs)
    
    # Summary comparison
    print("\n" + "=" * 60)
    print("COMPARISON SUMMARY")
    print("=" * 60)
    print(f"{'Size':>10s} {'NumPy ms':>10s} {'PF ms':>10s} {'PF/np':>8s} {'PF GFLOPS':>10s}")
    print("-" * 50)
    for n in sizes:
        np_t = numpy_results.get(n, {}).get("time_ms", float('nan'))
        pf_t = pixelflow_results.get(n, {}).get("time_ms", float('nan'))
        pf_gf = pixelflow_results.get(n, {}).get("gflops", float('nan'))
        ratio = pf_t / np_t if np_t > 0 else float('nan')
        print(f"{n:10d} {np_t:10.3f} {pf_t:10.3f} {ratio:8.2f}x {pf_gf:10.2f}")


if __name__ == "__main__":
    main()
