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

### Level 2: Source from PNG (Phase 93, planned)

A `.png` file contains pixelpack-encoded **assembly source code**. Load the image, decode to text, write onto the canvas grid at `0x8000+`, auto-trigger F8 assembly (preprocess + assemble), run from `0x1000`.

- Source code appears as colored syntax-highlighted text on the grid
- The image is both the source code AND the executable
- Double-click a `.png` and you see the code AND the output

### Level 3: Universal Pixel Executable (Phase 94, planned)

Combines Level 1 and Level 2. A single `.png` contains both bytecode AND source. The VM detects the encoding and loads the appropriate representation.

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

## LOADPNG Opcode (0xB1)

Runtime opcode for loading pixelpack PNGs from within a running program:

```asm
; Store file path in RAM
LDI r5, path_str
LDI r6, 0x1000        ; destination address
LOADPNG r5, r6         ; decode PNG, write bytecode to RAM[0x1000]
; r0 = byte count on success, 0xFFFFFFFF on error
```

**Encoding:** 3 words: `[0xB1, path_reg, dest_addr_reg]`

The path string is read from RAM at the address in `path_reg` (null-terminated). The decoded bytecode is written to RAM starting at the address in `dest_addr_reg`.

## CLI Integration

### --boot-png flag

```bash
cargo run -- --cli --boot-png program.png
```

Decodes the pixelpack PNG, loads bytecode to `RAM[0x1000]`, auto-runs the program.

### boot-png REPL command

```
geo> boot-png program.png
[pixel-boot] Loaded 42 bytes (14 RAM words) from program.png
[pixel-boot] Execution done. PC=0x100E Halted=true
```

## Memory Map Addition

```
0x1000-0x1FFF  Assembled/loaded bytecode (also used for pixelpack PNG boot)
```

The pixelpack boot path writes directly to the same bytecode region used by the assembler. A pixelpack-booted program can still use the assembler to compile new code on-the-fly.

## File Detection

PNG files are auto-detected by the `--boot-png` flag and by the CLI REPL's `boot-png` command. The `is_pixelpack_png()` function checks for `.png`/`.PNG` extension (excluding `.rts.png` files which use a different encoding).

## Round-Trip Pipeline

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

## Tests

- `pixel::tests::test_full_pixel_boot_roundtrip` -- assemble, encode, decode, load, run, verify registers
- `pixel::tests::test_pixelpack_roundtrip_encode_decode` -- encode/decode round-trip
- `pixel::tests::test_pixelpack_roundtrip_large` -- 100 bytes round-trip
- `vm::tests::test_loadpng_opcode_basic` -- LOADPNG opcode loads and runs program from PNG
- `vm::tests::test_loadpng_opcode_missing_file` -- error handling for missing files
- `vm::tests::test_loadpng_opcode_empty_path` -- error handling for empty paths
