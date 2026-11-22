import socket
import time

def send_mon(sock, cmd):
    sock.sendall((cmd + '\n').encode())
    time.sleep(1.0)

def send_chars(sock, text):
    for ch in text:
        if ch == ' ':
            send_mon(sock, 'sendkey spc')
        elif ch == '\n':
            send_mon(sock, 'sendkey ret')
        elif ch == '-':
            send_mon(sock, 'sendkey minus')
        elif ch == '.':
            send_mon(sock, 'sendkey dot')
        elif ch == '/':
            send_mon(sock, 'sendkey slash')
        elif ch == ':':
            send_mon(sock, 'sendkey shift-semicolon')
        elif ch == '_':
            send_mon(sock, 'sendkey shift-minus')
        else:
            send_mon(sock, f'sendkey {ch.lower()}')
        time.sleep(0.08)

def run():
    print("[+] Connecting to QEMU monitor socket...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop and DHCP to settle
    time.sleep(4)

    # 1. Default desktop startup (shows Network app + Terminal)
    print("[+] Capturing 1: Desktop Startup")
    send_mon(sock, 'screendump /tmp/v031_1_desktop_startup.ppm')

    # 2. Focus Network Control Center (F2) to bring it to top
    print("[+] Capturing 2: Network Control Center with Live Scanned Wi-Fi")
    send_mon(sock, 'sendkey f2')
    time.sleep(1)
    send_chars(sock, "s")
    time.sleep(1.5)
    send_chars(sock, "p")
    time.sleep(1.5)
    send_mon(sock, 'screendump /tmp/v031_2_network_app_wifi_scan.ppm')

    # 3. Focus Browser (F3) - shows overhauled UI on example.com
    print("[+] Capturing 3: Modern Web Browser (example.com)")
    send_mon(sock, 'sendkey f3')
    time.sleep(1.5)
    send_mon(sock, 'screendump /tmp/v031_3_browser_example_com.ppm')

    # 4. Navigate Browser to live http://example.com/
    print("[+] Navigating Browser to live http://example.com/...")
    # Clear omnibox URL by sending backspaces
    for _ in range(35):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.03)
    time.sleep(0.3)
    # Type new URL and press Enter
    send_chars(sock, "http://example.com/\n")
    print("[+] Waiting for HTTP request to resolve and fetch (6s)...")
    time.sleep(6)
    print("[+] Capturing 4: Browser live fetch")
    send_mon(sock, 'screendump /tmp/v031_4_browser_neverssl.ppm')

    # 5. Focus Terminal (F1), run commands
    print("[+] Capturing 5: Terminal with whoami and ping 1.1.1.1")
    send_mon(sock, 'sendkey f1')
    time.sleep(0.8)
    send_chars(sock, "whoami\n")
    time.sleep(1)
    send_chars(sock, "ping 1.1.1.1\n")
    time.sleep(2)
    send_chars(sock, "help\n")
    time.sleep(1)
    send_mon(sock, 'screendump /tmp/v031_5_terminal_whoami_network.ppm')

    # 6. Start Menu with "Network" item (F9)
    print("[+] Capturing 6: Start Menu with Network app")
    send_mon(sock, 'sendkey f9')
    time.sleep(1)
    send_mon(sock, 'screendump /tmp/v031_6_start_menu_network.ppm')
    send_mon(sock, 'sendkey f9') # close start menu
    time.sleep(0.5)

    # 7. About Dialog (F8)
    print("[+] Capturing 7: About JainOS v0.3.1 Dialog")
    send_mon(sock, 'sendkey f8')
    time.sleep(1)
    send_mon(sock, 'screendump /tmp/v031_7_about_v031.ppm')

    # 8. Arcade Hub (F5)
    print("[+] Capturing 8: Arcade Games Hub")
    send_mon(sock, 'sendkey f5')
    time.sleep(1)
    send_mon(sock, 'screendump /tmp/v031_8_arcade_games.ppm')

    print("[+] All screenshots captured successfully!")

if __name__ == '__main__':
    run()
