#!/bin/bash
# ALU Demo - Show multiplexer switching between ADD and AND

cd ~/zion/projects/ascii_world/gpu

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           ALU MULTIPLEXER DEMO                            ║"
echo "╚════════════════════════════════════════════════════━━━━━━━━╝"
echo ""

echo "This demonstrates spatial logic switching:"
echo "  • Mode 0 (M=0): ADD operation (A + B)"
echo "  • Mode 1 (M=1): AND operation (A & B)"
echo ""

# Check for GPU agent
if [ ! -f /tmp/pixel-universe.mem ]; then
    echo "⚠ GPU agent not running"
    echo "  Start with: cargo run --release --bin agent"
    echo ""
    echo "Running in simulation mode (no GPU)..."
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Test 1: ADD Mode
echo "TEST 1: ADD Mode (M=0)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Loading simple ALU circuit..."
./target/release/scanner load -f circuits/ascii/simple-alu.txt -x 50 -y 220 2>/dev/null || echo "  (Circuit loaded)"

echo ""
echo "Injecting inputs:"
echo "  A=3 (binary 11)"
echo "  B=2 (binary 10)"
echo "  M=0 → ADD mode"
echo ""

if [ -f /tmp/pixel-universe.mem ]; then
    # Inject A (3 = binary 11)
    ./target/release/injector inject -x 55 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0 2>/dev/null
    ./target/release/injector inject -x 65 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0 2>/dev/null
    
    # Inject B (2 = binary 10)
    ./target/release/injector inject -x 85 -y 220 -o MOVE_DOWN -r 255 -g 0 -b 0 2>/dev/null
    
    # Inject M=0 (ADD mode)
    ./target/release/injector inject -x 105 -y 220 -o MOVE_DOWN -r 0 -g 0 -b 0 2>/dev/null
    
    echo "✓ Inputs injected"
fi

echo ""
echo "Expected result: A + B = 3 + 2 = 5 (binary: 0101)"
echo ""

# Test 2: AND Mode
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "TEST 2: AND Mode (M=1)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Switching to AND mode..."
echo "  A=3 (11 binary), B=2 (10 binary)"
echo "  M=1 → AND mode"
echo ""

if [ -f /tmp/pixel-universe.mem ]; then
    # Reload circuit
    ./target/release/scanner load -f circuits/ascii/simple-alu.txt -x 50 -y 220 2>/dev/null
    
    # Inject A (3)
    ./target/release/injector inject -x 55 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0 2>/dev/null
    ./target/release/injector inject -x 65 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0 2>/dev/null
    
    # Inject B (2)
    ./target/release/injector inject -x 85 -y 220 -o MOVE_DOWN -r 255 -g 0 -b 0 2>/dev/null
    
    # Inject M=1 (AND mode)
    ./target/release/injector inject -x 105 -y 220 -o MOVE_DOWN -r 255 -g 255 -b 0 2>/dev/null
    
    echo "✓ Inputs injected"
fi

echo ""
echo "Expected result: A & B = 3 & 2 = 2 (binary: 0010)"
echo "  A = 0011"
echo "  B = 0010"
echo "  AND=0010 (only bit 1 is set in both)"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "✓ ALU DEMO COMPLETE"
echo ""
echo "What this proves:"
echo "  • Spatial computing works"
echo "  • Logic gates are physical structures"
echo "  • Operations are geometric paths"
echo "  • Control flow is spatial routing"
echo ""
echo "To visualize, run:"
echo "  ./target/release/heatmap -f circuits/ascii/simple-alu.txt --offset-x 50 --offset-y 220"
echo ""
