#!/bin/bash
# paint_regression_test.sh — Verify paint.c pixel output matches expectations.
#
# Draws a rectangle using fill mode (red right, blue down, green left, yellow up),
# then verifies pixel colors at specific coordinates and counts per color.
# Catches: channel bit-leaking, cursor overwrite, fill-mode position bugs.
#
# Source: tests/geos_test_lib.sh

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/geos_test_lib.sh"

DUMP="/tmp/paint_regression_$(date +%s).png"
ELF="examples/riscv-hello/paint.elf"

echo "=== Paint Regression Test ==="
echo "1. Booting GeOS..."
geos_boot

echo "2. Launching paint.elf..."
geos_run_elf "$ELF"

echo "3. Drawing rectangle via fill mode..."
# Fill mode ON
geos_inject_text "f"
sleep 0.5
# Move right 11 pixels (draws RED line at y=128)
geos_inject_text "ddddddddddd"
sleep 0.5
# Switch to BLUE (color 3)
geos_inject_text "3"
sleep 0.5
# Move down 10 pixels (draws BLUE line at x=139)
geos_inject_text "ssssssssss"
sleep 0.5
# Switch to GREEN (color 2)
geos_inject_text "2"
sleep 0.5
# Move left 10 pixels (draws GREEN line at y=138)
geos_inject_text "aaaaaaaaaa"
sleep 0.5
# Switch to YELLOW (color 4)
geos_inject_text "4"
sleep 0.5
# Move up 10 pixels (draws YELLOW line at x=129)
geos_inject_text "wwwwwwwwww"
sleep 0.5
# Fill mode OFF
geos_inject_text "f"
sleep 0.5

echo "4. Dumping framebuffer..."
geos_fb_dump "$DUMP"
sleep 0.5

echo "5. Asserting pixel values..."

# Background pixel (should be undisturbed)
assert_pixel "$DUMP" 50 50 10 10 20 3

# Red line: (128,128) through (137,128) -- cursor started at 128,128
# The cursor (yellow) sits at final position (129,128), so skip that
assert_pixel "$DUMP" 128 128 255 60 60 10
assert_pixel "$DUMP" 132 128 255 60 60 10
assert_pixel "$DUMP" 137 128 255 60 60 10

# Blue line: (139,128) through (139,137)
assert_pixel "$DUMP" 139 130 60 100 255 10
assert_pixel "$DUMP" 139 135 60 100 255 10

# Green line: (130,138) through (139,138)
assert_pixel "$DUMP" 134 138 60 200 60 10
assert_pixel "$DUMP" 139 138 60 200 60 10

# Yellow line: (129,128) through (129,137)
# Note: (129,128) has cursor (yellow), (129,138) has green/yellow overlap
assert_pixel "$DUMP" 129 132 255 255 60 10
assert_pixel "$DUMP" 129 136 255 255 60 10

# Adjacent pixels should be background (no spill)
assert_pixel "$DUMP" 140 128 10 10 20 3  # right of red line
assert_pixel "$DUMP" 139 139 10 10 20 3  # below blue line
assert_pixel "$DUMP" 128 138 10 10 20 3  # left of green line

# Palette bar colors (at y=248)
assert_pixel "$DUMP" 12 248 0 0 0 5      # color 0: black
assert_pixel "$DUMP" 37 248 255 60 60 10  # color 1: red
assert_pixel "$DUMP" 62 248 60 200 60 10  # color 2: green
assert_pixel "$DUMP" 87 248 60 100 255 10 # color 3: blue
assert_pixel "$DUMP" 112 248 255 255 60 10 # color 4: yellow

echo "6. Asserting pixel counts..."
# Red: ~10 pixels (128-137 at y=128)
assert_pixel_count "$DUMP" 200 0 0 255 100 100 10 "red"
# Blue: ~10 pixels (x=139, y=128-137)
assert_pixel_count "$DUMP" 0 50 200 100 150 255 10 "blue"
# Green: ~10 pixels (y=138, x=130-139)
assert_pixel_count "$DUMP" 0 150 0 100 255 100 10 "green"
# Yellow: ~11 pixels (x=129, y=128-138)
assert_pixel_count "$DUMP" 200 200 0 255 255 100 11 "yellow"

echo ""
echo "=== ALL CHECKS PASSED ==="

geos_kill
rm -f "$DUMP"
exit 0
