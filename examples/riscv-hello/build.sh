#!/bin/bash
# Build bare-metal RISC-V binaries for Geometry OS hypervisor.
#
# Usage:
#   ./build.sh          # builds hello.S (assembly)
#   ./build.sh hello.c  # builds hello.c (C)
#   ./build.sh hello.S  # builds hello.S (assembly)
#
# Output: hello.elf in the current directory.
# Boot: hypervisor_boot arch=riscv64 kernel=hello.elf ram=1

set -e
cd "$(dirname "$0")"

SRC="${1:-hello.S}"
OUT="hello.elf"

# Detect C vs assembly source
case "$SRC" in
    *.c) LANG_FLAGS="-fno-pic" ;;
    *.S) LANG_FLAGS="" ;;
    *)   echo "Unknown source type: $SRC"; exit 1 ;;
esac

echo "Building $SRC -> $OUT"

riscv64-linux-gnu-gcc \
    -ffreestanding \
    -nostdlib \
    -nostartfiles \
    -fno-pic \
    -march=rv64imac \
    -mabi=lp64 \
    -T hello.ld \
    -O2 \
    -static \
    -no-pie \
    -mcmodel=medany \
    -Wl,--no-dynamic-linker \
    -Wl,-e,_start \
    -Wl,--gc-sections \
    $LANG_FLAGS \
    -o "$OUT" "$SRC"

ENTRY=$(riscv64-linux-gnu-readelf -h "$OUT" | grep 'Entry point' | awk '{print $NF}')
SIZE=$(stat --format=%s "$OUT")

echo "Entry: $ENTRY  Size: ${SIZE} bytes"

# Quick smoke test: disassemble _start
riscv64-linux-gnu-objdump -d "$OUT" | head -30
