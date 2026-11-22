#!/usr/bin/env bash
# ==============================================================================
# JainOS QEMU Virtual Machine Launcher
# Author: Vedav Suresh (USN: 25BTRCL280, Section: CSE AIML-E)
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
ISO="${BUILD_DIR}/JainOS.iso"

if [ ! -f "${ISO}" ]; then
    echo "[*] ISO not found at ${ISO}. Building now..."
    "${BUILD_DIR}/build_jainos_iso.sh"
fi

echo "=============================================================================="
echo "          Launching JainOS in QEMU (Intel PRO/1000 MT + NAT)                  "
echo "=============================================================================="
echo "Commands available inside JainOS:"
echo "  whoami, help, clear, mem, tasks, uname, version, date, pci, ifconfig, ping, curl, reboot"
echo "=============================================================================="

# If running headless or in WSL without X server, default to display none with serial stdio
DISPLAY_OPT=""
if [ -z "${DISPLAY:-}" ]; then
    DISPLAY_OPT="-display none"
fi

qemu-system-x86_64 \
    -cdrom "${ISO}" \
    -boot d \
    -m 1024 \
    -device e1000,netdev=net0,mac=52:54:00:12:34:56 \
    -netdev user,id=net0 \
    -serial stdio \
    ${DISPLAY_OPT}
