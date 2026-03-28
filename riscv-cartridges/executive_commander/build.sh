#!/bin/bash
# Build the Executive Commander RISC-V cartridge
set -e

cd "$(dirname "$0")"

echo "Building Executive Commander (riscv32im, no compressed instructions)..."
RUSTFLAGS="-C link-arg=-T$(pwd)/riscv.ld" \
    cargo build --release --target riscv32im-unknown-none-elf

ELF="target/riscv32im-unknown-none-elf/release/executive-commander"

echo ""
echo "Binary: $ELF"
echo "Size: $(wc -c < "$ELF") bytes"
echo ""

# Show section sizes
if command -v rust-objdump &> /dev/null; then
    echo "Sections:"
    rust-objdump --section-headers "$ELF" 2>/dev/null | grep -E "^\s+[0-9]"
    echo ""
    echo "First 10 instructions:"
    rust-objdump --disassemble "$ELF" 2>/dev/null | head -20
fi

echo ""
echo "Done. Run from gpu/ with:"
echo "  cargo run --bin executive-commander"
echo "  cargo run --bin executive-commander -- --cmd ping"
echo "  cargo run --bin executive-commander -- --cmd status"
echo "  cargo run --bin executive-commander -- --cmd assign --target 3 --payload 42"
