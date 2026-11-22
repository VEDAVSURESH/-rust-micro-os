#!/usr/bin/env bash
# ==============================================================================
# JainOS Automated Standalone Bootable ISO Build Script
# Author: Vedav Suresh (USN: 25BTRCL280, Section: CSE AIML-E)
# Project: JainOS — Rust Micro Operating System
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${ROOT_DIR}/build"
STAGE_DIR="/tmp/jainos_iso_stage"

echo "=============================================================================="
echo "          JainOS — Rust Micro Operating System Automated Build                "
echo "   Author: Vedav Suresh  |  USN: 25BTRCL280  |  Section: CSE AIML-E          "
echo "=============================================================================="
echo ""

cd "${ROOT_DIR}"

# 1. Build bare-metal Rust kernel bootimage
echo "[1/4] Compiling bare-metal Rust kernel and generating bootable image..."
cargo bootimage

BOOT_BIN="${ROOT_DIR}/target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin"
if [ ! -f "${BOOT_BIN}" ]; then
    echo "[-] Error: Bootimage binary not found at ${BOOT_BIN}" >&2
    exit 1
fi

BIN_SIZE=$(stat -c%s "${BOOT_BIN}")
echo "[+] Kernel bootimage compiled successfully! Raw size: ${BIN_SIZE} bytes ($((BIN_SIZE / 1024)) KiB)."

# 2. Stage ISOLINUX and MEMDISK assets
echo "[2/4] Staging El Torito boot files and MEMDISK loader..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

ISOLINUX_BIN="/usr/lib/ISOLINUX/isolinux.bin"
LDLINUX_C32="/usr/lib/syslinux/modules/bios/ldlinux.c32"
MEMDISK_BIN="/usr/lib/syslinux/memdisk"

for f in "${ISOLINUX_BIN}" "${LDLINUX_C32}" "${MEMDISK_BIN}"; do
    if [ ! -f "$f" ]; then
        echo "[-] Error: Required syslinux component missing: $f" >&2
        echo "    Install with: sudo apt install -y isolinux syslinux-common" >&2
        exit 1
    fi
done

cp "${ISOLINUX_BIN}" "${STAGE_DIR}/"
cp "${LDLINUX_C32}" "${STAGE_DIR}/"
cp "${MEMDISK_BIN}" "${STAGE_DIR}/"
cp "${BOOT_BIN}" "${STAGE_DIR}/jainos.img"

cat << 'EOF' > "${STAGE_DIR}/isolinux.cfg"
SERIAL 0 115200
DEFAULT jainos
PROMPT 0
TIMEOUT 0
LABEL jainos
  MENU LABEL JainOS Microkernel
  KERNEL memdisk
  INITRD jainos.img
  APPEND harddisk
EOF

# 3. Create standalone bootable ISO with xorriso
echo "[3/4] Generating standalone bootable ISO with xorriso..."
ISO_OUTPUT="${BUILD_DIR}/JainOS.iso"
mkdir -p "${BUILD_DIR}"

xorriso -as mkisofs \
    -o "${ISO_OUTPUT}" \
    -b isolinux.bin \
    -c boot.cat \
    -no-emul-boot \
    -boot-load-size 4 \
    -boot-info-table \
    "${STAGE_DIR}" >/dev/null 2>&1

rm -rf "${STAGE_DIR}"

ISO_SIZE=$(stat -c%s "${ISO_OUTPUT}")
ISO_SIZE_MB=$(awk "BEGIN {printf \"%.2f\", ${ISO_SIZE}/1048576}")
echo "[+] Standalone bootable ISO generated: ${ISO_OUTPUT}"
echo "[+] Final ISO File Size: ${ISO_SIZE} bytes (${ISO_SIZE_MB} MB)."

# Verification of < 100 MB requirement
if [ "${ISO_SIZE}" -gt 104857600 ]; then
    echo "[-] ERROR: Final ISO exceeds the strict 100 MB college limit!" >&2
    exit 1
else
    echo "[✓] College Requirement PASSED: Final ISO (${ISO_SIZE_MB} MB) is strictly BELOW 100 MB!"
fi

# 4. Generate auxiliary VirtualBox disk images (VDI & VMDK)
echo "[4/4] Creating auxiliary disk images (VDI and VMDK) for VirtualBox..."
VDI_OUTPUT="${BUILD_DIR}/JainOS.vdi"
VMDK_OUTPUT="${BUILD_DIR}/JainOS.vmdk"

rm -f "${VDI_OUTPUT}" "${VMDK_OUTPUT}"
qemu-img convert -f raw -O vdi "${BOOT_BIN}" "${VDI_OUTPUT}"
qemu-img convert -f raw -O vmdk "${BOOT_BIN}" "${VMDK_OUTPUT}"

echo "[+] VirtualBox VDI image: ${VDI_OUTPUT}"
echo "[+] VirtualBox VMDK image: ${VMDK_OUTPUT}"
echo ""
echo "=============================================================================="
echo "                           BUILD COMPLETE!                                    "
echo "=============================================================================="
echo "Artifacts generated in ${BUILD_DIR}:"
ls -lh "${BUILD_DIR}"
echo "=============================================================================="
