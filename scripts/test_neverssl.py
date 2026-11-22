import socket
import subprocess
import time

def main():
    q = subprocess.Popen([
        'qemu-system-x86_64',
        '-cdrom', '/home/vedav/rust-micro-os/build/JainOS.iso',
        '-boot', 'd',
        '-m', '512M',
        '-vga', 'std',
        '-device', 'e1000,netdev=net0',
        '-netdev', 'user,id=net0',
        '-serial', 'file:/tmp/serial_test.log',
        '-monitor', 'unix:/tmp/qmon.sock,server,nowait',
        '-nographic'
    ])
    time.sleep(8)
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect('/tmp/qmon.sock')

        def sm(cmd):
            s.sendall((cmd + '\n').encode())
            time.sleep(0.3)

        sm('sendkey f1')
        time.sleep(0.5)

        for ch in 'ping neverssl.com\n':
            if ch == ' ': sm('sendkey spc')
            elif ch == '\n': sm('sendkey ret')
            elif ch == '.': sm('sendkey dot')
            else: sm(f'sendkey {ch}')

        time.sleep(4)

        for ch in 'curl http://neverssl.com/\n':
            if ch == ' ': sm('sendkey spc')
            elif ch == '\n': sm('sendkey ret')
            elif ch == '.': sm('sendkey dot')
            elif ch == '/': sm('sendkey slash')
            elif ch == ':': sm('sendkey shift-semicolon')
            else: sm(f'sendkey {ch}')

        time.sleep(5)
    finally:
        q.terminate()
        time.sleep(1)

    print("Serial output:")
    with open('/tmp/serial_test.log') as f:
        print(f.read()[-2000:])

if __name__ == '__main__':
    main()
