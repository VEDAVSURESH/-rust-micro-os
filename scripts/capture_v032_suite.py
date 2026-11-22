import socket
import time
import sys

def send_mon(sock, cmd):
    sock.sendall((cmd + '\n').encode())
    time.sleep(0.8)

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
        elif ch == '1':
            send_mon(sock, 'sendkey 1')
        elif ch == '2':
            send_mon(sock, 'sendkey 2')
        elif ch == '3':
            send_mon(sock, 'sendkey 3')
        elif ch == '4':
            send_mon(sock, 'sendkey 4')
        else:
            send_mon(sock, f'sendkey {ch.lower()}')
        time.sleep(0.06)

def run():
    print("[+] Connecting to QEMU monitor socket at /tmp/qemu-mon.sock...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop, network driver, and DHCP to settle
    time.sleep(4)

    # 1. Desktop Startup (shows desktop, wallpaper, taskbar, Network app + Terminal)
    print("[+] Capturing 1: Desktop Startup")
    send_mon(sock, 'screendump /tmp/v032_1_desktop_startup.ppm')

    # 2. Focus Network Control Center (F2) to bring it to top, trigger scan and ping
    print("[+] Capturing 2: Network Control Center with Live Scanned Wi-Fi")
    send_mon(sock, 'sendkey f2')
    time.sleep(0.8)
    # Send 's' for SCAN
    send_chars(sock, "s")
    time.sleep(1.5)
    # Send 'p' for PING 1.1.1.1
    send_chars(sock, "p")
    time.sleep(1.5)
    send_mon(sock, 'screendump /tmp/v032_2_network_app_wifi_scan.ppm')

    # 3. Focus Browser (F3) - shows overhauled UI with multi-tabs on example.com
    print("[+] Capturing 3: Modern Web Browser (Tab 0: example.com)")
    send_mon(sock, 'sendkey f3')
    time.sleep(1.2)
    send_mon(sock, 'screendump /tmp/v032_3_browser_example_com.ppm')

    # 4. In Browser, navigate to live http://neverssl.com/
    print("[+] Navigating Browser to live http://neverssl.com/...")
    # Clear omnibox URL by sending backspaces
    for _ in range(40):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.02)
    time.sleep(0.2)
    # Type new URL and press Enter
    send_chars(sock, "http://neverssl.com/\n")
    print("[+] Waiting for HTTP request to resolve and fetch (7s)...")
    time.sleep(7)
    print("[+] Capturing 4: Browser on live NeverSSL portal")
    send_mon(sock, 'screendump /tmp/v032_4_browser_neverssl.ppm')

    # 5. Focus Terminal (F1), run commands (whoami, ping 1.1.1.1, ping google.com, curl)
    print("[+] Capturing 5: Terminal with whoami, ping 1.1.1.1, ping google.com, curl")
    send_mon(sock, 'sendkey f1')
    time.sleep(0.8)
    send_chars(sock, "whoami\n")
    time.sleep(1.0)
    send_chars(sock, "ping 1.1.1.1\n")
    time.sleep(1.8)
    send_chars(sock, "ping google.com\n")
    time.sleep(2.0)
    send_chars(sock, "curl http://example.com\n")
    time.sleep(2.5)
    send_mon(sock, 'screendump /tmp/v032_5_terminal_whoami_ping.ppm')

    # 6. Arcade Suite (F5) - Game 1: Snake
    print("[+] Capturing 6: Game - Snake")
    send_mon(sock, 'sendkey f5')
    time.sleep(0.8)
    send_chars(sock, "1") # Launch Snake
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_6_game_snake.ppm')
    send_chars(sock, "q") # Return to launcher
    time.sleep(0.5)

    # 7. Arcade Suite - Game 2: Pong
    print("[+] Capturing 7: Game - Pong")
    send_chars(sock, "2") # Launch Pong
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_7_game_pong.ppm')
    send_chars(sock, "q") # Return to launcher
    time.sleep(0.5)

    # 8. Arcade Suite - Game 3: Tic-Tac-Toe
    print("[+] Capturing 8: Game - Tic-Tac-Toe")
    send_chars(sock, "3") # Launch Tic-Tac-Toe
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_8_game_tictactoe.ppm')
    send_chars(sock, "q") # Return to launcher
    time.sleep(0.5)

    # 9. Arcade Suite - Game 4: 2048
    print("[+] Capturing 9: Game - 2048")
    send_chars(sock, "4") # Launch 2048
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_9_game_2048.ppm')
    send_chars(sock, "q") # Return to launcher
    time.sleep(0.5)

    # 10. Start Menu with "Network" (F9)
    print("[+] Capturing 10: Start Menu with Network application")
    send_mon(sock, 'sendkey f9')
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_10_start_menu_network.ppm')
    send_mon(sock, 'sendkey f9') # close start menu
    time.sleep(0.5)

    # 11. About Dialog (F8)
    print("[+] Capturing 11: About JainOS v0.3.2 Dialog")
    send_mon(sock, 'sendkey f8')
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v032_11_about_v032.ppm')

    print("[+] All 11 screenshots captured successfully!")

if __name__ == '__main__':
    run()
