import sys
import zlib
import struct

def ppm_to_png(ppm_path, png_path):
    with open(ppm_path, 'rb') as f:
        header = f.readline().decode().strip()
        if header != 'P6':
            raise ValueError("Expected P6 PPM")
        line = f.readline().decode().strip()
        while line.startswith('#'):
            line = f.readline().decode().strip()
        width, height = map(int, line.split())
        maxval = int(f.readline().decode().strip())
        raw_bytes = f.read()

    # Create raw scanlines with filter byte 0 (None)
    scanlines = bytearray()
    row_bytes = width * 3
    for y in range(height):
        scanlines.append(0)  # filter type 0
        scanlines.extend(raw_bytes[y * row_bytes : (y + 1) * row_bytes])

    compressed = zlib.compress(scanlines, 9)

    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)

    png = bytearray(b'\x89PNG\r\n\x1a\n')
    # IHDR: width, height, bit_depth=8, color_type=2 (RGB), compression=0, filter=0, interlace=0
    png.extend(chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)))
    png.extend(chunk(b'IDAT', compressed))
    png.extend(chunk(b'IEND', b''))

    with open(png_path, 'wb') as f:
        f.write(png)
    print(f"Saved {png_path} ({width}x{height})")

if __name__ == '__main__':
    ppm_path = sys.argv[1] if len(sys.argv) > 1 else '/tmp/qemu_desktop.ppm'
    png_path = sys.argv[2] if len(sys.argv) > 2 else '/tmp/qemu_desktop.png'
    ppm_to_png(ppm_path, png_path)
