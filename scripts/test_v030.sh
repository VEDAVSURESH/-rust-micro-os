#!/usr/bin/env bash
set -e

rm -f /tmp/qemu_serial.log /tmp/qemu-mon.sock /tmp/v030_*.ppm /tmp/v030_*.png

echo "[+] Launching QEMU with JainOS ISO..."
qemu-system-x86_64 \
    -cdrom /home/vedav/rust-micro-os/build/JainOS.iso \
    -boot d \
    -m 512M \
    -vga std \
    -device e1000,netdev=net0 \
    -netdev user,id=net0 \
    -serial file:/tmp/qemu_serial.log \
    -monitor unix:/tmp/qemu-mon.sock,server,nowait \
    -vnc :1 &
QPID=$!

echo "[+] Waiting for JainOS kernel and desktop compositor to boot (7s)..."
sleep 7

echo "[+] Running automated test suite and screenshot capture..."
python3 /home/vedav/rust-micro-os/scripts/capture_v030_suite.py

sleep 1
kill -9 $QPID 2>/dev/null || true

echo "[+] Converting captured PPM screendumps to PNG..."
for ppm in /tmp/v030_*.ppm; do
    if [ -f "$ppm" ]; then
        png="${ppm%.ppm}.png"
        python3 /home/vedav/rust-micro-os/scripts/ppm2png.py "$ppm" "$png"
        basename=$(basename "$png")
        cp "$png" "/mnt/c/Users/USER/.gemini/antigravity/brain/39aff397-e9da-4a2e-84b6-6e7b06ac7623/${basename}" || true
    fi
done

echo "[+] Done! Check serial log and PNG images."
