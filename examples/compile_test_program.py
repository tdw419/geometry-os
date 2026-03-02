"""
Test script to generate a visual program and compile it to SPIR-V.
Program: 10 20 +
"""

import numpy as np
from PIL import Image
import os
import sys
from pathlib import Path

# Add project root to path
sys.path.append(str(Path(__file__).parent.parent))
from geometry_os_spirv.scripts.visual_to_spirv import compile_visual_program

def create_test_program():
    # 2x2 glyph grid = 32x32 pixels
    data = np.zeros((32, 32, 4), dtype=np.uint8)
    
    def set_glyph(gx, gy, r, g, b):
        x, y = gx * 16, gy * 16
        data[y:y+16, x:x+16, 0] = r
        data[y:y+16, x:x+16, 1] = g
        data[y:y+16, x:x+16, 2] = b
        data[y:y+16, x:x+16, 3] = 255 # Opaque

    # Hilbert Order: (0,0) -> (1,0) -> (1,1) -> (0,1)
    set_glyph(0, 0, 255, 0, 10)    # Constant 10
    set_glyph(1, 0, 255, 0, 20)    # Constant 20
    set_glyph(1, 1, 255, 0x6A, 0)  # Instruction: +
    
    img = Image.fromarray(data)
    img.save("test_program.png")
    print("Created test_program.png")

if __name__ == "__main__":
    create_test_program()
    compile_visual_program("test_program.png", "test_program.spv")
    
    if os.path.exists("test_program.spv"):
        print("Success! SPIR-V binary generated.")
        # Print first few bytes in hex
        with open("test_program.spv", "rb") as f:
            header = f.read(20)
            print(f"Header: {header.hex(' ')}")
