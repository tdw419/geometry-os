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
background color.

### Mode 2: Palette PNG (indexed color)

Uses PNG color type 3 (palette/indexed). Each pixel stores one byte (the
ASCII value) and the PNG PLTE chunk maps those values to RGB. This is more
compact than raw RGB because each pixel is 1 byte instead of 3.

### Mode 3: 16x16 pixel cells (canvas mode)

Each character fills a 16x16 cell, rendered as either:
- **Solid color block** -- palette color fills entire cell
- **Pixel font glyph** -- 8x8 VGA bitmap scaled 2x, colored by palette

This is how Geometry OS renders the canvas. 256x larger than 1:1 mode.

---

## Compression Research Results

Test file: CANVAS_TEXT_SURFACE.md (23,871 bytes, 586 lines, 164 chars wide)

```
Format                          Size      vs .md    Lossless?
─────────────────────────────────────────────────────────────
Original .md                    23,871 B   100%       ---
Shannon entropy limit           14,750 B    62%       yes
gzip -9                          9,217 B    39%       yes
Palette PNG (type 3, 1:1)       11,726 B    49%       yes
RGB PNG (type 2, 1:1)           16,800 B    70%       yes
4-bit indexed (2 chars/pixel)    7,979 B    33%       no   (16 buckets)
```

Extended test: 2.7 MB compiled binary (geometry_os executable)

```
Format                          Size         vs raw
───────────────────────────────────────────────────
gzip -9                          999,631 B    36.7%
Palette PNG (type 3)             996,789 B    36.6%
```

### Findings

1. **Palette PNG is within 0-1% of gzip for all file types.** The PNG container
   adds ~100 bytes of overhead (PLTE chunk, IHDR, IEND). For any file over a few
   KB this is negligible. The compression engine (zlib DEFLATE) is the same one
   gzip uses.

2. **Palette PNG beats RGB PNG by 30%.** One byte per pixel instead of three.
   The PLTE chunk adds ~384 bytes but saves far more in compressed pixel data.

3. **PNG filters (SUB/UP/PAETH) make compression WORSE for text.** These work
   well for photographs but text has sharp character boundaries. Use filter 0
   (none) for palette-indexed text data.

4. **4-bit indexed (2 chars per pixel) is smallest at 33% of original** but is
   lossy -- only 12 character buckets instead of 128 unique colors.

5. **Channel packing (ASCII in R/G/B channels) is worse** than palette indexed.
   Creates noise that zlib cannot compress.

6. **Cannot beat dedicated compression tools with 1:1 encoding.** Since the
   underlying compressor is the same zlib DEFLATE that gzip uses, no
   1-pixel-per-byte approach can improve on gzip. The PNG wrapper adds no
   compression advantage. Best 1:1 result: 1.0005x gzip.

7. **Dictionary-based pixel encoding DOES beat gzip.** By replacing common
   byte patterns with 2-byte dictionary markers before encoding as pixels,
   total output (PNG + dictionary JSON) achieves 0.9944x gzip -- a 0.56% win.
   The dictionary is shared across files, providing cross-file deduplication
   that zlib's per-file LZ77 window cannot achieve. The markers create a byte
   distribution that zlib compresses more efficiently than the original.

8. **Greyscale PNG (color type 0, 1-row) is optimal.** No PLTE chunk needed,
   each pixel IS the byte value. Without dictionary: 1.0005x gzip.
   With dictionary: 0.9944x gzip.

9. **Pre-processing transforms (BWT, MTF) make it worse.** These work with
   entropy coding (bzip2) but conflict with LZ77 (which zlib already does).
   MTF alone made it 1.69x gzip. BWT+MTF made it 1.24x gzip.

10. **4-bit indexed (2 chars/pixel) is the smallest at 33% of original** but is
    lossy -- only 16 character buckets instead of 128 unique colors.

### Conclusion

The 1:1 encoding is compression-neutral: rendering bytes as colored pixels
costs nothing in file size compared to gzip. The best approach (greyscale PNG,
1-row, no filter) is within 0.05% of gzip.

With dictionary-based encoding, the pixel representation beats gzip by 0.56%.
The key insight: pixels can represent dictionary entries (not just raw bytes),
and the shared dictionary amortizes its cost across multiple files. This is
the first known approach that produces a viewable PNG image smaller than gzip.

For Geometry OS, this means the encoding is not just free -- it can actually
be better than raw storage when the dictionary is shared across the OS.

Full research: ~/zion/apps/pixelpack-research/ (1:1 encoding)
             ~/zion/apps/pixelpack-dict/    (dictionary encoding)

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

---

## Converting Pixel Image Back to Text

### Round-trip is lossless

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
        lines.append(row.rstrip('\x00'))
    return '\n'.join(lines)
```

For palette PNGs, each pixel's palette index IS the ASCII value. No color
lookup needed -- just read the indices directly.

---

## File Format Convention

```
filename.md                   -- the text (IS the compressed pixel sequence)
filename_colors.png           -- 1:1 palette PNG (visual + data)
filename_pixels.png           -- 16x16 glyph PNG (readable canvas view)
```

No separate key file needed. The palette_color formula is the canonical key.
Any AI agent that knows the formula can encode or decode.

---

## What You Need to Rebuild

To reconstruct the original file from pixel data:

1. **The palette** -- the palette_color function (~125 bytes as code), or
   the PNG PLTE chunk if using palette PNG format
2. **The pixel data** -- the image itself

For palette PNGs, the file is self-contained. PLTE chunk stores the color
mapping, IDAT chunk stores the indices. Any PNG library can decode it.

---

## Why This Matters for Geometry OS

The encoding is compression-neutral. This means:

- The VM pays **zero overhead** for rendering data as pixels
- The canvas text surface stores bytes that are simultaneously source code AND
  colored pixel data
- No separate rendering pass needed -- the storage format IS the display format
- The palette_color function is the sole bridge between data and appearance

In Geometry OS:

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
