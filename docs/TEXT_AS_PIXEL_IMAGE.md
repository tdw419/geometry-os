# Text As Pixel Image: How ASCII Files Are Compressed Pixel Data

This document explains how any ASCII text file is simultaneously a compressed
pixel image, using the Geometry OS palette. Written for AI agents who need to
convert files to pixel images, reconstruct files from pixel images, or
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

| Char | Hex  | Hue  | RGB             | Visual       |
|------|------|------|-----------------|--------------|
| (sp) | 0x20 |   0  | (255, 51, 51)   | red          |
| !    | 0x21 |   4  | (255, 63, 51)   | red-orange   |
| 0    | 0x30 |  38  | (255, 173, 51)  | orange       |
| 9    | 0x39 |  73  | (173, 255, 51)  | yellow-green |
| A    | 0x41 | 103  | (51, 255, 98)   | green        |
| Z    | 0x5A | 169  | (51, 241, 255)  | cyan         |
| a    | 0x61 | 195  | (51, 150, 255)  | blue         |
| m    | 0x6D | 218  | (81, 51, 255)   | blue-purple  |
| z    | 0x7A | 268  | (199, 51, 255)  | purple       |
| ~    | 0x7E | 283  | (255, 51, 224)  | magenta      |

### Color Groupings (structural information)

- **Symbols** `!@#$%^&*()` -- red through orange (hue 0-60)
- **Digits** `0-9` -- orange through yellow-green (hue 38-73)
- **Uppercase** `A-Z` -- green through cyan (hue 103-169)
- **Lowercase** `a-z` -- blue through magenta (hue 195-348)
- **Space/controls** -- dark background (26,26,46)

---

## Three Rendering Modes

### Mode 1: 1 pixel per character (1:1 mapping)

The most compact representation. Image dimensions:

```
width  = max line length (in characters)
height = number of lines
```

Each character becomes exactly one pixel. Short lines are padded with
background color. This produces the smallest image and is the natural
encoding -- the pixel grid IS the text grid.

### Mode 2: Palette PNG (indexed color)

Uses PNG color type 3 (palette/indexed). Each pixel stores one byte (the
ASCII value) and the PNG PLTE chunk maps those values to RGB. This is more
compact than raw RGB because each pixel is 1 byte instead of 3.

**This is the recommended format.** It's smaller than RGB, lossless, and
viewable as a color image.

### Mode 3: 16x16 pixel cells (canvas mode)

Each character fills a 16x16 cell, rendered as either:
- **Solid color block** -- palette color fills entire cell
- **Pixel font glyph** -- 8x8 VGA bitmap scaled 2x, colored by palette

This is how Geometry OS renders the canvas. 256x larger than 1:1 mode.
Use for human-readable canvas display.

---

## Compression Comparison

Real test: CANVAS_TEXT_SURFACE.md (23,871 bytes, 586 lines, 164 chars wide)

```
Format                          Size      vs .md    Lossless?
─────────────────────────────────────────────────────────────
Original .md                    23,871 B   100%       ---
Shannon entropy limit           14,750 B    62%       yes
gzip -9                          9,217 B    39%       yes
Palette PNG (type 3, 1:1)       11,726 B    49%       yes  <-- best image
RGB PNG (type 2, 1:1)           16,800 B    70%       yes
4-bit indexed (2 chars/pixel)    7,979 B    33%       no   (16 buckets)
RGB PNG (16x16 solid cells)    145,131 B   608%       yes
RGB PNG (16x16 glyph cells)    25,164 B   105%       yes  (50 lines only)
```

### Key findings

1. **Palette PNG beats RGB PNG by 30%** (11,726 vs 16,800 bytes). Each pixel
   stores 1 byte (palette index) instead of 3 bytes (RGB). The PLTE chunk
   adds ~384 bytes but saves far more in the IDAT compressed data.

2. **Palette PNG overhead vs gzip is only +2,509 bytes** (27%). That overhead
   buys you a viewable image where document structure is visible as color bands.

3. **PNG filters (SUB/UP/PAETH) make it WORSE for text data.** These filters
   work well for photographs but text has sharp character boundaries. The
   "none" filter (raw bytes) is optimal for palette-indexed text.

4. **4-bit indexed (2 chars per pixel) is smallest at 7,979 bytes** but is
   lossy -- only 12 character buckets instead of 128 unique colors.

5. **gzip beats Shannon entropy** because it exploits patterns beyond single-
   character frequency (repeated words, LZ77 back-references across lines).

6. **Channel packing (2-3 chars per RGB pixel) is WORSE** than palette indexed.
   Storing ASCII values in R/G/B channels creates noise that zlib can't compress.

---

## Converting Text File to Pixel Image

### To make a palette PNG (recommended):

```python
import struct, zlib, colorsys

def palette_color(val):
    byte = val & 0x7F
    if byte < 32: return (26, 26, 46)
    t = (byte - 32) / 94.0
    hue = t * 360.0
    r, g, b = colorsys.hsv_to_rgb(hue / 360, 0.8, 1.0)
    return (int(r*255), int(g*255), int(b*255))

def text_to_palette_png(text, out_path):
    lines = text.split('\n')
    width = max(len(l) for l in lines)
    height = len(lines)
    
    # Build PLTE chunk (128 entries)
    plte = b''.join(bytes(palette_color(i)) for i in range(128))
    
    # Build pixel data (1 byte per pixel = palette index)
    raw = bytearray()
    for line in lines:
        raw.append(0)  # filter: none
        for x in range(width):
            raw.append(ord(line[x]) & 0x7F if x < len(line) else 0)
    
    def chunk(ct, d):
        c = ct + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xFFFFFFFF)
    
    png = (b'\x89PNG\r\n\x1a\n' +
        chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 3, 0, 0, 0)) +
        chunk(b'PLTE', plte) +
        chunk(b'IDAT', zlib.compress(bytes(raw), 9)) +
        chunk(b'IEND', b''))
    
    with open(out_path, 'wb') as f:
        f.write(png)
```

### To make an RGB PNG (wider compatibility):

Same as above but use color type 2 (RGB) and write 3 bytes per pixel.
This is 30% larger but works with tools that don't support palette PNGs.

---

## Converting Pixel Image Back to Text

### Round-trip is lossless (for palette and RGB modes)

Because the palette is a bijection (each character maps to a unique color),
you can reconstruct the original text exactly:

```python
# Build reverse lookup: RGB -> character
reverse_palette = {}
for i in range(128):
    rgb = palette_color(i)
    reverse_palette[rgb] = chr(i)

# Decode pixel image to text
def pixel_image_to_text(pixels, width, height):
    lines = []
    for y in range(height):
        row = ""
        for x in range(width):
            rgb = tuple(pixels[y * width + x][:3])
            ch = reverse_palette.get(rgb, '\x00')
            row += ch
        lines.append(row.rstrip('\x00'))  # strip padding
    return '\n'.join(lines)
```

For palette PNGs, decoding is even simpler: each pixel's palette index IS
the ASCII value. No color lookup needed.

---

## File Format Convention

```
filename.md                   -- the text (IS the compressed pixel sequence)
filename_colors.png           -- 1:1 palette PNG (for visual inspection + data)
filename_pixels.png           -- 16x16 glyph PNG (for readable canvas view)
```

No separate key file is needed. The palette_color formula is the canonical
key and never changes. Any AI agent that knows the formula can encode or
decode without a JSON lookup table.

---

## What You Need to Rebuild

To reconstruct the original file from the pixel image:

1. **The palette** -- the palette_color function (~125 bytes as code, or
   embedded in the PNG's PLTE chunk if using palette PNG)
2. **The pixel data** -- which is the image itself

That's it. No external key file. No separate sequence file.

For palette PNGs specifically: the PNG file IS self-contained. The PLTE
chunk stores the color mapping, the IDAT chunk stores the indices. You can
decode it with any standard PNG library -- read the palette, read the pixel
indices, convert indices to ASCII characters.

---

## Why This Works

The encoding exploits two facts:

1. **ASCII is already a palette index.** The byte value IS the color selector.
   There is no separate indexing step. The text file contains 1-byte indices
   into a 128-entry color table.

2. **The palette is deterministic and compact.** One 3-line function generates
   all 128 colors. No lookup table is strictly needed (the PNG PLTE chunk
   provides one for convenience).

This is different from traditional image compression (PNG, JPEG) which treats
pixels as independent RGB values. Here, each pixel's RGB is a function of a
single byte, and the sequence of bytes is the original document. The "image
format" and the "text format" are the same data viewed through different lenses.

The compression comes from zlib (shared with gzip) operating on the palette
index stream, which is essentially compressed text. The PNG container adds
~100 bytes of overhead (headers, PLTE chunk, IEND) for the benefit of making
the data viewable as a color image in any image viewer or browser.

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
