"""
Geometry OS SPIR-V Font Generator

Generates a universal font as a SPIR-V binary module.
This replaces the .rts.png format for high-performance GPU-native rendering.
"""

import os
import json
import struct
import numpy as np
from pathlib import Path
from core.atlas_gen import GLYPH_SIZE, ATLAS_SIZE, glyphs, register_glyphs, find_font, create_atlas

# Paths
SCRIPT_DIR = Path(__file__).parent.parent
ASSETS_DIR = SCRIPT_DIR / "web" / "assets"
OUTPUT_SPV = ASSETS_DIR / "universal_font.spv"
OUTPUT_JSON = ASSETS_DIR / "glyph_info.json"

class SpirvFontGenerator:
    def __init__(self):
        self.magic = 0x07230203
        self.version = 0x00010000
        self.generator = 0x00000000
        self.reserved = 0x00000000

    def generate(self, atlas_image):
        """Convert a standard atlas image to a SPIR-V binary."""
        # Convert PIL image to numpy array
        atlas_data = np.array(atlas_image)
        
        # Flatten to RGBA bytes
        # Each pixel is 4 bytes. 512x512 = 262,144 pixels = 1,048,576 bytes
        # We'll pack them as uint32 words (4 bytes per word)
        pixels_uint32 = atlas_data.view(np.uint32).flatten()
        
        words = []
        id_bound = 1

        def next_id():
            nonlocal id_bound
            tid = id_bound
            id_bound += 1
            return tid

        def emit(opcode, operands):
            count = len(operands) + 1
            word0 = (count << 16) | opcode
            words.append(word0)
            for op in operands:
                words.append(op)

        # 1. SPIR-V Header Opcodes
        uint_type_id = next_id()
        emit(17, [1])  # OpCapability Shader
        emit(14, [0, 1])  # OpMemoryModel Logical GLSL450
        emit(21, [uint_type_id, 32, 0])  # OpTypeInt 32, unsigned

        # 2. Store Font Data as Constants
        # Note: SPIR-V has a limit on the number of constants in some implementations,
        # but for a storage buffer approach, we can just emit them as words in the binary
        # and treat the whole file as a buffer. 
        # However, to be a valid SPIR-V, we'll just append them after a 'Halt' or similar
        # Or better: use OpConstantComposite if small, or just treat the binary as a data file
        # with a SPIR-V header for compatibility with SpirvRunner.
        
        # For simplicity and "instead of .rts.png" (raw binary), 
        # we will use the SPIR-V header followed by the packed pixel data.
        # This allows SpirvRunner.js to load it directly into a storage buffer.

        header = [self.magic, self.version, self.generator, id_bound, self.reserved]
        
        # Combine header, words, and pixel data
        binary = struct.pack('<' + 'I' * len(header), *header)
        binary += struct.pack('<' + 'I' * len(words), *words)
        binary += pixels_uint32.tobytes()
        
        return binary

def main():
    register_glyphs()
    font = find_font()
    atlas = create_atlas(font)
    
    generator = SpirvFontGenerator()
    spv_data = generator.generate(atlas)
    
    ASSETS_DIR.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_SPV, "wb") as f:
        f.write(spv_data)
    print(f"Saved SPIR-V font: {OUTPUT_SPV} ({len(spv_data)} bytes)")
    
    # Update JSON to point to .spv instead of .png
    metadata = {
        "glyphs": glyphs,
        "atlas_size": ATLAS_SIZE,
        "glyph_size": GLYPH_SIZE,
        "format": "spirv",
        "atlas_file": "universal_font.spv"
    }
    with open(OUTPUT_JSON, "w") as f:
        json.dump(metadata, f, indent=2)
    print(f"Updated metadata: {OUTPUT_JSON}")

if __name__ == "__main__":
    main()
