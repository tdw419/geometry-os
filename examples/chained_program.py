"""
Test script to generate a chained visual program and compile it to SPIR-V.
Program: 10 20 + 5 *  (computes (10 + 20) * 5 = 150)

This demonstrates instruction chaining - multiple operations in sequence.
"""

import numpy as np
from PIL import Image
import os
import sys
from pathlib import Path

# Add project root to path
sys.path.append(str(Path(__file__).parent.parent))
from geometry_os_spirv.scripts.visual_to_spirv_v2 import compile_visual_program
from core.hilbert_util import HilbertCurve

# Opcode constants (G channel values)
GEO_FADD = 0x6A  # Float addition
GEO_FMUL = 0x6C  # Float multiplication


def create_chained_program():
    """
    Create a visual program that computes (10 + 20) * 5 = 150.

    Uses a 4x4 glyph grid (64x64 pixels) with order=2 Hilbert curve.

    Postfix notation: 10 20 + 5 *
    Stack trace:
      1. PUSH 10 -> stack: [10]
      2. PUSH 20 -> stack: [10, 20]
      3. ADD     -> stack: [30]
      4. PUSH 5  -> stack: [30, 5]
      5. MUL     -> stack: [150]
    """
    # 4x4 glyph grid = 64x64 pixels (16x16 per glyph)
    glyph_size = 16
    grid_size = 4  # 4x4 grid
    img_size = grid_size * glyph_size  # 64x64

    data = np.zeros((img_size, img_size, 4), dtype=np.uint8)

    def set_glyph(gx, gy, r, g, b):
        """Set a glyph at grid position (gx, gy) with RGB values."""
        x, y = gx * glyph_size, gy * glyph_size
        data[y:y+16, x:x+16, 0] = r
        data[y:y+16, x:x+16, 1] = g
        data[y:y+16, x:x+16, 2] = b
        data[y:y+16, x:x+16, 3] = 255  # Opaque

    # Get Hilbert curve positions
    curve = HilbertCurve(order=2)  # 4x4 grid

    # Program: 10 20 + 5 * (in Hilbert order)
    # d=0: PUSH 10 (G=0 means constant, B=value)
    # d=1: PUSH 20
    # d=2: ADD
    # d=3: PUSH 5
    # d=4: MUL

    operations = [
        (255, 0, 10),      # PUSH 10: R=255, G=0 (constant), B=10
        (255, 0, 20),      # PUSH 20: R=255, G=0 (constant), B=20
        (255, GEO_FADD, 0), # ADD: R=255, G=0x6A, B=0
        (255, 0, 5),       # PUSH 5: R=255, G=0 (constant), B=5
        (255, GEO_FMUL, 0), # MUL: R=255, G=0x6C, B=0
    ]

    for d, (r, g, b) in enumerate(operations):
        gx, gy = curve.d2xy(d)
        set_glyph(gx, gy, r, g, b)
        print(f"  d={d} -> ({gx},{gy}): R={r}, G={g:#04x}, B={b}")

    img = Image.fromarray(data)
    img.save("chained_program.png")
    print(f"\nCreated chained_program.png ({img_size}x{img_size} pixels)")
    return True


def main():
    print("=" * 50)
    print("Geometry OS - Chained Instruction Program")
    print("Computing: (10 + 20) * 5 = 150")
    print("=" * 50)

    # Step 1: Create the visual program
    print("\n[1/3] Creating visual program...")
    create_chained_program()

    # Step 2: Compile to SPIR-V
    print("\n[2/3] Compiling to SPIR-V...")
    success = compile_visual_program("chained_program.png", "chained_program.spv")

    if not success:
        print("ERROR: Compilation failed!")
        return 1

    # Step 3: Verify the binary exists
    print("\n[3/3] Verifying output...")
    if os.path.exists("chained_program.spv"):
        size = os.path.getsize("chained_program.spv")
        print(f"Success! Generated chained_program.spv ({size} bytes)")

        # Print first few bytes in hex
        with open("chained_program.spv", "rb") as f:
            header = f.read(20)
            print(f"Header: {header.hex(' ')}")
        return 0
    else:
        print("ERROR: SPIR-V file not created!")
        return 1


if __name__ == "__main__":
    sys.exit(main())
