import socket
import time
import sys

def send_mon(sock, cmd):
    sock.sendall((cmd + '\n').encode())
    time.sleep(0.7)

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
        time.sleep(0.05)

def run():
    print("[+] Connecting to QEMU monitor socket at /tmp/qemu-mon.sock...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop compositor and DHCP to settle
    time.sleep(4)

    # 1. Desktop Startup (shows desktop, wallpaper, taskbar, Network app + Terminal)
    print("[+] Capturing 1: Desktop Startup")
    send_mon(sock, 'screendump /tmp/v033_1_desktop_startup.ppm')

    # 2. Focus Network Control Center (F2): Confirms 3 separated sections and CONNECTED state
    print("[+] Capturing 2: Network Control Center (Connected state)")
    send_mon(sock, 'sendkey f2')
    time.sleep(0.8)
    send_chars(sock, "s") # Trigger scan
    time.sleep(1.2)
    send_chars(sock, "p") # Trigger ICMP ping
    time.sleep(1.5)
    send_mon(sock, 'screendump /tmp/v033_2_network_app_connected.ppm')

    # 3. Test Disconnection: Press 'w' (toggle Wi-Fi OFF)
    print("[+] Toggling Wi-Fi radio OFF to verify DISCONNECTED state...")
    send_chars(sock, "w")
    time.sleep(1.2)
    print("[+] Capturing 3: Network Control Center showing DISCONNECTED state independently from eth0")
    send_mon(sock, 'screendump /tmp/v033_3_network_app_disconnected.ppm')

    # 4. Turn Wi-Fi back ON ('w') and refresh ('s')
    print("[+] Toggling Wi-Fi radio back ON and refreshing...")
    send_chars(sock, "w")
    time.sleep(1.0)
    send_chars(sock, "s")
    time.sleep(1.5)
    print("[+] Capturing 4: Network Control Center restored to CONNECTED")
    send_mon(sock, 'screendump /tmp/v033_4_network_app_reconnected.ppm')

    # 5. Focus Browser (F3): Confirms professional JainOS Start Page (about:start)
    print("[+] Capturing 5: Web Browser - JainOS Start Page (about:start)")
    send_mon(sock, 'sendkey f3')
    time.sleep(1.2)
    send_mon(sock, 'screendump /tmp/v033_5_browser_start_page.ppm')

    # 6. Navigate Browser to local about:jainos (offline technical architecture website)
    print("[+] Navigating Browser to local about:jainos documentation...")
    for _ in range(40):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.02)
    time.sleep(0.2)
    send_chars(sock, "about:jainos\n")
    time.sleep(1.5)
    print("[+] Capturing 6: Browser on local technical architecture page (about:jainos)")
    send_mon(sock, 'screendump /tmp/v033_6_browser_jainos_doc.ppm')

    # 7. Navigate Browser to live http://example.com/
    print("[+] Navigating Browser to live http://example.com/...")
    for _ in range(40):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.02)
    time.sleep(0.2)
    send_chars(sock, "http://example.com/\n")
    print("[+] Waiting for HTTP request to resolve and fetch (6s)...")
    time.sleep(6.0)
    print("[+] Capturing 7: Browser on live Example Domain")
    send_mon(sock, 'screendump /tmp/v033_7_browser_example_com.ppm')

    # 8. Navigate Browser to live http://neverssl.com/
    print("[+] Navigating Browser to live http://neverssl.com/...")
    for _ in range(40):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.02)
    time.sleep(0.2)
    send_chars(sock, "http://neverssl.com/\n")
    print("[+] Waiting for HTTP request to resolve and fetch (6s)...")
    time.sleep(6.0)
    print("[+] Capturing 8: Browser on live NeverSSL portal")
    send_mon(sock, 'screendump /tmp/v033_8_browser_neverssl.ppm')

    # 9. Test Bookmarks Drawer: Click Bookmarks button (rel_x: 660, rel_y: 42 in Browser window at x:130, y:45)
    # Using mouse_move or hotkey/drawer toggle. In Browser: rel_x: 660, rel_y: 42
    # Absolute coord: x = 130 + 660 = 790, y = 45 + 42 = 87
    print("[+] Toggling Bookmarks Drawer in Browser...")
    send_mon(sock, 'mouse_move 790 87')
    time.sleep(0.3)
    send_mon(sock, 'mouse_button 1')
    time.sleep(0.1)
    send_mon(sock, 'mouse_button 0')
    time.sleep(1.0)
    print("[+] Capturing 9: Browser Bookmarks Drawer")
    send_mon(sock, 'screendump /tmp/v033_9_browser_bookmarks_drawer.ppm')

    # 10. Test History Drawer: rel_x: 710, rel_y: 42 -> abs x: 840, y: 87
    print("[+] Toggling History Drawer in Browser...")
    send_mon(sock, 'mouse_move 840 87')
    time.sleep(0.3)
    send_mon(sock, 'mouse_button 1')
    time.sleep(0.1)
    send_mon(sock, 'mouse_button 0')
    time.sleep(1.0)
    print("[+] Capturing 10: Browser History Drawer")
    send_mon(sock, 'screendump /tmp/v033_10_browser_history_drawer.ppm')

    # Close drawer by clicking document area: x: 400, y: 200
    send_mon(sock, 'mouse_move 400 200')
    time.sleep(0.2)
    send_mon(sock, 'mouse_button 1')
    time.sleep(0.1)
    send_mon(sock, 'mouse_button 0')
    time.sleep(0.5)

    # 11. Test Error Handling: Enter https://google.com/
    print("[+] Testing HTTPS unsupported error page...")
    for _ in range(40):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.02)
    time.sleep(0.2)
    send_chars(sock, "https://google.com/\n")
    time.sleep(1.2)
    print("[+] Capturing 11: Browser HTTPS Unsupported Error Page")
    send_mon(sock, 'screendump /tmp/v033_11_browser_https_error.ppm')

    # 12. Focus Terminal (F1), run commands: whoami, ping 1.1.1.1, ping google.com, curl
    print("[+] Capturing 12: Terminal with whoami, ping, and curl")
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
    send_mon(sock, 'screendump /tmp/v033_12_terminal_whoami_network.ppm')

    # 13. Arcade Suite (F5)
    print("[+] Capturing 13: Arcade Games Suite")
    send_mon(sock, 'sendkey f5')
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v033_13_arcade_games.ppm')

    # 14. Start Menu with "Network" and "Browser" (F9)
    print("[+] Capturing 14: Start Menu")
    send_mon(sock, 'sendkey f9')
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v033_14_start_menu_network.ppm')
    send_mon(sock, 'sendkey f9') # close start menu
    time.sleep(0.5)

    # 15. About Dialog (F8)
    print("[+] Capturing 15: About JainOS v0.3.3 Dialog")
    send_mon(sock, 'sendkey f8')
    time.sleep(1.0)
    send_mon(sock, 'screendump /tmp/v033_15_about_v033.ppm')

    print("[+] All 15 automated test stages completed successfully!")

if __name__ == '__main__':
    run()
