# Code-Pixel Execution

Geometry OS programs can be encoded as viewable PNG images. Each pixel's 32-bit RGBA value is a "seed" that expands to 1-16 bytes of data via the pixelpack encoding scheme. The PNG file IS the executable.

## Three Levels

### Level 1: Bytecode from PNG (Phase 92)

A `.png` file contains pixelpack-encoded **bytecode**. Load the image, decode seeds to bytes, write directly to `RAM[0x1000]`, run.

- **No assembler step** -- the PNG contains pre-assembled bytecode
- **No canvas text** -- source code is not visible on the grid
- **The image IS the binary executable**

```bash
# CLI: boot a pixelpack-encoded program
cargo run -- --cli --boot-png programs/my_program.png

# CLI REPL: load and run a PNG
geo> boot-png programs/my_program.png
```

Opcode: `LOADPNG path_reg, dest_addr_reg` (0xB1) -- runtime bytecode loading from PNG.

### Level 2: Source from PNG (Phase 93)

A `.png` file contains pixelpack-encoded **assembly source code**. Load the image, decode seeds to UTF-8 text, write onto the canvas buffer (128x32 grid), auto-assemble to bytecode at `0x1000`, run.

- Source code appears as colored syntax-highlighted text on the grid
- The image is both the source code AND the executable
- Double-click a `.png` and you see the code AND the output
- PNG metadata `geo_boot=source` distinguishes from bytecode PNGs

```bash
# CLI: boot a source-encoded PNG
cargo run -- --cli --boot-src-png programs/my_program.png

# Auto-detect: source PNGs are detected by metadata
cargo run -- --cli programs/my_source_program.png
```

Opcode: `LOADSRCIMG path_reg` (0xB2) -- runtime source loading from PNG, canvas write + assemble.

### Level 3: Universal Pixel Executable (Phase 94)

The unified boot path. One loader detects the PNG type automatically:
- `geo_boot=source` metadata → Level 2 (source decode → canvas → assemble → run)
- `geo_boot=bytecode` or no metadata → Level 1 (bytecode decode → RAM → run)
- File extension: `.png` triggers pixel boot in CLI mode

Programs can bootstrap other programs from pixels at runtime:
```asm
; Program A loads program B from a PNG and runs it
LDI r5, path_str     ; path to program B's PNG
LOADSRCIMG r5        ; decode source PNG → canvas → assemble → bytecode at 0x1000
; r0 = bytecode word count on success
RUNNEXT              ; jump PC to 0x1000 to execute program B
```

## Pixelpack Encoding

Each pixel (32-bit RGBA) encodes data via a 4-bit strategy + 28-bit params:

```
[31:28] strategy  |  [27:0] params
     0x0-0x6      |  Dictionary lookup (1-7 entries from 16-word table)
     0x7          |  Nibble encoding (7 hex digits)
     0x8          |  4 raw bytes
     0x9          |  RLE (repeat byte N times)
     0xA          |  3 raw bytes
     0xB          |  XOR chain
     0xC          |  Linear sequence (start + step * N)
     0xD          |  Delta encoding
     0xE          |  Bytepack
     0xF          |  4-byte literal
```

### Strategy Details

| Strategy | Name | Params Layout | Output |
|----------|------|---------------|--------|
| 0x0-0x6 | Dict | N x 4-bit indices | Dictionary words (LDI, HALT, etc.) |
| 0x7 | Nibble | 7 x 4-bit | ASCII hex digits |
| 0x8 | Raw4 | byte3, byte2, byte1, byte0 | 4 raw bytes |
| 0x9 | RLE | count:8, byte:8 | Repeated byte |
| 0xA | Raw3 | byte2:8, byte1:8, byte0:8 | 3 raw bytes |
| 0xB | XOR | count:4, key:8, start:8 | XOR-chain bytes |
| 0xC | Linear | count:4, step:8, start:8 | Arithmetic sequence |
| 0xD | Delta | count:4, base:8, deltas | Delta-encoded bytes |
| 0xE | Bytepack | Packed nibble pairs | Compact encoding |
| 0xF | Literal | 4 raw bytes from 28-bit params | 4 bytes |

## Opcodes

### LOADPNG (0xB1) -- Bytecode from PNG

```asm
LDI r5, path_str
LDI r6, 0x1000        ; destination address
LOADPNG r5, r6         ; decode PNG, write bytecode to RAM[0x1000]
; r0 = byte count on success, 0xFFFFFFFF on error
```

**Encoding:** 3 words: `[0xB1, path_reg, dest_addr_reg]`

### LOADSRCIMG (0xB2) -- Source from PNG

```asm
LDI r5, path_str
LOADSRCIMG r5          ; decode source PNG, write to canvas, assemble at 0x1000
; r0 = bytecode word count on success, 0xFFFFFFFF on error
```

**Encoding:** 2 words: `[0xB2, path_reg]`

The source text is decoded from the PNG, written to the canvas buffer (visible as syntax-highlighted text), then assembled to bytecode at `0x1000`. The program can then be run with `RUNNEXT` (0x74) or by setting `PC = 0x1000`.

## CLI Integration

### Boot Flags

```bash
# Boot bytecode from PNG
cargo run -- --cli --boot-png program.png

# Boot source from PNG
cargo run -- --cli --boot-src-png program.png

# Auto-detect (checks PNG metadata)
cargo run -- --cli program.png
```

### REPL Commands

```
geo> boot-png program.png         ; load bytecode PNG
geo> boot-src-png program.png     ; load source PNG
```

## PNG Metadata

| Key | Value | Meaning |
|-----|-------|---------|
| `seedcnt` | integer | Number of seeds (pixels with data) |
| `bytecnt` | integer | Original byte count before encoding |
| `geo_boot` | `source` or absent | `source` = Level 2, absent = Level 1 |

The `geo_boot` metadata key is the discriminator. Source PNGs set `geo_boot=source`. Bytecode PNGs either set nothing or `geo_boot=bytecode`.

## Memory Map

```
0x1000-0x1FFF  Assembled/loaded bytecode (also used for pixelpack PNG boot)
0x8000-0x8FFF  Canvas buffer (128 rows x 32 cols, source PNG text written here)
```

The pixelpack boot path writes to the same bytecode region used by the assembler. A pixelpack-booted program can still use the assembler to compile new code on-the-fly.

## Round-Trip Pipeline

### Bytecode Path (Level 1)

```
source.asm
    ↓ assembler::assemble()
bytecode (Vec<u32>)
    ↓ byte extraction (LE)
raw bytes (Vec<u8>)
    ↓ encode_pixelpack_png()
PNG image (viewable!)
    ↓ decode_pixelpack_png()
raw bytes (Vec<u8>)
    ↓ load_bytecode_to_ram()
RAM[0x1000..]
    ↓ vm.step() loop
Execution
```

### Source Path (Level 2)

```
source.asm (text)
    ↓ encode_source_pixelpack_png()
PNG image (viewable!)
    ↓ decode_pixelpack_source()
source text (String)
    ↓ load_source_to_canvas_buffer()
canvas_buffer (128x32 grid, visible as syntax-highlighted text)
    ↓ assembler::assemble()
bytecode at RAM[0x1000]
    ↓ vm.step() loop
Execution
```

## Tests

### Phase 92 (Bytecode from PNG)
- `pixel::tests::test_full_pixel_boot_roundtrip` -- assemble, encode, decode, load, run, verify registers
- `pixel::tests::test_pixelpack_roundtrip_encode_decode` -- encode/decode round-trip
- `vm::tests::test_loadpng_opcode_basic` -- LOADPNG opcode loads and runs program from PNG
- `vm::tests::test_loadpng_opcode_missing_file` -- error handling for missing files
- `vm::tests::test_loadpng_opcode_empty_path` -- error handling for empty paths

### Phase 93 (Source from PNG)
- `vm::tests::test_loadsrcimg_opcode_basic` -- LOADSRCIMG loads source PNG, assembles, runs
- `vm::tests::test_loadsrcimg_opcode_missing_file` -- error handling
- `vm::tests::test_loadsrcimg_opcode_empty_path` -- error handling
- `vm::tests::test_source_png_round_trip` -- encode/decode source round-trip
- `vm::tests::test_source_png_assembles_correctly` -- source PNG assembles to correct bytecode
- `vm::tests::test_source_png_distinguishes_from_bytecode` -- metadata detection
- `vm::tests::test_loadsrcimg_canvas_buffer_populated` -- canvas buffer gets source text
- `vm::tests::test_loadsrcimg_invalid_register` -- register bounds check

### Phase 94 (Universal Pixel Executable)
- `vm::tests::test_universal_pixel_boot_bytecode` -- auto-detect bytecode PNG
- `vm::tests::test_universal_pixel_boot_source` -- auto-detect source PNG
- `vm::tests::test_pixel_chain_program_a_loads_b` -- program A loads program B from PNG
