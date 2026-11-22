#!/usr/bin/env bash
set -e

rm -f /tmp/qemu_serial.log /tmp/qemu-mon.sock /tmp/v033_*.ppm /tmp/v033_*.png

echo "[+] Starting local Wi-Fi bridge helper in WSL on 0.0.0.0:19820..."
pkill -f windows_host_wifi_helper.py 2>/dev/null || true
python3 /home/vedav/rust-micro-os/tools/windows_host_wifi_helper.py > /tmp/helper.log 2>&1 &
HELPER_PID=$!
sleep 1

echo "[+] Launching QEMU with JainOS v0.3.3 ISO..."
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

echo "[+] Waiting for JainOS kernel and desktop compositor to boot (8s)..."
sleep 8

echo "[+] Running automated test suite and screenshot capture..."
python3 /home/vedav/rust-micro-os/scripts/capture_v033_suite.py

sleep 1
kill -9 $QPID 2>/dev/null || true
kill -9 $HELPER_PID 2>/dev/null || true

echo "[+] Converting captured PPM screendumps to PNG and staging to artifacts..."
for ppm in /tmp/v033_*.ppm; do
    if [ -f "$ppm" ]; then
        png="${ppm%.ppm}.png"
        python3 /home/vedav/rust-micro-os/scripts/ppm2png.py "$ppm" "$png"
        basename=$(basename "$png")
        cp "$png" "/mnt/c/Users/USER/.gemini/antigravity/brain/39aff397-e9da-4a2e-84b6-6e7b06ac7623/${basename}" || true
        echo "  [✓] Generated: $png -> artifacts/$basename"
    fi
done

echo "[+] Done! Serial log output summary:"
cat /tmp/qemu_serial.log | grep -E "\[SHELL|\[DHCP|\[BGA|\[SERIAL|JainOS Kernel" | tail -n 40
