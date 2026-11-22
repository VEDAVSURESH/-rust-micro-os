import socket
import time

def send_mon(sock, cmd):
    sock.sendall((cmd + '\n').encode())
    time.sleep(1.2)

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
        else:
            send_mon(sock, f'sendkey {ch.lower()}')
        time.sleep(0.1)

def run():
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('/tmp/qemu-mon.sock')

    # Wait for desktop to settle
    time.sleep(2)

    # 1. Default desktop startup
    send_mon(sock, 'screendump /tmp/v030_1_desktop_startup.ppm')

    # 2. Focus terminal and type whoami + ping 1.1.1.1
    send_mon(sock, 'sendkey f1')
    time.sleep(0.5)
    send_chars(sock, "whoami\n")
    time.sleep(1)
    send_chars(sock, "ping 1.1.1.1\n")
    time.sleep(1.5)
    send_mon(sock, 'screendump /tmp/v030_2_terminal_whoami.ppm')

    # 3. Focus RiftNet Manager (F2)
    send_mon(sock, 'sendkey f2')
    send_mon(sock, 'screendump /tmp/v030_3_riftnet_manager.ppm')

    # 4. Web Browser (F3)
    send_mon(sock, 'sendkey f3')
    send_mon(sock, 'screendump /tmp/v030_4_browser_html.ppm')

    # 5. Arcade Games Hub (F5)
    send_mon(sock, 'sendkey f5')
    send_mon(sock, 'screendump /tmp/v030_5_arcade_hub.ppm')

    # 6. Snake (press 1)
    send_chars(sock, "1")
    time.sleep(0.8)
    # Move snake a bit
    send_chars(sock, "d")
    time.sleep(0.3)
    send_chars(sock, "s")
    time.sleep(0.3)
    send_mon(sock, 'screendump /tmp/v030_6_game_snake.ppm')
    send_chars(sock, "q") # back to hub
    time.sleep(0.5)

    # 7. Pong (press 2)
    send_chars(sock, "2")
    time.sleep(1.0)
    send_chars(sock, "s")
    time.sleep(0.3)
    send_mon(sock, 'screendump /tmp/v030_7_game_pong.ppm')
    send_chars(sock, "q") # back to hub
    time.sleep(0.5)

    # 8. Tic-Tac-Toe (press 3)
    send_chars(sock, "3")
    time.sleep(0.8)
    send_mon(sock, 'screendump /tmp/v030_8_game_tictactoe.ppm')
    send_chars(sock, "q") # back to hub
    time.sleep(0.5)

    # 9. 2048 (press 4)
    send_chars(sock, "4")
    time.sleep(0.8)
    send_chars(sock, "a")
    time.sleep(0.3)
    send_chars(sock, "w")
    time.sleep(0.3)
    send_mon(sock, 'screendump /tmp/v030_9_game_2048.ppm')
    send_chars(sock, "q") # back to hub
    time.sleep(0.5)

    # 10. File Manager (F4)
    send_mon(sock, 'sendkey f4')
    send_mon(sock, 'screendump /tmp/v030_10_file_manager.ppm')

    # 11. System Monitor (F6)
    send_mon(sock, 'sendkey f6')
    send_mon(sock, 'screendump /tmp/v030_11_system_monitor.ppm')

    # 12. Settings (F7)
    send_mon(sock, 'sendkey f7')
    send_mon(sock, 'screendump /tmp/v030_12_settings.ppm')

    # 13. About JainOS (F8)
    send_mon(sock, 'sendkey f8')
    send_mon(sock, 'screendump /tmp/v030_13_about.ppm')

    # 14. Start Menu (F9)
    send_mon(sock, 'sendkey f9')
    send_mon(sock, 'screendump /tmp/v030_14_start_menu.ppm')

    sock.close()
    print("All v0.3.0 screenshots successfully captured!")

if __name__ == '__main__':
    run()
