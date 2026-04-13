#!/usr/bin/env bash
# download_riscv_linux.sh -- Download pre-built RISC-V 64-bit Linux kernel + rootfs
#
# Places files in .geometry_os/fs/linux/ for use with the Geometry OS QEMU bridge.
#
# Usage:
#   ./scripts/download_riscv_linux.sh          # download kernel + initrd
#   ./scripts/download_riscv_linux.sh --rootfs  # also build minimal Alpine rootfs
#
# Requirements: curl, gzip, qemu-system-riscv64 (for --verify)
#
# The kernel and initrd come from Debian's riscv64 netboot installer images,
# which are known-good for qemu-system-riscv64 -machine virt.

set -euo pipefail

# ── Config ─────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEST_DIR="$PROJECT_ROOT/.geometry_os/fs/linux"

KERNEL_URL="https://deb.debian.org/debian/dists/testing/main/installer-riscv64/current/images/netboot/debian-installer/riscv64/linux"
INITRD_URL="https://deb.debian.org/debian/dists/testing/main/installer-riscv64/current/images/netboot/debian-installer/riscv64/initrd.gz"
ROOTFS_URL="https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/riscv64/alpine-minirootfs-3.21.3-riscv64.tar.gz"

KERNEL_FILE="Image"
INITRD_FILE="initrd.gz"
ROOTFS_DIR="rootfs"

# ── Colors (if terminal supports it) ───────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[download_riscv_linux]${NC} $*"; }
warn()  { echo -e "${YELLOW}[download_riscv_linux]${NC} $*"; }
error() { echo -e "${RED}[download_riscv_linux]${NC} $*" >&2; exit 1; }

# ── Check dependencies ─────────────────────────────────────────────────
check_deps() {
    for cmd in curl; do
        if ! command -v "$cmd" &>/dev/null; then
            error "Missing dependency: $cmd"
        fi
    done
}

# ── Download with progress ─────────────────────────────────────────────
download() {
    local url="$1"
    local dest="$2"
    local desc="$3"

    if [[ -f "$dest" ]]; then
        info "$desc already exists, skipping ($dest)"
        return 0
    fi

    info "Downloading $desc..."
    info "  URL: $url"
    info "  Dest: $dest"

    curl --fail --location --progress-bar --output "$dest" "$url" || {
        error "Failed to download $desc from $url"
    }

    local size
    size=$(stat -c%s "$dest" 2>/dev/null || stat -f%z "$dest" 2>/dev/null || echo "?")
    info "$desc downloaded (${size} bytes)"
}

# ── Build minimal Alpine rootfs ────────────────────────────────────────
build_rootfs() {
    local rootfs_dir="$DEST_DIR/$ROOTFS_DIR"
    local rootfs_tar="$DEST_DIR/alpine-minirootfs.tar.gz"

    if [[ -d "$rootfs_dir" ]] && [[ -f "$rootfs_dir/bin/sh" ]]; then
        info "Rootfs already exists at $rootfs_dir, skipping"
        return 0
    fi

    download "$ROOTFS_URL" "$rootfs_tar" "Alpine minirootfs (riscv64)"

    info "Extracting rootfs to $rootfs_dir..."
    mkdir -p "$rootfs_dir"
    tar -xzf "$rootfs_tar" -C "$rootfs_dir"

    # Create minimal /init script
    cat > "$rootfs_dir/init" << 'INIT_EOF'
#!/bin/sh
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null
echo ""
echo "=== Geometry OS RISC-V Linux ==="
echo "Alpine minimal rootfs on QEMU riscv64 virt"
echo "Kernel: $(uname -r)"
echo "Arch:   $(uname -m)"
echo ""
exec /bin/sh
INIT_EOF
    chmod +x "$rootfs_dir/init"

    # Clean up tarball
    rm -f "$rootfs_tar"

    info "Rootfs ready at $rootfs_dir"
}

# ── Verify QEMU can boot ───────────────────────────────────────────────
verify_boot() {
    if ! command -v qemu-system-riscv64 &>/dev/null; then
        warn "qemu-system-riscv64 not found, skipping boot verification"
        warn "Install with: sudo apt install qemu-system-misc"
        return 0
    fi

    info "Verifying QEMU boot (5-second smoke test)..."

    local kernel="$DEST_DIR/$KERNEL_FILE"
    local initrd="$DEST_DIR/$INITRD_FILE"

    if [[ ! -f "$kernel" ]]; then
        error "Kernel not found at $kernel"
    fi

    # Boot QEMU with timeout, capture output, look for Linux boot messages
    local output
    output=$(timeout 10 qemu-system-riscv64 \
        -machine virt \
        -nographic \
        -m 256M \
        -kernel "$kernel" \
        -initrd "$initrd" \
        -append "console=ttyS0 panic=1" \
        2>&1 || true)

    if echo "$output" | grep -qi "Linux version"; then
        info "SUCCESS: Linux kernel boots! Found 'Linux version' in output."
        echo "$output" | head -5
    elif echo "$output" | grep -qi "OpenSBI"; then
        info "SUCCESS: OpenSBI starts. Kernel loading."
        echo "$output" | head -5
    else
        warn "Could not confirm Linux boot in output."
        warn "First 10 lines of QEMU output:"
        echo "$output" | head -10
    fi
}

# ── Main ────────────────────────────────────────────────────────────────
main() {
    local do_rootfs=false
    local do_verify=false

    for arg in "$@"; do
        case "$arg" in
            --rootfs)  do_rootfs=true ;;
            --verify)  do_verify=true ;;
            --help|-h)
                echo "Usage: $0 [--rootfs] [--verify] [--help]"
                echo ""
                echo "  --rootfs   Also download and extract Alpine minimal rootfs"
                echo "  --verify   Test-boot the kernel with qemu-system-riscv64"
                echo "  --help     Show this help"
                exit 0
                ;;
            *)
                error "Unknown argument: $arg (try --help)"
                ;;
        esac
    done

    check_deps

    info "Destination: $DEST_DIR"
    mkdir -p "$DEST_DIR"

    # Download kernel and initrd
    download "$KERNEL_URL" "$DEST_DIR/$KERNEL_FILE" "RISC-V 64-bit Linux kernel (Image)"
    download "$INITRD_URL" "$DEST_DIR/$INITRD_FILE" "RISC-V 64-bit initrd"

    # Optionally build rootfs
    if $do_rootfs; then
        build_rootfs
    fi

    # Print summary
    echo ""
    info "=== Download Complete ==="
    echo ""
    echo "Files in $DEST_DIR:"
    ls -lh "$DEST_DIR"
    echo ""
    echo "Boot with QEMU:"
    echo "  qemu-system-riscv64 \\"
    echo "    -machine virt \\"
    echo "    -nographic \\"
    echo "    -m 256M \\"
    echo "    -kernel $DEST_DIR/$KERNEL_FILE \\"
    echo "    -initrd $DEST_DIR/$INITRD_FILE \\"
    echo '    -append "console=ttyS0"'

    if $do_rootfs; then
        echo ""
        echo "Boot with rootfs (requires packing into ext4/cpio first):"
        echo "  # Create cpio initramfs from rootfs:"
        echo "  cd $DEST_DIR/$ROOTFS_DIR && find . | cpio -o -H newc | gzip > ../rootfs.cpio.gz"
        echo "  qemu-system-riscv64 \\"
        echo "    -machine virt -nographic -m 256M \\"
        echo "    -kernel $DEST_DIR/$KERNEL_FILE \\"
        echo "    -initrd $DEST_DIR/rootfs.cpio.gz \\"
        echo '    -append "console=ttyS0 root=/dev/ram0 init=/init"'
    fi

    # Optionally verify
    if $do_verify; then
        verify_boot
    fi

    echo ""
    info "Done."
}

main "$@"
