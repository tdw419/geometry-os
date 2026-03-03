"""
Geometry OS Font Atlas Generator

Generates a universal font atlas with:
- Standard ASCII glyphs (32-126)
- GeoASM instruction glyphs (128+)
- Symmetry enforcement for morphological consistency

Output: web/assets/universal_font.rts.png + glyph_info.json
"""

import os
import json
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

# Configuration
GLYPH_SIZE = 16
ATLAS_SIZE = 512  # 32x32 glyphs = 1024 slots

# Paths (relative to toolkit root)
SCRIPT_DIR = Path(__file__).parent.parent
ASSETS_DIR = SCRIPT_DIR / "web" / "assets"
OUTPUT_ATLAS = ASSETS_DIR / "universal_font.rts.png"
OUTPUT_JSON = ASSETS_DIR / "glyph_info.json"

# Symmetry types
SYM_NONE = 0
SYM_ROT_90 = 1
SYM_ASYMMETRIC = 2
SYM_GRID = 3
SYM_INV_POINT = 4

# Glyph registry
glyphs = {}

def add_glyph(char, code, r=255, g=0, b=0, is_instruction=False, sym_type=SYM_NONE):
    """Register a glyph in the atlas."""
    glyphs[code] = {
        "char": char,
        "r": r, "g": g, "b": b,
        "is_instruction": is_instruction,
        "symmetry": sym_type
    }

def register_glyphs():
    """Register all glyphs for the atlas."""
    # Standard ASCII (32-126)
    for i in range(32, 127):
        add_glyph(chr(i), i, r=255)

    # GeoASM Instructions (128-150)
    add_glyph('+', 128, g=0x6A, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('-', 129, g=0x6B, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('*', 130, g=0x6C, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('/', 136, g=0x6D, is_instruction=True, sym_type=SYM_GRID)
    add_glyph('sin', 137, g=0x70, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('cos', 138, g=0x71, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('>', 131, g=0x10, is_instruction=True, sym_type=SYM_ASYMMETRIC)
    add_glyph('<', 139, g=0x11, is_instruction=True, sym_type=SYM_ASYMMETRIC)
    add_glyph('st', 140, g=0x72, is_instruction=True, sym_type=SYM_GRID)
    add_glyph('ld', 141, g=0x73, is_instruction=True, sym_type=SYM_GRID)
    add_glyph('?', 132, g=0x45, is_instruction=True, sym_type=SYM_INV_POINT)
    add_glyph(':', 142, g=0x46, is_instruction=True, sym_type=SYM_ROT_90)
    add_glyph('rect', 143, g=0x80, is_instruction=True, sym_type=SYM_GRID)
    add_glyph('clr', 144, g=0x81, is_instruction=True, sym_type=SYM_GRID)
    add_glyph('=', 133, g=0x21, is_instruction=True, sym_type=SYM_GRID)

    # ========================================
    # MORPHOLOGICAL VISUAL LANGUAGE (160-199)
    # Self-hosting compiler glyphs
    # ========================================

    # Arithmetic Operations (FADD, FSUB, FMUL, FDIV)
    add_glyph('⊕', 160, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)   # Circle-Plus = FADD
    add_glyph('⊖', 161, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)   # Circle-Minus = FSUB
    add_glyph('⊗', 162, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)   # Circle-Times = FMUL
    add_glyph('⊘', 163, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)   # Circle-Div = FDIV

    # Memory Operations (STORE, LOAD, SHARED_STORE, SHARED_LOAD)
    add_glyph('→', 164, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Arrow-Right = STORE
    add_glyph('←', 165, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Arrow-Left = LOAD
    add_glyph('⇒', 166, r=100, g=255, b=255, is_instruction=True, sym_type=SYM_ASYMMETRIC)  # Double-Arrow-R = SHARED_STORE
    add_glyph('⇐', 167, r=100, g=255, b=255, is_instruction=True, sym_type=SYM_ASYMMETRIC)  # Double-Arrow-L = SHARED_LOAD

    # Control Flow (JMP, JZ, JNZ, LABEL, RETURN)
    add_glyph('↻', 168, r=255, g=200, b=100, is_instruction=True, sym_type=SYM_ROT_90)     # Loop = JMP
    add_glyph('∅', 169, r=255, g=150, b=50, is_instruction=True, sym_type=SYM_INV_POINT)   # Empty = JZ
    add_glyph('◉', 170, r=255, g=200, b=50, is_instruction=True, sym_type=SYM_ROT_90)     # Diamond = JNZ
    add_glyph('█', 171, r=150, g=150, b=255, is_instruction=True, sym_type=SYM_GRID)       # Block = LABEL
    add_glyph('▣', 172, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_GRID)       # End-Block = RETURN

    # IPC Operations (MSG_SEND, MSG_RECV, MSG_PEEK)
    add_glyph('✉', 173, r=255, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)  # Envelope = MSG_SEND
    add_glyph('📥', 174, r=100, g=255, b=255, is_instruction=True, sym_type=SYM_ASYMMETRIC)  # Inbox = MSG_RECV
    add_glyph('👁', 175, r=255, g=200, b=200, is_instruction=True, sym_type=SYM_INV_POINT)  # Eye = MSG_PEEK

    # System Operations (SYSCALL, YIELD, CONSTANT)
    add_glyph('⚡', 176, r=255, g=255, b=50, is_instruction=True, sym_type=SYM_ROT_90)       # Lightning = SYSCALL
    add_glyph('⏸', 177, r=200, g=200, b=255, is_instruction=True, sym_type=SYM_ROT_90)       # Pause = YIELD
    add_glyph('◇', 178, r=255, g=255, b=255, is_instruction=True, sym_type=SYM_ROT_90)       # Diamond = CONSTANT

    # Comparison Operations
    add_glyph('≟', 179, r=255, g=200, b=100, is_instruction=True, sym_type=SYM_INV_POINT)    # Question = CMP_EQ
    add_glyph('≠', 180, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)       # Not-Equal = CMP_NE
    add_glyph('≤', 181, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Less-Equal = CMP_LE
    add_glyph('≥', 182, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Greater-Equal = CMP_GE

    # Stack Operations
    add_glyph('⬆', 183, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Up-Arrow = PUSH
    add_glyph('⬇', 184, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Down-Arrow = POP
    add_glyph('⇄', 185, r=100, g=255, b=255, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Swap-Arrow = SWAP
    add_glyph('⊗', 186, r=255, g=150, b=50, is_instruction=True, sym_type=SYM_ROT_90)        # Overlap = DUP

    # Process/Agent Glyphs
    add_glyph('▶', 187, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Play = SPAWN
    add_glyph('⏹', 188, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Stop = KILL
    add_glyph('🔄', 189, r=100, g=200, b=255, is_instruction=True, sym_type=SYM_ROT_90)      # Refresh = RESTART

    # Semantic/Memory Glyphs
    add_glyph('💠', 190, r=100, g=200, b=255, is_instruction=True, sym_type=SYM_ROT_90)      # Crystal = EMBED
    add_glyph('🔮', 191, r=200, g=100, b=255, is_instruction=True, sym_type=SYM_ROT_90)      # Crystal-Ball = PREDICT
    add_glyph('🧠', 192, r=255, g=200, b=200, is_instruction=True, sym_type=SYM_ROT_90)      # Brain = THINK

    # Hilbert/Spatial Glyphs
    add_glyph('🌀', 193, r=100, g=255, b=200, is_instruction=True, sym_type=SYM_ROT_90)      # Spiral = HILBERT_ENCODE
    add_glyph('📍', 194, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_INV_POINT)   # Pin = HILBERT_DECODE
    add_glyph('🗺', 195, r=100, g=200, b=255, is_instruction=True, sym_type=SYM_GRID)        # Map = NAVIGATE

    # Compiler Glyphs
    add_glyph('📝', 196, r=255, g=200, b=100, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Memo = PARSE
    add_glyph('🔨', 197, r=150, g=150, b=255, is_instruction=True, sym_type=SYM_ASYMMETRIC)   # Hammer = COMPILE
    add_glyph('✅', 198, r=100, g=255, b=100, is_instruction=True, sym_type=SYM_ROT_90)       # Check = VERIFY
    add_glyph('❌', 199, r=255, g=100, b=100, is_instruction=True, sym_type=SYM_ROT_90)       # X = ERROR

def find_font():
    """Find a suitable monospace font."""
    font_paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
    ]
    for path in font_paths:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, 12)
            except Exception:
                continue
    return ImageFont.load_default()

def apply_symmetry(pixels, sym_type):
    """Enforce geometric symmetry on alpha channel."""
    if sym_type == SYM_NONE:
        return pixels

    alpha = pixels[:, :, 3].astype(float)

    if sym_type == SYM_ROT_90:
        # 4-way rotational symmetry
        r1 = alpha
        r2 = np.rot90(alpha, 1)
        r3 = np.rot90(alpha, 2)
        r4 = np.rot90(alpha, 3)
        alpha = (r1 + r2 + r3 + r4) / 4.0
    elif sym_type == SYM_ASYMMETRIC:
        # Directional bias
        mask = np.zeros(GLYPH_SIZE)
        mask[8:] = 1.0
        alpha = alpha * mask[None, :]
    elif sym_type == SYM_GRID:
        # Grid pattern
        grid = np.zeros((GLYPH_SIZE, GLYPH_SIZE))
        grid[::4, :] = 1.0
        grid[:, ::4] = 1.0
        alpha = np.maximum(alpha * grid, alpha * 0.1)
    elif sym_type == SYM_INV_POINT:
        # Point inversion symmetry
        inv = np.flip(alpha)
        alpha = (alpha + inv) / 2.0

    pixels[:, :, 3] = np.clip(alpha, 0, 255).astype(np.uint8)
    return pixels

def create_atlas(font, mode="standard"):
    """Generate atlas image for a given mode."""
    atlas = Image.new("RGBA", (ATLAS_SIZE, ATLAS_SIZE), (0, 0, 0, 0))

    for code, info in glyphs.items():
        col = code % (ATLAS_SIZE // GLYPH_SIZE)
        row = code // (ATLAS_SIZE // GLYPH_SIZE)
        x, y = col * GLYPH_SIZE, row * GLYPH_SIZE

        glyph_img = Image.new("RGBA", (GLYPH_SIZE, GLYPH_SIZE), (0, 0, 0, 0))
        glyph_draw = ImageDraw.Draw(glyph_img)

        if mode == "standard":
            text = info["char"]
            bbox = glyph_draw.textbbox((0, 0), text, font=font)
            tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
            tx, ty = (GLYPH_SIZE - tw) // 2, (GLYPH_SIZE - th) // 2
            glyph_draw.text((tx, ty), text, font=font, fill=(255, 255, 255, 255))

        glyph_data = np.array(glyph_img)
        glyph_data = apply_symmetry(glyph_data, info["symmetry"])

        # Build final glyph with semantic RGB
        final_glyph = np.zeros((GLYPH_SIZE, GLYPH_SIZE, 4), dtype=np.uint8)
        final_glyph[:, :, 0] = info["r"]
        final_glyph[:, :, 1] = info["g"]
        final_glyph[:, :, 2] = info["b"]
        final_glyph[:, :, 3] = glyph_data[:, :, 3]

        atlas.paste(Image.fromarray(final_glyph), (x, y))

    return atlas

def generate_atlas():
    """Main generation function."""
    ASSETS_DIR.mkdir(parents=True, exist_ok=True)

    register_glyphs()
    font = find_font()

    print(f"Generating atlas with {len(glyphs)} glyphs...")
    atlas = create_atlas(font, mode="standard")
    atlas.save(OUTPUT_ATLAS)
    print(f"Saved atlas: {OUTPUT_ATLAS}")

    metadata = {
        "glyphs": glyphs,
        "atlas_size": ATLAS_SIZE,
        "glyph_size": GLYPH_SIZE,
        "modes": {"standard": 0}
    }

    with open(OUTPUT_JSON, "w") as f:
        json.dump(metadata, f, indent=2)
    print(f"Saved metadata: {OUTPUT_JSON}")

    return True

if __name__ == "__main__":
    generate_atlas()
