#!/bin/bash
# Geometry OS Build Script
# Builds all binaries for release

set -e

echo "╔═══════════════════════════════════════════════════════╗"
echo "║           GEOMETRY OS — Build Script                  ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "✗ Rust not installed"
    echo "  Install from: https://rustup.rs"
    exit 1
fi

echo "✓ Rust installed"
echo ""

# Build release binaries
echo "Building release binaries..."
cargo build --release 2>&1 | grep -E "(Compiling|Finished)" || true

echo ""
echo "Binaries built:"
ls -lh target/release/agent target/release/siphon-demo 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║  ✓ Build complete                                     ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""
echo "Run with:"
echo "  ./target/release/agent          # CPU on framebuffer"
echo "  sudo ./target/release/agent     # Live /dev/fb0 output"
echo "  sudo ./target/release/siphon-demo --mouse  # Desktop siphon"
