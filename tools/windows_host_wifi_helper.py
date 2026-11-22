#!/usr/bin/env python3
"""
JainOS Windows Host Wi-Fi Helper Service (v0.3.2)
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
import os
import tempfile

HOST = "0.0.0.0"
PORT = 19820
NETSH_BIN = shutil.which("netsh") or shutil.which("netsh.exe") or "netsh.exe"

def get_wifi_info():
    iface = {
        "adapter": "MediaTek Wi-Fi 6 MT7921 Wireless LAN Card",
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
            if line.startswith("Description") and ":" in line:
                iface["adapter"] = line.split(":", 1)[1].strip()
            elif line.startswith("State") and ":" in line:
                val = line.split(":", 1)[1].strip().lower()
                iface["state"] = "CONNECTED" if "connected" in val else "DISCONNECTED"
            elif line.startswith("SSID") and ":" in line and not line.startswith("SSID 1"):
                parts = line.split(":", 1)
                if parts[0].strip() == "SSID":
                    iface["connected_ssid"] = parts[1].strip()
            elif line.startswith("Signal") and ":" in line:
                iface["signal"] = line.split(":", 1)[1].strip()
            elif line.startswith("Radio type") and ":" in line:
                iface["radio"] = line.split(":", 1)[1].strip()
            elif line.startswith("Authentication") and ":" in line:
                iface["auth"] = line.split(":", 1)[1].strip()
    except Exception as e:
        print(f"[-] Interface query error: {e}")

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
        print(f"[-] Profile query error: {e}")

    # Read visible networks from active beacon scan
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
                if line.startswith("Authentication") and ":" in line:
                    cur_net["auth"] = line.split(":", 1)[1].strip()
                elif line.startswith("Signal") and ":" in line:
                    cur_net["signal"] = line.split(":", 1)[1].strip()
        if cur_net and cur_net["ssid"] and cur_net["ssid"] not in seen_ssids:
            networks.append(cur_net)
            seen_ssids.add(cur_net["ssid"])
    except Exception as e:
        print(f"[-] Networks query error: {e}")

    # Ensure connected network is top of list
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

    # Append saved profiles that may not currently be beaconing
    for prof in sorted(saved_profiles):
        if prof not in seen_ssids and len(networks) < 14:
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

def create_wlan_profile(ssid, password=None):
    """Creates a Windows WLAN profile XML to allow connecting to secured networks."""
    safe_name = ssid.replace('"', '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
    if password:
        safe_pass = password.replace('"', '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
        xml_content = f"""<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>{safe_name}</name>
    <SSIDConfig>
        <SSID>
            <name>{safe_name}</name>
        </SSID>
    </SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>manual</connectionMode>
    <MSM>
        <security>
            <authEncryption>
                <authentication>WPA2PSK</authentication>
                <encryption>AES</encryption>
                <useOneX>false</useOneX>
            </authEncryption>
            <sharedKey>
                <keyType>passPhrase</keyType>
                <protected>false</protected>
                <keyMaterial>{safe_pass}</keyMaterial>
            </sharedKey>
        </security>
    </MSM>
</WLANProfile>"""
    else:
        xml_content = f"""<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>{safe_name}</name>
    <SSIDConfig>
        <SSID>
            <name>{safe_name}</name>
        </SSID>
    </SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>manual</connectionMode>
    <MSM>
        <security>
            <authEncryption>
                <authentication>open</authentication>
                <encryption>none</encryption>
                <useOneX>false</useOneX>
            </authEncryption>
        </security>
    </MSM>
</WLANProfile>"""

    tmp_file = os.path.join(tempfile.gettempdir(), f"jainos_profile_{int(time.time())}.xml")
    try:
        with open(tmp_file, "w", encoding="utf-8") as f:
            f.write(xml_content)
        subprocess.run([NETSH_BIN, "wlan", "add", "profile", f"filename={tmp_file}"], capture_output=True, text=True)
    finally:
        if os.path.exists(tmp_file):
            try:
                os.remove(tmp_file)
            except Exception:
                pass

def handle_connect(target_ssid, password=None):
    print(f"[*] Attempting Wi-Fi connection to: '{target_ssid}' (password={'yes' if password else 'no'})")
    try:
        if password:
            create_wlan_profile(target_ssid, password)

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

def handle_wifi_toggle(enable: bool):
    state_str = "ENABLED" if enable else "DISABLED"
    print(f"[*] Setting host Wi-Fi interface {state_str}...")
    try:
        if not enable:
            handle_disconnect()
        subprocess.run([NETSH_BIN, "interface", "set", "interface", "name=Wi-Fi", f"admin={state_str}"], capture_output=True)
        return "OK:TOGGLED\n"
    except Exception as e:
        return f"FAIL:{e}\n"

def handle_client(conn, addr):
    try:
        conn.settimeout(5.0)
        req = conn.recv(1024).decode("utf-8", errors="ignore").strip()
        print(f"[+] Request from {addr}: '{req}'")

        if req.startswith("CONNECT_KEY "):
            payload = req[len("CONNECT_KEY "):].strip()
            parts = payload.split("|", 1)
            target = parts[0].strip()
            pwd = parts[1].strip() if len(parts) > 1 else None
            resp = handle_connect(target, pwd)
            conn.sendall(resp.encode("utf-8"))
        elif req.startswith("CONNECT "):
            target = req[len("CONNECT "):].strip()
            resp = handle_connect(target, None)
            conn.sendall(resp.encode("utf-8"))
        elif req == "DISCONNECT":
            resp = handle_disconnect()
            conn.sendall(resp.encode("utf-8"))
        elif req == "WIFI_ON":
            resp = handle_wifi_toggle(True)
            conn.sendall(resp.encode("utf-8"))
        elif req == "WIFI_OFF":
            resp = handle_wifi_toggle(False)
            conn.sendall(resp.encode("utf-8"))
        else:
            # SCAN or STATUS
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
    print("      JainOS Host Wi-Fi Helper Service (v0.3.2)")
    print(f"      Listening on {HOST}:{PORT} (Gateway 10.0.2.2)")
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
