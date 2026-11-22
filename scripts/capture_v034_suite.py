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
        elif ch == '5':
            send_mon(sock, 'sendkey 5')
        elif ch == '6':
            send_mon(sock, 'sendkey 6')
        elif ch == '7':
            send_mon(sock, 'sendkey 7')
        elif ch == '8':
            send_mon(sock, 'sendkey 8')
        elif ch == '9':
            send_mon(sock, 'sendkey 9')
        elif ch == '0':
            send_mon(sock, 'sendkey 0')
        elif ch == '\t':
            send_mon(sock, 'sendkey tab')
        elif ch == '\x08':
            send_mon(sock, 'sendkey backspace')
        elif ch.isupper():
            send_mon(sock, f'sendkey shift-{ch.lower()}')
        else:
            send_mon(sock, f'sendkey {ch.lower()}')
        time.sleep(0.06)

def run():
    print("[+] Connecting to QEMU monitor socket at /tmp/qemu-mon.sock...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop compositor and DHCP to settle
    time.sleep(4)

    # 1. Desktop Startup (clean desktop, wallpaper, taskbar, Network app + Terminal)
    print("[+] Capturing 1: Desktop Startup (v034_1_desktop_startup.ppm)")
    send_mon(sock, 'screendump /tmp/v034_1_desktop_startup.ppm')

    # 2. Focus Network Control Center (F2):
    # Verifies:
    # - Section headings: "Current Connection", "Ethernet Connection", "Wi-Fi Networks"
    # - REMOVED redundant header badges: no "Wi-Fi: CONNECTED" and no "eth0: UP" in header
    # - Unfocused password field showing placeholder "Enter Password"
    print("[+] Bringing Network Control Center to front (F2)...")
    send_mon(sock, 'sendkey f2')
    time.sleep(1.0)
    print("[+] Capturing 2: Network App Clean Interface (v034_2_network_clean_interface.ppm)")
    send_mon(sock, 'screendump /tmp/v034_2_network_clean_interface.ppm')

    # 3. Select a secured Wi-Fi network using arrow down / 'j'
    print("[+] Selecting secured Wi-Fi network in list ('j')...")
    send_chars(sock, "j")
    time.sleep(0.8)

    # 4. Focus Password Field (Tab or Enter)
    print("[+] Activating password field focus (Tab)...")
    send_mon(sock, 'sendkey tab')
    time.sleep(0.8)
    print("[+] Capturing 3: Password Field Focused with High-Visibility Glow (v034_3_password_focused.ppm)")
    send_mon(sock, 'screendump /tmp/v034_3_password_focused.ppm')

    # 5. Type Password: "JainOS@2026"
    print("[+] Typing password 'JainOS@2026' into secured Wi-Fi field...")
    send_chars(sock, "JainOS@2026")
    time.sleep(0.8)
    print("[+] Capturing 4: Password Field with Masked Bullet Characters (v034_4_password_typed_masked.ppm)")
    send_mon(sock, 'screendump /tmp/v034_4_password_typed_masked.ppm')

    # 6. Test Backspace Editing: Press backspace 4 times (removes "2026")
    print("[+] Pressing Backspace 4 times to verify editing...")
    for _ in range(4):
        send_mon(sock, 'sendkey backspace')
        time.sleep(0.1)
    time.sleep(0.5)
    print("[+] Capturing 5: Password Field after Backspace Editing (v034_5_password_backspaced.ppm)")
    send_mon(sock, 'screendump /tmp/v034_5_password_backspaced.ppm')

    # 7. Type replacement characters "999" and submit via Enter or CONNECT
    print("[+] Typing '999' and submitting connection via Enter...")
    send_chars(sock, "999\n")
    time.sleep(1.5)
    print("[+] Capturing 6: Connection Request Submitted (v034_6_connected_status.ppm)")
    send_mon(sock, 'screendump /tmp/v034_6_connected_status.ppm')

    # 8. Terminal: Focus Terminal (F1), run whoami, ping, curl
    print("[+] Focusing Terminal (F1) and executing 'whoami'...")
    send_mon(sock, 'sendkey f1')
    time.sleep(0.8)
    send_chars(sock, "whoami\n")
    time.sleep(0.5)
    send_chars(sock, "ping 1.1.1.1\n")
    time.sleep(1.5)
    send_chars(sock, "ping google.com\n")
    time.sleep(1.5)
    print("[+] Capturing 7: Terminal Execution whoami and ping (v034_7_terminal_whoami.ppm)")
    send_mon(sock, 'screendump /tmp/v034_7_terminal_whoami.ppm')

    # 9. Toggle Start Menu (F9)
    print("[+] Toggling Start Menu (F9)...")
    send_mon(sock, 'sendkey f9')
    time.sleep(0.8)
    print("[+] Capturing 8: Desktop Start Menu (v034_8_start_menu.ppm)")
    send_mon(sock, 'screendump /tmp/v034_8_start_menu.ppm')
    send_mon(sock, 'sendkey f9')
    time.sleep(0.4)

    # 10. Open About JainOS Dialog (F8)
    print("[+] Opening About JainOS Dialog (F8)...")
    send_mon(sock, 'sendkey f8')
    time.sleep(0.8)
    print("[+] Capturing 9: About JainOS v0.3.4 (v034_9_about_v034.ppm)")
    send_mon(sock, 'screendump /tmp/v034_9_about_v034.ppm')

    print("[+] All QEMU automated test captures completed successfully!")
    sock.close()

if __name__ == '__main__':
    run()
