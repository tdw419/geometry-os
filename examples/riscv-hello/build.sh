#!/bin/bash
# Build bare-metal RISC-V binaries for Geometry OS hypervisor.
#
# Usage:
#   ./build.sh              # builds hello.S (assembly)
#   ./build.sh hello.c      # builds hello.c -> hello.elf (C)
#   ./build.sh hello.c life.elf  # builds hello.c -> life.elf
#   ./build.sh --all        # builds all C programs that have a .c source
#   ./build.sh --lib        # rebuilds libgeos.a only
#
# Output: ELF binary in the current directory.
# Boot: hypervisor_boot arch=riscv32 kernel=hello.elf ram=1
#
# IMPORTANT: Geometry OS CPU supports RV32IMAC + Zicsr (see src/riscv/cpu/alu.rs for
# M extension, src/riscv/dtb.rs advertises rv32imac). We compile with
# -march=rv32imac_zicsr -mabi=ilp32 -- do NOT change to rv64/lp64 or the
# compiler will emit ld/sd/addiw instructions the CPU cannot execute.

set -e
cd "$(dirname "$0")"

CC=riscv64-linux-gnu-gcc
AR=riscv64-linux-gnu-ar
CFLAGS="-ffreestanding -nostdlib -nostartfiles -fno-pic -march=rv32imac_zicsr -mabi=ilp32 -O2 -static -no-pie -mcmodel=medany"
LDFLAGS="-T hello.ld -Wl,--no-dynamic-linker -Wl,-e,_start -Wl,--gc-sections"

# Build libgeos.a if missing or stale
build_lib() {
    if [ ! -f libgeos.a ] || [ libgeos.c -nt libgeos.a ] || [ libgeos.h -nt libgeos.a ]; then
        echo "Building libgeos.a..."
        $CC $CFLAGS -c libgeos.c -o libgeos.o
        $AR rcs libgeos.a libgeos.o
        rm -f libgeos.o
    fi
}

# Build a single program
build_one() {
    local SRC="$1"
    local OUT="$2"

    case "$SRC" in
        *.c)
            build_lib
            echo "Building $SRC -> $OUT (crt0.S + libgeos.a)"
            $CC $CFLAGS $LDFLAGS -o "$OUT" crt0.S "$SRC" -L. -lgeos
            ;;
        *.S)
            echo "Building $SRC -> $OUT (assembly, no libgeos)"
            $CC $CFLAGS $LDFLAGS -o "$OUT" "$SRC"
            ;;
        *)
            echo "Unknown source type: $SRC"; exit 1 ;;
    esac

    ENTRY=$($CC-readelf -h "$OUT" 2>/dev/null | grep 'Entry point' | awk '{print $NF}' || echo "?")
    SIZE=$(stat --format=%s "$OUT")
    echo "  Entry: $ENTRY  Size: ${SIZE} bytes"
}

# Build all C programs
build_all() {
    build_lib
    for src in *.c; do
        # Skip libgeos.c itself
        [ "$src" = "libgeos.c" ] && continue
        # Derive output name: foo.c -> foo.elf
        local base="${src%.c}"
        build_one "$src" "${base}.elf"
    done
}

# Main dispatch
case "${1:-}" in
    --all)
        build_all
        ;;
    --lib)
        rm -f libgeos.a
        build_lib
        echo "libgeos.a rebuilt."
        ;;
    "")
        # Default: build hello.S
        build_one "hello.S" "hello.elf"
        ;;
    *)
        SRC="$1"
        OUT="${2:-hello.elf}"
        build_one "$SRC" "$OUT"
        ;;
esac
