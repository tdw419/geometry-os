#!/bin/bash
# Test signal injector

cd ~/zion/projects/ascii_world/gpu

echo "=== Signal Injector Test ==="
echo ""

# Test 1: Basic inject
echo "Test 1: Inject REPLICATE agent at (240, 120)"
./target/release/injector inject -x 240 -y 120 -o REPLICATE -r 255 -g 100 -b 50

echo ""
echo "Test 2: Draw a wire from (50,50) to (150,50)"
./target/release/injector wire --x1 50 --y1 50 --x2 150 --y2 50 --color FF0000

echo ""
echo "Test 3: Place AND gate at (200, 100)"
./target/release/injector and-gate -x 200 -y 100

echo ""
echo "Test 4: Place XOR gate at (250, 100)"
./target/release/injector xor-gate -x 250 -y 100

echo ""
echo "Test 5: Grid stats"
./target/release/injector stats

echo ""
echo "=== Test Complete ==="
echo ""
echo "To use interactively:"
echo "  ./target/release/injector interactive"
