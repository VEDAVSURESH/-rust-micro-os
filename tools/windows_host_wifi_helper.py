#!/usr/bin/env python3
"""
JainOS Windows Host Wi-Fi Helper Service
Author: Vedav Suresh (USN: 25BTRCL280)
Project: JainOS — Rust Micro Operating System

This lightweight service runs on the Windows host machine.
It queries the host's actual Wi-Fi adapter state and available SSIDs using Windows Netsh,
and provides this information to the JainOS guest over TCP port 19820.
"""

import subprocess
import socket
import sys
import re
import threading
import time

HOST = "0.0.0.0"
PORT = 19820

def get_host_wifi_info():
    info = {
        "connected_ssid": "Disconnected",
        "signal": "0%",
        "state": "DISCONNECTED",
        "adapter": "Unknown",
        "radio": "802.11ax",
        "available": []
    }

    try:
        # 1. Query active interface
        out = subprocess.check_output(["netsh", "wlan", "show", "interfaces"], encoding="utf-8", errors="ignore")
        for line in out.splitlines():
            line = line.strip()
            if line.startswith("Description"):
                info["adapter"] = line.split(":", 1)[1].strip()
            elif line.startswith("State"):
                val = line.split(":", 1)[1].strip()
                if "connected" in val.lower():
                    info["state"] = "CONNECTED"
            elif line.startswith("SSID") and not line.startswith("SSID "):
                info["connected_ssid"] = line.split(":", 1)[1].strip()
            elif line.startswith("Signal"):
                info["signal"] = line.split(":", 1)[1].strip()
            elif line.startswith("Radio type"):
                info["radio"] = line.split(":", 1)[1].strip()
    except Exception as e:
        print(f"[-] Error querying interfaces: {e}")

    try:
        # 2. Query available visible networks
        net_out = subprocess.check_output(["netsh", "wlan", "show", "networks"], encoding="utf-8", errors="ignore")
        ssids = []
        for line in net_out.splitlines():
            line = line.strip()
            if line.startswith("SSID ") and ":" in line:
                ssid_name = line.split(":", 1)[1].strip()
                if ssid_name and ssid_name not in ssids:
                    ssids.append(ssid_name)
        info["available"] = ssids
    except Exception as e:
        print(f"[-] Error querying networks: {e}")

    return info

def format_response(info):
    avail_str = ",".join(info["available"]) if info["available"] else "None"
    # Format plain text for easy parsing inside bare-metal Rust kernel
    return (
        f"RIFTNET_WIFI_V1\n"
        f"STATE:{info['state']}\n"
        f"SSID:{info['connected_ssid']}\n"
        f"SIGNAL:{info['signal']}\n"
        f"ADAPTER:{info['adapter']}\n"
        f"RADIO:{info['radio']}\n"
        f"AVAILABLE:{avail_str}\n"
        f"END\n"
    )

def handle_client(conn, addr):
    try:
        conn.settimeout(3.0)
        req = conn.recv(1024).decode('utf-8', errors='ignore')
        print(f"[+] Request from {addr}: {req.strip()}")

        if req.startswith("CONNECT"):
            # Optional connection command
            target = req.split(None, 1)[1].strip() if len(req.split()) > 1 else ""
            if target:
                subprocess.run(["netsh", "wlan", "connect", f"name={target}"], capture_output=True)
            time.sleep(1)

        info = get_host_wifi_info()
        resp = format_response(info)
        conn.sendall(resp.encode('utf-8'))
    except Exception as e:
        print(f"[-] Client error: {e}")
    finally:
        conn.close()

def main():
    print("=================================================================")
    print("       JainOS Windows Host Wi-Fi Helper Service (Port 19820)     ")
    print("       Providing real Windows laptop Wi-Fi telemetry to RiftNet  ")
    print("=================================================================")

    # Test local query
    initial = get_host_wifi_info()
    print(f"[*] Detected Host Adapter : {initial['adapter']}")
    print(f"[*] Current Connected SSID: {initial['connected_ssid']} ({initial['signal']})")
    print(f"[*] Visible Host Networks : {', '.join(initial['available'])}")

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((HOST, PORT))
    server.listen(5)
    print(f"[+] Listening on {HOST}:{PORT} (Reachable by JainOS at 10.0.2.2:{PORT})...\n")

    while True:
        try:
            conn, addr = server.accept()
            t = threading.Thread(target=handle_client, args=(conn, addr))
            t.daemon = True
            t.start()
        except KeyboardInterrupt:
            print("\n[*] Stopping helper...")
            break
        except Exception as e:
            print(f"[-] Server error: {e}")
            break

    server.close()

if __name__ == '__main__':
    main()
