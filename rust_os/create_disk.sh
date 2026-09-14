#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "========================================================"
echo " Building Rust OS Dedicated Hard Disk / SSD Images"
echo "========================================================"

# 1. Generate initramfs tar
python3 ../generate_initfs.py

# 2. Build kernel and bootimage
cargo bootimage

# Create output build directory
BUILD_DIR="$SCRIPT_DIR/build"
mkdir -p "$BUILD_DIR"

RAW_BOOTIMAGE="$SCRIPT_DIR/target/x86_64-unknown-none/debug/bootimage-rust_os.bin"

# --------------------------------------------------------
# 1. Legacy BIOS Hard Disk / SSD Images
# --------------------------------------------------------
echo "[1/2] Generating Legacy BIOS Hard Disk / SSD Images..."

# RAW Disk image for physical HDD/SSD/USB flashing (dd / Rufus / Etcher)
cp "$RAW_BOOTIMAGE" "$BUILD_DIR/rust_os_bios.img"

# VMware VMDK Virtual Hard Disk (IDE / SATA)
qemu-img convert -f raw -O vmdk "$RAW_BOOTIMAGE" "$BUILD_DIR/rust_os_bios.vmdk"

# --------------------------------------------------------
# 2. UEFI Hard Disk / SSD Images
# --------------------------------------------------------
echo "[2/2] Generating UEFI Hard Disk / SSD Images..."

CACHE_DIR="$SCRIPT_DIR/.cache"
mkdir -p "$CACHE_DIR"
BOOTX64_EFI="$CACHE_DIR/BOOTX64.EFI"

if [ ! -f "$BOOTX64_EFI" ]; then
    echo "  Downloading Limine UEFI Bootloader..."
    curl -sSL -o "$BOOTX64_EFI" https://raw.githubusercontent.com/limine-bootloader/limine/v8.x-binary/BOOTX64.EFI
fi

# Create FAT EFI Partition Image
FAT_EFI_IMG="$BUILD_DIR/rust_os_uefi.img"
python3 make_efi_img.py "$BOOTX64_EFI" "$FAT_EFI_IMG"

# Convert to VMware VMDK for UEFI Hard Disk
qemu-img convert -f raw -O vmdk "$FAT_EFI_IMG" "$BUILD_DIR/rust_os_uefi.vmdk"

# Keep symbolic link/copy at root of rust_os for convenience
cp "$BUILD_DIR/rust_os_bios.vmdk" "$SCRIPT_DIR/rust_os.vmdk"

echo "--------------------------------------------------------"
echo " Hard Disk / SSD Images Generated Successfully in 'build/':"
echo "--------------------------------------------------------"
echo "  [VMware Dedicated Files]"
echo "    - BIOS Mode:  build/rust_os_bios.vmdk (also copied to rust_os/rust_os.vmdk)"
echo "    - UEFI Mode:  build/rust_os_uefi.vmdk"
echo ""
echo "  [Physical Hard Disk / SSD / USB Drive Raw Files]"
echo "    - BIOS Mode:  build/rust_os_bios.img"
echo "    - UEFI Mode:  build/rust_os_uefi.img"
echo "--------------------------------------------------------"
