#!/usr/bin/env python3
"""
JainOS Windows Host Wi-Fi Helper Service (v0.3.1)
Author: Vedav Suresh (USN: 25BTRCL280)
Project: JainOS — Rust Micro Operating System

Provides genuine host Windows Wi-Fi telemetry and connection management
to the JainOS guest operating system over TCP port 19820 (gateway 10.0.2.2).
"""

import subprocess
import socket
import sys
import time
import threading
import shutil

HOST = "0.0.0.0"
PORT = 19820
NETSH_BIN = shutil.which("netsh") or shutil.which("netsh.exe") or "netsh.exe"

def get_wifi_info():
    iface = {
        "adapter": "Wi-Fi",
        "state": "DISCONNECTED",
        "connected_ssid": "None",
        "signal": "0%",
        "radio": "802.11ax",
        "auth": "None",
    }

    try:
        out = subprocess.check_output([NETSH_BIN, "wlan", "show", "interfaces"], text=True, errors="ignore")
        for line in out.splitlines():
            line = line.strip()
            if line.startswith("Description"):
                iface["adapter"] = line.split(":", 1)[1].strip()
            elif line.startswith("State"):
                val = line.split(":", 1)[1].strip().lower()
                iface["state"] = "CONNECTED" if "connected" in val else "DISCONNECTED"
            elif line.startswith("SSID") and ":" in line and not line.startswith("SSID 1"):
                parts = line.split(":", 1)
                if parts[0].strip() == "SSID":
                    iface["connected_ssid"] = parts[1].strip()
            elif line.startswith("Signal"):
                iface["signal"] = line.split(":", 1)[1].strip()
            elif line.startswith("Radio type"):
                iface["radio"] = line.split(":", 1)[1].strip()
            elif line.startswith("Authentication"):
                iface["auth"] = line.split(":", 1)[1].strip()
    except Exception as e:
        print(f"[-] Interface error: {e}")

    # Read saved profiles
    saved_profiles = set()
    try:
        p_out = subprocess.check_output([NETSH_BIN, "wlan", "show", "profiles"], text=True, errors="ignore")
        for line in p_out.splitlines():
            line = line.strip()
            if "All User Profile" in line and ":" in line:
                prof = line.split(":", 1)[1].strip()
                if prof:
                    saved_profiles.add(prof)
    except Exception as e:
        print(f"[-] Profile error: {e}")

    # Read visible networks
    networks = []
    seen_ssids = set()
    try:
        n_out = subprocess.check_output([NETSH_BIN, "wlan", "show", "networks", "mode=bssid"], text=True, errors="ignore")
        cur_net = None
        for line in n_out.splitlines():
            line = line.strip()
            if line.startswith("SSID ") and ":" in line:
                if cur_net and cur_net["ssid"] and cur_net["ssid"] not in seen_ssids:
                    networks.append(cur_net)
                    seen_ssids.add(cur_net["ssid"])
                ssid_val = line.split(":", 1)[1].strip()
                cur_net = {
                    "ssid": ssid_val,
                    "auth": "Open",
                    "signal": "0%",
                    "connected": (ssid_val == iface["connected_ssid"] and iface["state"] == "CONNECTED"),
                    "saved": (ssid_val in saved_profiles),
                }
            elif cur_net:
                if line.startswith("Authentication"):
                    cur_net["auth"] = line.split(":", 1)[1].strip()
                elif line.startswith("Signal"):
                    cur_net["signal"] = line.split(":", 1)[1].strip()
        if cur_net and cur_net["ssid"] and cur_net["ssid"] not in seen_ssids:
            networks.append(cur_net)
            seen_ssids.add(cur_net["ssid"])
    except Exception as e:
        print(f"[-] Networks error: {e}")

    # Always ensure connected network is in the list
    if iface["state"] == "CONNECTED" and iface["connected_ssid"] != "None":
        if iface["connected_ssid"] not in seen_ssids:
            networks.insert(0, {
                "ssid": iface["connected_ssid"],
                "auth": iface["auth"],
                "signal": iface["signal"],
                "connected": True,
                "saved": True,
            })
            seen_ssids.add(iface["connected_ssid"])

    # Append saved profiles that may be nearby but not beaconing immediately
    for prof in sorted(saved_profiles):
        if prof not in seen_ssids and len(networks) < 12:
            networks.append({
                "ssid": prof,
                "auth": "Saved Profile",
                "signal": "Saved",
                "connected": False,
                "saved": True,
            })
            seen_ssids.add(prof)

    # Format protocol JAINOS_WIFI_V2
    resp_lines = [
        "JAINOS_WIFI_V2",
        f"STATE:{iface['state']}",
        f"CONNECTED_SSID:{iface['connected_ssid']}",
        f"SIGNAL:{iface['signal']}",
        f"ADAPTER:{iface['adapter']}",
        f"RADIO:{iface['radio']}",
        f"AUTH:{iface['auth']}",
        "NETWORKS:"
    ]

    for net in networks:
        is_conn = "YES" if net["connected"] else "NO"
        is_saved = "YES" if net["saved"] else "NO"
        resp_lines.append(f"{net['ssid']}|{net['signal']}|{net['auth']}|{is_conn}|{is_saved}")

    resp_lines.append("END")
    return "\n".join(resp_lines) + "\n"

def handle_connect(target_ssid):
    print(f"[*] Attempting Wi-Fi connection to: '{target_ssid}'")
    try:
        res = subprocess.run([NETSH_BIN, "wlan", "connect", f"name={target_ssid}"], capture_output=True, text=True)
        time.sleep(2.0)
        # Verify connection
        info = get_wifi_info()
        if target_ssid in info and "STATE:CONNECTED" in info:
            return "OK:CONNECTED\n"
        elif "completed successfully" in res.stdout:
            return "OK:PENDING\n"
        else:
            return f"FAIL:{res.stdout.strip() or res.stderr.strip() or 'Connection failed'}\n"
    except Exception as e:
        return f"FAIL:{e}\n"

def handle_disconnect():
    print("[*] Disconnecting host Wi-Fi...")
    try:
        subprocess.run([NETSH_BIN, "wlan", "disconnect"], capture_output=True)
        time.sleep(1.0)
        return "OK:DISCONNECTED\n"
    except Exception as e:
        return f"FAIL:{e}\n"

def handle_client(conn, addr):
    try:
        conn.settimeout(4.0)
        req = conn.recv(1024).decode("utf-8", errors="ignore").strip()
        print(f"[+] Request from {addr}: '{req}'")

        if req.startswith("CONNECT"):
            target = req.split(None, 1)[1].strip() if len(req.split()) > 1 else ""
            if target:
                resp = handle_connect(target)
            else:
                resp = "FAIL:Missing SSID\n"
            conn.sendall(resp.encode("utf-8"))
        elif req == "DISCONNECT":
            resp = handle_disconnect()
            conn.sendall(resp.encode("utf-8"))
        else:
            # SCAN or STATUS or default
            resp = get_wifi_info()
            conn.sendall(resp.encode("utf-8"))
    except Exception as e:
        print(f"[-] Client error: {e}")
    finally:
        conn.close()

def main():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

    try:
        server.bind((HOST, PORT))
    except Exception as e:
        print(f"[-] Failed to bind {HOST}:{PORT}: {e}")
        sys.exit(1)

    server.listen(5)
    print("================================================================")
    print("      JainOS Host Wi-Fi Helper Service (v0.3.1)")
    print(f"      Listening on {HOST}:{PORT} (VirtualBox gateway 10.0.2.2)")
    print("      Serving genuine Windows Wi-Fi telemetry to JainOS")
    print("================================================================")

    while True:
        try:
            conn, addr = server.accept()
            t = threading.Thread(target=handle_client, args=(conn, addr), daemon=True)
            t.start()
        except KeyboardInterrupt:
            print("\nShutting down helper.")
            break
        except Exception as e:
            print(f"[-] Accept error: {e}")

if __name__ == "__main__":
    main()
