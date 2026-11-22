import socket
import time

def send_mon(sock, cmd):
    sock.sendall((cmd + '\n').encode())
    time.sleep(1.2)

def run():
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop to settle
    time.sleep(2)

    # 1. Default desktop screenshot
    send_mon(sock, 'screendump /tmp/suite_1_terminal_net.ppm')

    # 2. Open Web Browser (F3)
    send_mon(sock, 'sendkey f3')
    send_mon(sock, 'screendump /tmp/suite_2_browser.ppm')

    # 3. Open File Manager (F4)
    send_mon(sock, 'sendkey f4')
    send_mon(sock, 'screendump /tmp/suite_3_files.ppm')

    # 4. Open System Monitor (F5)
    send_mon(sock, 'sendkey f5')
    send_mon(sock, 'screendump /tmp/suite_4_sysmon.ppm')

    # 5. Open Settings (F6)
    send_mon(sock, 'sendkey f6')
    send_mon(sock, 'screendump /tmp/suite_5_settings.ppm')

    # 6. Open About JainOS (F7)
    send_mon(sock, 'sendkey f7')
    send_mon(sock, 'screendump /tmp/suite_6_about.ppm')

    # 7. Toggle Start Menu (F8)
    send_mon(sock, 'sendkey f8')
    send_mon(sock, 'screendump /tmp/suite_7_startmenu.ppm')

    sock.close()
    print("All suite screendumps captured!")

if __name__ == '__main__':
    run()
