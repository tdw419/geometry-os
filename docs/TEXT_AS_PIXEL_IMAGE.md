# Text As Pixel Image: How ASCII Files Are Compressed Pixel Data

This document explains how any ASCII text file is simultaneously a compressed
pixel image, using the Geometry OS palette. This is written for AI agents who
need to convert files to pixel images, reconstruct files from pixel images, or
understand the dual-nature encoding.

---

## The Core Idea

Every ASCII character (0x00-0x7F) maps to exactly one RGB color via the
palette_color function. This means:

- A text file is a sequence of palette indices (one byte per pixel)
- The palette_color function is the decoding key
- Converting text to a pixel image is trivial: replace each byte with its color
- Converting a pixel image back to text is trivial: look up each color in the
  palette to get the original byte

**The text file IS the compressed pixel data.** No separate encoding step needed.

---

## The Palette (Key)

The palette_color function maps ASCII byte values to RGB colors:

```
palette_color(val):
  byte = val & 0x7F
  if byte < 0x20 (32):        return GRID_BG = (26, 26, 46)
  t = (byte - 32) / 94.0      // normalize printable ASCII to 0.0..1.0
  hue = t * 360.0              // spread across full color wheel
  return HSV_to_RGB(hue, 0.8 saturation, 1.0 value)
```

This maps the 95 printable ASCII characters (0x20-0x7E) across the full
360-degree color wheel at 80% saturation and 100% brightness. Each character
gets a unique color. Control characters (0x00-0x1F) all map to dark background.

### Character-to-Color Examples

| Char | Hex  | t     | Hue  | RGB             | Visual       |
|------|------|-------|------|-----------------|--------------|
| (sp) | 0x20 | 0.000 |   0  | (255, 51, 51)   | red          |
| !    | 0x21 | 0.011 |   4  | (255, 63, 51)   | red-orange   |
| 0    | 0x30 | 0.106 |  38  | (255, 173, 51)  | orange       |
| 9    | 0x39 | 0.202 |  73  | (173, 255, 51)  | yellow-green |
| A    | 0x41 | 0.287 | 103  | (51, 255, 98)   | green        |
| Z    | 0x5A | 0.468 | 169  | (51, 241, 255)  | cyan         |
| a    | 0x61 | 0.543 | 195  | (51, 150, 255)  | blue         |
| m    | 0x6D | 0.606 | 218  | (81, 51, 255)   | blue-purple  |
| z    | 0x7A | 0.745 | 268  | (199, 51, 255)  | purple       |
| ~    | 0x7E | 0.787 | 283  | (255, 51, 224)  | magenta      |

### Color Groupings (structural information)

- **Symbols** `!@#$%^&*()` etc. -- red through orange (hue 0-60)
- **Digits** `0-9` -- orange through yellow-green (hue 38-73)
- **Uppercase** `A-Z` -- green through cyan (hue 103-169)
- **Lowercase** `a-z` -- blue through magenta (hue 195-348)
- **Space/controls** -- dark background (26,26,46)

This means you can see document structure at a glance: code lines have lots of
green (opcodes) and blue-purple (variables), markdown has green (headings with
uppercase) and blue-purple (body text), data files have yellow-green (numbers).

---

## Converting Text File to Pixel Image

### Method 1: 1 pixel per character (1:1 mapping)

The most compact representation. The image dimensions are:

```
width  = max line length (in characters)
height = number of lines
```

Each character becomes exactly one pixel. Newlines separate rows. Short lines
are padded with the background color (26,26,46).

For a file with lines ["ABC", "DEF"]:
```
Pixel (0,0) = palette_color('A') = green
Pixel (1,0) = palette_color('B') = green
Pixel (2,0) = palette_color('C') = green
Pixel (0,1) = palette_color('D') = green
Pixel (1,1) = palette_color('E') = green
Pixel (2,1) = palette_color('F') = green
```

The resulting PNG is typically SMALLER than the text file because:
- PNG compresses runs of identical colors well
- Each pixel is one byte of index data, but RGB expansion is handled by PNG
- Real-world text has repeated characters = long color runs

### Method 2: 16x16 pixel cells (canvas mode)

Each character fills a 16x16 cell, either as:
- **Solid color block** -- the palette color fills the entire cell
- **Pixel font glyph** -- the 8x8 VGA bitmap from font.rs scaled 2x, with
  palette color as foreground and dark background

This is how Geometry OS renders the canvas. The image is 256x larger than
1:1 mode (each char = 256 pixels instead of 1). Use this when the image needs
to be human-readable as text.

---

## Converting Pixel Image Back to Text

### Round-trip is lossless

Because the palette is a bijection (each character maps to a unique color), you
can reconstruct the original text exactly:

1. Read each pixel's RGB value
2. Find the character whose palette_color matches that RGB
3. Write that character to the output
4. Newlines are implicit between rows

### Implementation

```python
# Build reverse lookup: RGB -> character
reverse_palette = {}
for i in range(128):
    r, g, b = palette_color(i)
    reverse_palette[(r, g, b)] = chr(i)

# Decode pixel image to text
def pixel_image_to_text(pixels, width, height):
    lines = []
    for y in range(height):
        row = ""
        for x in range(width):
            r, g, b = pixels[y * width + x]
            ch = reverse_palette.get((r, g, b), '?')
            row += ch
        lines.append(row.rstrip('\x00'))  # strip trailing nulls/bg padding
    return '\n'.join(lines)
```

---

## Size Comparison

Using CANVAS_TEXT_SURFACE.md as a real example (585 lines, 164 chars widest):

| Format                    | Size     | Notes                           |
|---------------------------|----------|---------------------------------|
| Original text (.md)       | 23,871 B | The source + the compressed form|
| 1:1 color PNG (164x586)   | 16,800 B | Smaller than source!            |
| 16x16 color PNG (1024x9376)| 145,131 B | Canvas-scale, every cell solid |
| 16x16 glyph PNG (first 50 lines) | 25,164 B | With font rendering          |
| Palette key (JSON)        | 2,746 B  | Needed for decode               |
| Palette key (formula)     | ~50 B    | The 3-line function             |

The 1:1 color PNG is smaller than the original text because PNG's DEFLATE
compression exploits the limited color palette (only ~95 unique colors for
printable ASCII, plus background).

**Key + text file = everything needed to produce any pixel representation.**

---

## File Format Convention

When saving a text-as-pixel-image alongside its source:

```
filename.md             -- the text (IS the compressed pixel sequence)
filename_colors.png     -- 1:1 pixel image (for visual inspection)
filename_pixels.png     -- 16x16 glyph image (for readable canvas view)
filename_key.json       -- palette key (optional, formula is sufficient)
```

The `_key.json` file is for convenience only. The palette_color formula is
the canonical key and never changes. Any AI agent that knows the formula can
encode or decode without the JSON file.

---

## Practical Usage for AI Agents

### To encode any text file as a pixel image:

1. Read the file as bytes
2. Split by newlines to get rows
3. For each character, compute palette_color(byte & 0x7F)
4. Write pixels to PNG (1 pixel per char for compact, 16x16 for canvas)

### To decode a pixel image back to text:

1. Read the PNG pixel data
2. For each pixel, find the matching character via reverse palette lookup
3. Join rows with newlines

### To verify encoding:

1. Encode the text file to a pixel image
2. Decode the pixel image back to text
3. The result must match the original exactly (lossless round-trip)

### When to use which format:

- **1:1 color PNG** -- visual overview, structural analysis, smallest file
- **16x16 solid PNG** -- zoomed-in color view, see individual chars as blocks
- **16x16 glyph PNG** -- readable text on the pixel grid (Geometry OS canvas)
- **Raw text file** -- the compressed form itself, most efficient to store/transfer

---

## Why This Works

The encoding exploits two facts:

1. **ASCII is already a palette index.** The byte value IS the color selector.
   There is no separate indexing step. The text file contains 1-byte indices
   into a 128-entry color table.

2. **The palette is deterministic and compact.** One 3-line function generates
   all 128 colors. No lookup table is strictly needed (the JSON key is
   convenience, not necessity).

This is different from traditional image compression (PNG, JPEG) which treats
pixels as independent RGB values. Here, each pixel's RGB is a function of a
single byte, and the sequence of bytes is the original document. The "image
format" and the "text format" are the same data viewed through different lenses.

---

## Relationship to Geometry OS

In Geometry OS, this encoding is the foundation of the canvas text surface:

```
keystroke -> ASCII byte -> stored in canvas_buffer[cell]
                                |
                      +---------+---------+
                      |                   |
                 rendering            assembly (F8)
                      |                   |
               palette_color()     read as text string
               + font glyph             |
                      |            assembler::assemble()
               colored pixels           |
                      |            bytecode at 0x1000
               the letter IS
               the pixel color
```

The same byte value determines both the color AND the character shape. This
document describes the color-only path. See CANVAS_TEXT_SURFACE.md for the
full rendering pipeline including pixel font glyphs.
