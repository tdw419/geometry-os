#!/bin/bash
# persistence_roundtrip_test.sh -- Phase G round-trip verification
#
# Tests that geos_save_canvas / geos_load_canvas preserve pixel data
# across VM restart. The test:
#   1. Boots paint.elf, draws a known pattern
#   2. Dumps framebuffer (baseline)
#   3. Saves canvas via 'p' key
#   4. Kills VM
#   5. Reboots paint.elf (VFS surface persists in memory across kills?
#      NO -- we need to test differently)
#
# IMPORTANT: The VFS surface is in-memory. When the VM process dies,
# the surface is lost. For true persistence, the VFS surface would need
# to be flushed to disk (via the VFS control register). However, the
# canvas save writes directly to the VFS pixel array, not through the
# file system. The VfsSurface Drop impl calls flush() which only writes
# files with valid directory entries -- our canvas data in rows 1-255
# won't be flushed as a file because there's no directory entry for it.
#
# So the round-trip test uses a single VM session:
#   1. Boot paint.elf, draw pattern
#   2. Dump framebuffer (baseline)
#   3. Inject 'p' to save canvas to VFS surface
#   4. Inject 'c' to clear canvas
#   5. Inject 'o' to load canvas from VFS surface
#   6. Dump framebuffer (restored)
#   7. Compare baseline vs restored in canvas area (y < 244)
#
# This verifies the save/load logic is correct. Cross-session persistence
# would require either:
#   a) Saving the canvas as a VFS file entry (needs split across files
#      due to 64KB limit), or
#   b) Adding a "raw region" flush mechanism to VfsSurface
# Both are future enhancements beyond Phase G scope.

set -euo pipefail
cd "$(dirname "$0")/.."

source tests/geos_test_lib.sh

ELF_PATH="examples/riscv-hello/paint.elf"
BASELINE_PNG="/tmp/geos_roundtrip_baseline.png"
RESTORED_PNG="/tmp/geos_roundtrip_restored.png"

echo "=== Persistence Round-Trip Test ==="

# ---- Phase 1: Draw, save baseline, save to VFS ----
echo "--- Phase 1: Boot and draw ---"
geos_boot

geos_run_elf "$ELF_PATH"

# Draw a recognizable pattern:
# Move right 30px from center (128,128), paint red
# Move down 20px, paint green
# Move left 10px, paint blue
# Each movement: 'd' = right, 's' = down, 'a' = left, 'w' = up

# Select red (1)
geos_inject_text "1"

# Paint at center
geos_inject " "  # space = paint
sleep 0.3

# Move right 30 pixels, fill mode on to leave a trail
geos_inject_text "f"  # fill mode ON
sleep 0.1
for i in $(seq 1 30); do
    geos_inject_text "d"
    sleep 0.05
done

# Select green (2) and move down 20 pixels
geos_inject_text "2"
for i in $(seq 1 20); do
    geos_inject_text "s"
    sleep 0.05
done

# Select blue (3) and move left 15 pixels
geos_inject_text "3"
for i in $(seq 1 15); do
    geos_inject_text "a"
    sleep 0.05
done

# Turn off fill mode
geos_inject_text "f"
sleep 0.1

# Paint a single white pixel at current position
geos_inject_text "7"
geos_inject " "
sleep 0.3

echo "Pattern drawn. Dumping baseline..."
geos_fb_dump "$BASELINE_PNG"
sleep 0.5

# Save canvas
echo "Saving canvas..."
geos_inject_text "p"
sleep 1

# Clear canvas
echo "Clearing canvas..."
geos_inject_text "c"
sleep 0.5

# ---- Phase 2: Load and verify ----
echo "--- Phase 2: Load and verify ---"
geos_inject_text "o"
sleep 1

echo "Dumping restored..."
geos_fb_dump "$RESTORED_PNG"
sleep 0.5

# Quit
geos_inject_text "$(printf '\x1b')"
sleep 0.5

geos_kill

# ---- Phase 3: Compare ----
echo "--- Phase 3: Compare pixels ---"

python3 -c "
from PIL import Image
import sys

baseline = Image.open('$BASELINE_PNG')
restored = Image.open('$RESTORED_PNG')

if baseline.size != restored.size:
    print(f'FAIL: Size mismatch: {baseline.size} vs {restored.size}')
    sys.exit(1)

bp = baseline.load()
rp = restored.load()

mismatches = 0
checked = 0
# Compare canvas area only (y < 244, excluding palette bar)
# Also skip y >= 244 since palette bar is redrawn by paint
for y in range(0, 244):
    for x in range(0, baseline.width):
        br, bg, bb = bp[x, y]
        rr, rg, rb = rp[x, y]
        checked += 1
        if br != rr or bg != rg or bb != rb:
            mismatches += 1
            if mismatches <= 5:
                print(f'  MISMATCH at ({x},{y}): baseline=({br},{bg},{bb}) restored=({rr},{rg},{rb})')

if mismatches > 0:
    print(f'FAIL: {mismatches}/{checked} canvas pixels mismatched')
    sys.exit(1)
else:
    print(f'  OK: All {checked} canvas pixels match exactly (tolerance=0)')
"

echo ""
echo "=== Persistence Round-Trip Test PASSED ==="
