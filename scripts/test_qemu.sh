#!/usr/bin/env bash
set -e

rm -f /tmp/qemu_serial.log /tmp/qemu-mon.sock /tmp/suite_*.ppm /tmp/suite_*.png

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

sleep 7

python3 /home/vedav/rust-micro-os/capture_suite.py

sleep 1
kill -9 $QPID 2>/dev/null || true

for ppm in /tmp/suite_*.ppm; do
    if [ -f "$ppm" ]; then
        png="${ppm%.ppm}.png"
        python3 /home/vedav/rust-micro-os/ppm2png.py "$ppm" "$png"
        basename=$(basename "$png")
        cp "$png" "/mnt/c/Users/USER/.gemini/antigravity/brain/39aff397-e9da-4a2e-84b6-6e7b06ac7623/${basename}" || true
    fi
done

echo "Screenshot suite processing complete."
