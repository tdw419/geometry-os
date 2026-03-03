"""
Geometry OS Visual Compiler (SPIR-V)

Converts a visual program PNG into a SPIR-V binary module.
Uses a Stack-based (Postfix) execution model.
"""

import sys
import os
import numpy as np
from PIL import Image
from pathlib import Path

# Add project root to path for core utilities
sys.path.append(str(Path(__file__).parent.parent.parent))
from core.hilbert_util import HilbertCurve
from geometry_os_spirv.scripts.emit_spirv import *

def compile_visual_program(image_path, output_path):
    print(f"[GOS Compiler] Compiling {image_path}...")
    
    # Load image
    img = Image.open(image_path).convert("RGBA")
    data = np.array(img)
    
    h, w, _ = data.shape
    glyph_size = 16
    grid_size = w // glyph_size
    
    # We assume a square grid for Hilbert mapping
    order = int(np.log2(grid_size))
    curve = HilbertCurve(order)
    
    emitter = SPIRVEmitter()
    
    # 1. Setup Types and Basic IDs
    float_id = emitter.next_id()
    void_id = emitter.next_id()
    func_type_id = emitter.next_id()
    main_func_id = emitter.next_id()
    label_id = emitter.next_id()
    
    emitter.emit(OP_CAPABILITY, 1)  # Shader
    emitter.emit(OP_MEMORY_MODEL, 0, 1) # Logical, GLSL450
    
    emitter.emit(OP_TYPE_FLOAT, float_id, 32)
    emitter.emit(OP_TYPE_VOID, void_id)
    emitter.emit(OP_TYPE_FUNCTION, func_type_id, void_id)
    
    # Start main function
    emitter.emit(OP_FUNCTION, void_id, main_func_id, 0, func_type_id)
    emitter.emit(OP_LABEL, label_id)
    
    # 2. Execution Loop
    stack = []
    
    # Read glyphs in Hilbert order
    for d in range(grid_size * grid_size):
        gx, gy = curve.d2xy(d)
        px, py = gx * glyph_size, gy * glyph_size
        
        # Sample the center pixel of the glyph for semantic data
        # (Assuming the entire glyph grid carries the same RGB)
        sample = data[py + glyph_size // 2, px + glyph_size // 2]
        r, g, b, a = sample
        
        if a < 128: continue # Skip empty/transparent pixels
        
        # Opcode processing
        if g < 128:
            # Constant Push (B channel is the value)
            val = float(b)
            cid = emitter.next_id()
            emitter.emit(OP_CONSTANT, float_id, cid, val)
            stack.append(cid)
        else:
            # Instruction Execution
            if g == 0x6A: # + (OpFAdd)
                if len(stack) >= 2:
                    v2 = stack.pop()
                    v1 = stack.pop()
                    rid = emitter.next_id()
                    emitter.emit(OP_FADD, float_id, rid, v1, v2)
                    stack.append(rid)
            elif g == 0x6B: # - (OpFSub)
                if len(stack) >= 2:
                    v2 = stack.pop()
                    v1 = stack.pop()
                    rid = emitter.next_id()
                    emitter.emit(OP_FSUB, float_id, rid, v1, v2)
                    stack.append(rid)
            elif g == 0x6C: # * (OpFMul)
                if len(stack) >= 2:
                    v2 = stack.pop()
                    v1 = stack.pop()
                    rid = emitter.next_id()
                    emitter.emit(OP_FMUL, float_id, rid, v1, v2)
                    stack.append(rid)
            # Add more opcodes as needed...

    # Finalize function
    emitter.emit(OP_RETURN)
    emitter.emit(OP_FUNCTION_END)
    
    # Save binary
    binary = emitter.finalize()
    with open(output_path, "wb") as f:
        f.write(binary)
        
    print(f"✅ Success: Generated {output_path} ({len(binary)} bytes)")
    return True

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python3 visual_to_spirv.py <input.png> <output.spv>")
    else:
        compile_visual_program(sys.argv[1], sys.argv[2])
