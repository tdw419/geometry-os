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
