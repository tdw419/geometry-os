#!/usr/bin/env python3
"""
Geometry OS Font Preview Demo

Renders text with GOS styling:
- 16x16 grid alignment
- Circuit trace extensions
- Terminal ports (chip pins)
- Holographic interference (noise)
"""

import os
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

GLYPH_SIZE = 16

def find_font():
    """Find a monospace font."""
    paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
    ]
    for p in paths:
        if os.path.exists(p):
            try:
                return ImageFont.truetype(p, 10)
            except Exception:
                continue
    return ImageFont.load_default()

def stylize_gos_char(char, font):
    """
    Render a character with Geometry OS aesthetic:
    - Circuit traces extending from glyph edges
    - Terminal ports at horizontal centers
    - Holographic interference (10% noise)
    """
    img = Image.new("L", (GLYPH_SIZE, GLYPH_SIZE), 0)
    draw = ImageDraw.Draw(img)

    # Render centered text
    bbox = draw.textbbox((0, 0), char, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    tx, ty = (GLYPH_SIZE - tw) // 2, (GLYPH_SIZE - th) // 2
    draw.text((tx, ty), char, font=font, fill=255)

    pixels = np.array(img)

    # Add terminal ports (chip pins)
    pixels[GLYPH_SIZE // 2, 0] = 255
    pixels[GLYPH_SIZE // 2, GLYPH_SIZE - 1] = 255

    # Add circuit traces
    nonzero = np.nonzero(pixels > 0)
    if len(nonzero[0]) > 0:
        min_y, max_y = np.min(nonzero[0]), np.max(nonzero[0])
        min_x, max_x = np.min(nonzero[1]), np.max(nonzero[1])
        mid_x = (min_x + max_x) // 2

        # Vertical traces to edges
        pixels[0:min_y, mid_x] = 255
        pixels[max_y + 1:GLYPH_SIZE, mid_x] = 255

    # Holographic interference (10% random removal)
    mask = np.random.rand(GLYPH_SIZE, GLYPH_SIZE) > 0.1
    pixels = pixels * mask.astype(np.uint8)

    return Image.fromarray(pixels.astype(np.uint8))

def render_text(text, font, output_path="gos_preview.png"):
    """Render text string as GOS-styled atlas."""
    width = GLYPH_SIZE * len(text)
    atlas = Image.new("L", (width, GLYPH_SIZE), 0)

    for i, char in enumerate(text):
        glyph = stylize_gos_char(char, font)
        atlas.paste(glyph, (i * GLYPH_SIZE, 0))

    atlas.save(output_path)
    print(f"Generated preview: {output_path}")
    return output_path

if __name__ == "__main__":
    font = find_font()
    render_text("GOS7", font)
    render_text("Hello World!", font, "gos_preview_full.png")
