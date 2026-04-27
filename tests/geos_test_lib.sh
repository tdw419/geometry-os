#!/bin/bash
# geos_test_lib.sh — Shared test harness for Geometry OS RISC-V guest programs.
#
# Usage: source this file from test scripts.
# Provides: geos_boot, geos_run_elf, geos_inject, geos_inject_text,
#           geos_fb_dump, geos_kill, assert_pixel, assert_pixel_count.
#
# Requirements: socat, Pillow (python3-pil), GeOS binary built at
#               $GEOS_BIN (default: ./target/release/geometry_os)

set -euo pipefail

GEOS_SOCK="${GEOS_SOCK:-/tmp/geo_cmd.sock}"
GEOS_BIN="${GEOS_BIN:-./target/release/geometry_os}"
GEOS_PID=""
GEOS_DISPLAY="${DISPLAY:-:0}"
GEOS_TMPDIR=""

# ── Lifecycle ──────────────────────────────────────────────────────

geos_boot() {
    # Start GeOS in the background, wait for socket to appear.
    rm -f "$GEOS_SOCK"
    GEOS_TMPDIR=$(mktemp -d /tmp/geos_test_XXXXXX)
    DISPLAY="$GEOS_DISPLAY" "$GEOS_BIN" 2>"$GEOS_TMPDIR/geos_err.log" &
    GEOS_PID=$!
    # Wait for socket (up to 10s)
    for i in $(seq 1 20); do
        if [ -S "$GEOS_SOCK" ]; then return 0; fi
        sleep 0.5
    done
    echo "FAIL: GeOS socket did not appear after 10s"
    geos_kill
    return 1
}

geos_kill() {
    if [ -n "$GEOS_PID" ]; then
        kill "$GEOS_PID" 2>/dev/null || true
        wait "$GEOS_PID" 2>/dev/null || true
        GEOS_PID=""
    fi
    rm -f "$GEOS_SOCK"
    if [ -n "$GEOS_TMPDIR" ] && [ -d "$GEOS_TMPDIR" ]; then
        rm -rf "$GEOS_TMPDIR"
    fi
}

# ── Socket commands ────────────────────────────────────────────────

geos_cmd() {
    # Send a raw socket command and return the response.
    local cmd="$1"
    echo "$cmd" | socat - UNIX-CONNECT:"$GEOS_SOCK" 2>/dev/null
}

geos_run_elf() {
    # Launch a RISC-V ELF. Arg: path to .elf
    local elf="$1"
    geos_cmd "riscv_run $elf"
    sleep 1  # Let the guest initialize
}

geos_inject() {
    # Inject a single character by hex value.
    local hex="$1"
    geos_cmd "inject $hex"
}

geos_inject_text() {
    # Inject a text string (each char becomes a key event).
    local text="$1"
    geos_cmd "inject_text $text"
}

geos_fb_dump() {
    # Dump the RISC-V framebuffer to a PNG file. Arg: output path.
    local path="$1"
    geos_cmd "riscv_fb_dump $path"
}

# ── Assertions ─────────────────────────────────────────────────────

assert_pixel() {
    # Assert pixel at (x,y) matches expected RGB.
    # Args: png_path x y expected_r expected_g expected_b [tolerance]
    local png="$1"
    local x="$2"
    local y="$3"
    local er="$4"
    local eg="$5"
    local eb="$6"
    local tol="${7:-5}"  # tolerance, default ±5

    python3 -c "
from PIL import Image
img = Image.open('$png')
px = img.load()
r,g,b = px[$x,$y]
er,eg,eb = $er,$eg,$eb
tol = $tol
if abs(r-er) > tol or abs(g-eg) > tol or abs(b-eb) > tol:
    print(f'FAIL: pixel ($x,$y) = RGB({r},{g},{b}), expected RGB({er},{eg},{eb}) ±{tol}')
    exit(1)
else:
    print(f'  OK: pixel ($x,$y) = RGB({r},{g},{b})')
"
}

assert_pixel_count() {
    # Count pixels matching a color range in the canvas area (y < 244).
    # Args: png_path min_r min_g min_b max_r max_g max_b expected_count label
    local png="$1"
    local min_r="$2" min_g="$3" min_b="$4"
    local max_r="$5" max_g="$6" max_b="$7"
    local expected="$8"
    local label="${9:-pixels}"

    python3 -c "
from PIL import Image
img = Image.open('$png')
px = img.load()
count = 0
for y in range(0, 244):
    for x in range(0, 256):
        r,g,b = px[x,y]
        if $min_r <= r <= $max_r and $min_g <= g <= $max_g and $min_b <= b <= $max_b:
            count += 1
expected = $expected
if count != expected:
    print(f'FAIL: $label count = {count}, expected {expected}')
    exit(1)
else:
    print(f'  OK: $label count = {count}')
"
}
