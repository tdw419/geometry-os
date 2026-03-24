#!/bin/bash
# Circuit Watcher - Hot-reload ASCII circuits on file change
# Edit in VS Code, see updates live on GPU

WATCH_DIR="${1:-circuits/ascii}"
GPU_OFFSET_X="${2:-100}"
GPU_OFFSET_Y="${3:-100}"

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           CIRCUIT WATCHER - Hot Reload                    ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo "Watching: $WATCH_DIR"
echo "GPU Offset: ($GPU_OFFSET_X, $GPU_OFFSET_Y)"
echo ""
echo "Edit any .txt file and it will auto-inject into GPU"
echo "Press Ctrl+C to stop"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check for inotifywait
if ! command -v inotifywait &> /dev/null; then
    echo "Error: inotifywait not found"
    echo "Install: sudo apt install inotify-tools"
    exit 1
fi

# Watch for changes
inotifywait -m -r -e modify,create "$WATCH_DIR" --format '%w%f' | while read file; do
    if [[ "$file" == *.txt ]]; then
        echo "$(date '+%H:%M:%S') - Change detected: $file"
        
        # Inject into GPU
        ./target/release/scanner load -f "$file" -x $GPU_OFFSET_X -y $GPU_OFFSET_Y
        
        echo "✓ Injected into GPU at ($GPU_OFFSET_X, $GPU_OFFSET_Y)"
        echo ""
    fi
done
