# encode_png Audit — Phase F (f.2.1)

Date: 2026-04-27
Status: COMPLETE

## The Bug

`encode_png()` in `src/vision.rs` produced invalid PNG files from its inception
until commit eb9e066 (2026-04-27). The IDAT chunk contained raw deflate data
without the required zlib wrapper (2-byte CMF/FLG header + Adler-32 checksum).

Strict decoders (Pillow/libpng) would reject these PNGs. Lenient decoders
(some browsers, some image viewers) might accept raw deflate stored blocks.

## Callers at Time of Fix

| Caller | Path | Used for | Affected? |
|--------|------|----------|-----------|
| `screenshot_b64` socket cmd | main.rs:3207 | Base64-encoded vm.screen PNG | YES — broken PNGs |
| `screenshot_annotated_b64` socket cmd | main.rs:3244 | Annotated vm.screen PNG (window borders) | YES — broken PNGs |
| VM screenshot opcode | vm/mod.rs:2675 | Screenshot to VFS file from pixel programs | YES — broken PNGs |
| `riscv_fb_dump` socket cmd | main.rs:3179 | RISC-V framebuffer dump | NO — added same day as fix |
| Unit tests | vision.rs:435-518 | Structural PNG validity tests | YES — but tests don't decode |
| `screenshot` socket cmd | main.rs:1823 | Full display buffer PNG | NO — uses `png` crate via `save_full_buffer_png` |
| MCP `vm_screenshot` tool | mcp_server.rs:721 | Delegates to `screenshot` socket cmd | NO — uses `png` crate |

## Impact Assessment

### Verified-Safe Paths (no broken PNGs)

1. **`screenshot` socket command** — uses `save_full_buffer_png()` in `save.rs`,
   which uses the `png` crate (proper zlib-compressed PNG). This is the command
   used by `mcp_geo_vm_screenshot` MCP tool. All prior MCP screenshot verifications
   are valid.

2. **`mcp_geo_vm_screenshot` MCP tool** — delegates to `screenshot` socket command.
   Safe.

3. **Phase B/C verification (life64, GUI bridge)** — done via MCP vm_screenshot or
   visual inspection of the live display. Not affected.

### Affected Paths (broken PNGs produced)

1. **`screenshot_b64`** — produced invalid PNGs. Used by Phase 88 (AI Vision Bridge)
   for cloud LLM screenshot analysis. The LLM would have received a base64 string
   that decoded to an invalid PNG. Whether the LLM could interpret it depends on
   the decoding pipeline — likely failed silently or produced garbage analysis.

2. **`screenshot_annotated_b64`** — same issue. Phase 88 annotated screenshots were
   invalid.

3. **VM screenshot opcode** — pixel programs that called the screenshot opcode would
   get invalid PNGs in the VFS. No known pixel program uses this opcode today.

4. **Vision unit tests** — `test_png_valid_signature`, `test_png_has_ihdr_idat_iend`,
   `test_png_size_reasonable`, `test_png_base64` all pass with broken PNGs because
   they only check structure (signature, chunk names, size), not decodability.
   **No test actually decoded a produced PNG.**

### Prior "Verified" Claims

Checking session history for claims using encode_png paths:

- **Phase B/C visual verifications**: Used MCP vm_screenshot (safe path) or live
  display observation. NOT affected.
- **Phase 88 AI Vision Bridge**: Used `screenshot_b64` (broken). LLM analysis
  of screenshots was unreliable — the PNGs were invalid. However, the feature
  was experimental and no critical decisions were based on LLM screenshot
  interpretation.
- **paint.c verification (this session)**: Used `riscv_fb_dump` with the FIXED
  encode_png. All pixel assertions are valid.

## Remediation

1. Fixed in commit eb9e066: added zlib wrapper (CMF/FLG header + Adler-32 checksum)
   around raw deflate output in `encode_png()`.

2. Added `adler32()` function to vision.rs.

3. All 21 vision tests pass with the fixed encoder.

4. New `riscv_fb_dump` command and the paint regression test verify actual pixel
   decodability — the test decodes the PNG with Pillow and asserts pixel values,
   which would fail on an invalid PNG.

## Recommendations

1. Add a decode round-trip test to vision.rs: encode a known image, decode with
   the `png` crate or raw zlib, assert pixel values match. This would have caught
   the bug immediately. (Filed as Phase F improvement.)

2. Any future verification using screenshots should use `riscv_fb_dump` (for RISC-V)
   or `screenshot` (for vm.screen) — both now produce valid PNGs and can be
   decoded by strict decoders.
