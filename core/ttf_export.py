"""
Geometry OS TTF Font Generator

Converts Geometry OS bitmapped glyphs into a TrueType font.
Each pixel becomes a vector square, preserving the grid aesthetic.
"""

from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphCoordinates
from fontTools.fontBuilder import FontBuilder
import numpy as np
from PIL import Image, ImageDraw, ImageFont

GLYPH_SIZE = 16
UNITS_PER_EM = 2048
SCALE = UNITS_PER_EM // GLYPH_SIZE

def get_gos_pattern(char):
    """Generate a 16x16 bitmap pattern for a character."""
    grid = np.zeros((GLYPH_SIZE, GLYPH_SIZE), dtype=int)

    # GOS-style glyph definitions with terminal pins
    patterns = {
        'G': [(4, 4, 4, 11), (4, 4, 11, 4), (11, 4, 11, 11), (8, 11, 11, 11), (8, 8, 8, 11)],
        'O': [(4, 4, 4, 11), (4, 11, 11, 11), (11, 4, 11, 11), (4, 4, 11, 4)],
        'S': [(4, 4, 11, 4), (4, 4, 4, 7), (7, 7, 11, 7), (11, 7, 11, 11), (4, 11, 11, 11)],
        '7': [(4, 4, 11, 4), (4, 11, 11, 11)],
    }

    if char in patterns:
        for x1, y1, x2, y2 in patterns[char]:
            if x1 == x2:  # Vertical line
                for y in range(min(y1, y2), max(y1, y2) + 1):
                    if 0 <= y < GLYPH_SIZE:
                        grid[y, x1] = 1
            elif y1 == y2:  # Horizontal line
                for x in range(min(x1, x2), max(x1, x2) + 1):
                    if 0 <= x < GLYPH_SIZE:
                        grid[y1, x] = 1
    else:
        # Default: render from system font
        try:
            font = ImageFont.load_default()
            img = Image.new("L", (GLYPH_SIZE, GLYPH_SIZE), 0)
            draw = ImageDraw.Draw(img)
            bbox = draw.textbbox((0, 0), char, font=font)
            tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
            tx, ty = (GLYPH_SIZE - tw) // 2, (GLYPH_SIZE - th) // 2
            draw.text((tx, ty), char, font=font, fill=255)
            grid = (np.array(img) > 128).astype(int)
        except Exception:
            grid[4:12, 4:12] = 1
            grid[6:10, 6:10] = 0

    return grid

def grid_to_contours(grid):
    """Convert bitmap grid to font contours."""
    coords = []
    end_pts = []
    current_pt = 0

    for y in range(GLYPH_SIZE):
        for x in range(GLYPH_SIZE):
            if grid[y, x]:
                # Font coords: (0,0) is bottom-left, grid: (0,0) is top-left
                x0, y0 = x * SCALE, (GLYPH_SIZE - 1 - y) * SCALE
                x1, y1 = x0 + SCALE, y0 + SCALE

                # CW order: BL, TL, TR, BR
                coords.extend([(x0, y0), (x0, y1), (x1, y1), (x1, y0)])
                current_pt += 4
                end_pts.append(current_pt - 1)

    return coords, end_pts

def create_ttf(output_path, chars="GOS7"):
    """Generate a TTF font file."""
    family_name = "GeometryOS"
    style_name = "Regular"
    version = "1.0.0"

    chars = chars.replace(" ", "")
    glyph_order = [".notdef", "space"] + list(chars)

    fb = FontBuilder(unitsPerEm=UNITS_PER_EM, isTTF=True)
    fb.setupGlyphOrder(glyph_order)

    # Setup required tables
    fb.setupHead(unitsPerEm=UNITS_PER_EM)
    fb.setupHorizontalHeader(ascent=UNITS_PER_EM, descent=-UNITS_PER_EM // 4)
    fb.setupMaxp()

    # Name table
    nameStrings = {
        "familyName": family_name,
        "styleName": style_name,
        "uniqueFontIdentifier": f"{family_name}-{style_name}",
        "fullName": f"{family_name} {style_name}",
        "version": version,
        "psName": f"{family_name}-{style_name}",
    }
    fb.setupNameTable(nameStrings)
    fb.setupPost()

    # Character map
    cmap = {ord(c): c for c in chars}
    cmap[ord(" ")] = "space"
    fb.setupCharacterMap(cmap)

    # Build glyphs
    metrics = {".notdef": (UNITS_PER_EM, 0), "space": (UNITS_PER_EM // 4, 0)}
    glyphs_dict = {".notdef": Glyph(), "space": Glyph()}

    for char in chars:
        grid = get_gos_pattern(char)
        coords, end_pts = grid_to_contours(grid)

        g = Glyph()
        g.numberOfContours = len(end_pts)
        g.endPtsOfContours = end_pts
        g.flags = bytearray([1] * len(coords))  # On-curve points
        g.coordinates = GlyphCoordinates(coords)

        glyphs_dict[char] = g
        metrics[char] = (UNITS_PER_EM, 0)

    fb.setupGlyf(glyphs_dict)
    fb.setupHorizontalMetrics(metrics)

    # OS/2 table (must come after hmtx)
    fb.setupOS2()

    fb.save(output_path)
    print(f"Generated TTF: {output_path}")
    return output_path

if __name__ == "__main__":
    import sys
    output = sys.argv[1] if len(sys.argv) > 1 else "GeometryOS-Regular.ttf"
    create_ttf(output, "GOS7 abcdefghijklmnopqrstuvwxyz")
