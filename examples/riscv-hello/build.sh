#!/bin/bash
# Build bare-metal RISC-V binaries for Geometry OS hypervisor.
#
# Usage:
#   ./build.sh          # builds hello.S (assembly)
#   ./build.sh hello.c  # builds hello.c (C)
#   ./build.sh hello.S  # builds hello.S (assembly)
#
# Output: hello.elf in the current directory.
# Boot: hypervisor_boot arch=riscv32 kernel=hello.elf ram=1
#
# IMPORTANT: Geometry OS CPU is RV32I (see src/riscv/cpu/mod.rs). We compile
# with -march=rv32imac -mabi=ilp32 -- do NOT change to rv64/lp64 or the
# compiler will emit ld/sd/addiw instructions the CPU cannot execute.

set -e
cd "$(dirname "$0")"

SRC="${1:-hello.S}"
OUT="hello.elf"

# Detect C vs assembly source
case "$SRC" in
    *.c)
        LANG_FLAGS="-fno-pic"
        EXTRA_SRCS="crt0.S"
        ;;
    *.S)
        LANG_FLAGS=""
        EXTRA_SRCS=""
        ;;
    *)   echo "Unknown source type: $SRC"; exit 1 ;;
esac

echo "Building $SRC (with $EXTRA_SRCS) -> $OUT"

riscv64-linux-gnu-gcc \
    -ffreestanding \
    -nostdlib \
    -nostartfiles \
    -fno-pic \
    -march=rv32imac \
    -mabi=ilp32 \
    -T hello.ld \
    -O2 \
    -static \
    -no-pie \
    -mcmodel=medany \
    -Wl,--no-dynamic-linker \
    -Wl,-e,_start \
    -Wl,--gc-sections \
    $LANG_FLAGS \
    -o "$OUT" $EXTRA_SRCS "$SRC"

ENTRY=$(riscv64-linux-gnu-readelf -h "$OUT" | grep 'Entry point' | awk '{print $NF}')
SIZE=$(stat --format=%s "$OUT")

echo "Entry: $ENTRY  Size: ${SIZE} bytes"

# Quick smoke test: disassemble _start
riscv64-linux-gnu-objdump -d "$OUT" | head -30
