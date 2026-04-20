#!/usr/bin/env python3
"""Train a small XOR neural network and export weights as Geometry OS assembly.

Architecture: 2 -> 4 -> 1 (small enough to fit in VM RAM)
Fixed-point: 16.16 (multiply float by 65536, round to int)

The VM runs the forward pass using MATVEC + RELU opcodes.
"""

import numpy as np
import sys
import os

# Fixed-point scale: 16.16
FP_SCALE = 65536.0

def to_fixed(val):
    """Convert float to fixed-point u32 (16.16)"""
    return int(round(float(val) * FP_SCALE)) & 0xFFFFFFFF

def sigmoid(x):
    return 1.0 / (1.0 + np.exp(-x))

def sigmoid_deriv(x):
    return x * (1.0 - x)

def train_xor(hidden_size=4, epochs=10000, lr=0.5):
    """Train a 2->hidden->1 network on XOR"""
    # Training data
    X = np.array([[0, 0], [0, 1], [1, 0], [1, 1]])
    Y = np.array([[0], [1], [1], [0]])

    np.random.seed(42)
    # Xavier initialization
    w1 = np.random.randn(2, hidden_size) * np.sqrt(2.0 / 2)
    b1 = np.zeros((1, hidden_size))
    w2 = np.random.randn(hidden_size, 1) * np.sqrt(2.0 / hidden_size)
    b2 = np.zeros((1, 1))

    for epoch in range(epochs):
        # Forward
        h = sigmoid(X @ w1 + b1)
        out = sigmoid(h @ w2 + b2)

        # Backward
        error = Y - out
        d_out = error * sigmoid_deriv(out)
        d_h = d_out @ w2.T * sigmoid_deriv(h)

        w2 += h.T @ d_out * lr
        b2 += np.sum(d_out, axis=0, keepdims=True) * lr
        w1 += X.T @ d_h * lr
        b1 += np.sum(d_h, axis=0, keepdims=True) * lr

        if epoch % 2000 == 0:
            loss = np.mean(error ** 2)
            print(f"Epoch {epoch}: loss={loss:.6f}")

    # Verify
    h = sigmoid(X @ w1 + b1)
    out = sigmoid(h @ w2 + b2)
    print("\nResults:")
    for i in range(4):
        print(f"  XOR({X[i][0]}, {X[i][1]}) = {out[i][0]:.4f} (expected {Y[i][0]})")

    return w1, b1, w2, b2

def export_assembly(w1, b1, w2, b2, out_path):
    """Export weights as Geometry OS assembly LDI/STORE sequences"""
    lines = []
    lines.append("; XOR Neural Network Weights")
    lines.append("; Architecture: 2 -> 4 -> 1")
    lines.append("; Fixed-point 16.16 format")
    lines.append("")

    # RAM layout:
    # 0x1000: input (2 values)
    # 0x1100: hidden layer weights (2*4 = 8 values) + bias (4 values) = 12 values
    # 0x1200: hidden output (4 values)
    # 0x1300: output weights (4*1 = 4 values) + bias (1 value) = 5 values
    # 0x1400: final output (1 value)

    INPUT_ADDR = 0x1000
    W1_ADDR = 0x1100
    B1_ADDR = 0x1160
    HIDDEN_ADDR = 0x1200
    W2_ADDR = 0x1300
    B2_ADDR = 0x1340
    OUTPUT_ADDR = 0x1400

    # Store weights layer 1 (row-major: 2 rows x 4 cols)
    lines.append("; === Layer 1 Weights (2x4) ===")
    for i in range(w1.shape[0]):
        for j in range(w1.shape[1]):
            addr = W1_ADDR + i * w1.shape[1] + j
            val = to_fixed(w1[i][j])
            lines.append(f"  LDI r1, {val}  ; w1[{i}][{j}] = {w1[i][j]:.4f}")
            lines.append(f"  STORE r1, {addr}")

    # Store bias layer 1
    lines.append("")
    lines.append("; === Layer 1 Bias (4) ===")
    for j in range(b1.shape[1]):
        addr = B1_ADDR + j
        val = to_fixed(b1[0][j])
        lines.append(f"  LDI r1, {val}  ; b1[{j}] = {b1[0][j]:.4f}")
        lines.append(f"  STORE r1, {addr}")

    # Store weights layer 2 (4x1)
    lines.append("")
    lines.append("; === Layer 2 Weights (4x1) ===")
    for i in range(w2.shape[0]):
        for j in range(w2.shape[1]):
            addr = W2_ADDR + i * w2.shape[1] + j
            val = to_fixed(w2[i][j])
            lines.append(f"  LDI r1, {val}  ; w2[{i}][{j}] = {w2[i][j]:.4f}")
            lines.append(f"  STORE r1, {addr}")

    # Store bias layer 2
    lines.append("")
    lines.append("; === Layer 2 Bias (1) ===")
    val = to_fixed(b2[0][0])
    lines.append(f"  LDI r1, {val}  ; b2[0] = {b2[0][0]:.4f}")
    lines.append(f"  STORE r1, {B2_ADDR}")

    lines.append("")
    lines.append("; === Weight addresses ===")
    lines.append(f"; INPUT_ADDR  = 0x{INPUT_ADDR:04X}")
    lines.append(f"; W1_ADDR     = 0x{W1_ADDR:04X}")
    lines.append(f"; B1_ADDR     = 0x{B1_ADDR:04X}")
    lines.append(f"; HIDDEN_ADDR = 0x{HIDDEN_ADDR:04X}")
    lines.append(f"; W2_ADDR     = 0x{W2_ADDR:04X}")
    lines.append(f"; B2_ADDR     = 0x{B2_ADDR:04X}")
    lines.append(f"; OUTPUT_ADDR = 0x{OUTPUT_ADDR:04X}")

    with open(out_path, 'w') as f:
        f.write('\n'.join(lines))
    print(f"\nWeights written to {out_path}")
    print(f"W1: {w1.shape}, B1: {b1.shape}, W2: {w2.shape}, B2: {b2.shape}")

    return {
        'INPUT_ADDR': INPUT_ADDR,
        'W1_ADDR': W1_ADDR,
        'B1_ADDR': B1_ADDR,
        'HIDDEN_ADDR': HIDDEN_ADDR,
        'W2_ADDR': W2_ADDR,
        'B2_ADDR': B2_ADDR,
        'OUTPUT_ADDR': OUTPUT_ADDR,
        'w1_shape': w1.shape,
        'w2_shape': w2.shape,
    }

def export_reference_results(w1, b1, w2, b2):
    """Compute expected fixed-point outputs for verification in tests"""
    X = np.array([[0, 0], [0, 1], [1, 0], [1, 1]])

    print("\nFixed-point reference outputs:")
    for i in range(4):
        h = sigmoid(X[i] @ w1 + b1)
        out = sigmoid(h @ w2 + b2)
        fp_out = to_fixed(out[0])
        print(f"  XOR({X[i][0]}, {X[i][1]}): float={float(out[0]):.4f}, fixed=0x{fp_out:08X} ({fp_out})")

if __name__ == "__main__":
    hidden = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    w1, b1, w2, b2 = train_xor(hidden_size=hidden)
    addrs = export_assembly(w1, b1, w2, b2, "tools/xor_weights.asm")
    export_reference_results(w1, b1, w2, b2)
