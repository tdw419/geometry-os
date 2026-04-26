#!/bin/bash
# Install Geometry OS as a fullscreen display manager
# Usage: ./install_desktop.sh [--user]
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DIST_DIR="$SCRIPT_DIR"

echo "=== Geometry OS Desktop Installer ==="
echo "Project: $PROJECT_DIR"

# Build release binary
echo ""
echo "[1/4] Building release binary..."
cd "$PROJECT_DIR" && cargo build --release --features native 2>&1 | tail -3

BINARY="$PROJECT_DIR/target/release/geometry_os"
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    exit 1
fi
echo "Binary: $BINARY ($(stat -c%s "$BINARY") bytes)"

# Install systemd user service
echo ""
echo "[2/4] Installing systemd user service..."
SYSTEMD_DIR="$HOME/.config/systemd/user"
mkdir -p "$SYSTEMD_DIR"

# Update ExecStart path in service file
sed "s|ExecStart=.*|ExecStart=$BINARY --fullscreen|" \
    "$DIST_DIR/geometry-os.service" > "$SYSTEMD_DIR/geometry-os.service"
echo "Installed: $SYSTEMD_DIR/geometry-os.service"

# Install .desktop file
echo ""
echo "[3/4] Installing .desktop file..."
DESKTOP_DIR="$HOME/.local/share/applications"
mkdir -p "$DESKTOP_DIR"
sed "s|Exec=.*|Exec=$BINARY --fullscreen|" \
    "$DIST_DIR/geometry-os.desktop" > "$DESKTOP_DIR/geometry-os.desktop"
chmod +x "$DESKTOP_DIR/geometry-os.desktop"
echo "Installed: $DESKTOP_DIR/geometry-os.desktop"

# Reload systemd
echo ""
echo "[4/4] Reloading systemd..."
systemctl --user daemon-reload

echo ""
echo "=== Installation Complete ==="
echo ""
echo "To enable auto-start on login:"
echo "  systemctl --user enable geometry-os.service"
echo ""
echo "To start now:"
echo "  systemctl --user start geometry-os.service"
echo ""
echo "To stop:"
echo "  systemctl --user stop geometry-os.service"
echo ""
echo "To disable auto-start:"
echo "  systemctl --user disable geometry-os.service"
