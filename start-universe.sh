#!/bin/bash
# Start the pixel universe with shared memory control
cd ~/zion/projects/ascii_world/gpu

echo "Starting Pixel Universe..."
echo "Shared memory: /tmp/pixel-universe.mem"
echo ""

# Clean up old files
rm -f /tmp/pixel-universe.mem
rm -f output/frame_*.png

# Run agent in background
cargo run --release --bin agent &
AGENT_PID=$!

# Wait for startup
sleep 2

echo ""
echo "Pixel Universe running (PID: $AGENT_PID)"
echo ""
echo "Signal Injector ready. Usage:"
echo "  ./target/release/injector inject -x 100 -y 100 -opcode REPLICATE -r 255 -g 0 -b 0"
echo "  ./target/release/injector wire --x1 50 --y1 50 --x2 200 --y2 50"
echo "  ./target/release/injector and-gate -x 150 -y 100"
echo "  ./target/release/injector interactive"
echo ""
echo "Press Ctrl+C to stop..."

# Wait for agent
wait $AGENT_PID
