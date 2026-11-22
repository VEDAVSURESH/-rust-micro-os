//! Host Wi-Fi Bridge Client for JainOS.
//!
//! Connects over virtual TCP to the Windows host helper service on port 19820
//! (default gateway 10.0.2.2) to retrieve genuine host Wi-Fi network telemetry:
//! connected SSID, signal %, host adapter name, security mode, and available networks list.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::net::{tcp_request, NET_CONFIG};

#[derive(Debug, Clone)]
pub struct AvailableWifiNetwork {
    pub ssid: String,
    pub signal: String,
    pub security: String,
    pub is_connected: bool,
    pub is_saved: bool,
}

#[derive(Debug, Clone)]
pub struct HostWifiTelemetry {
    pub is_available: bool,
    pub state: String,
    pub connected_ssid: String,
    pub signal: String,
    pub adapter: String,
    pub radio: String,
    pub auth: String,
    pub available_networks: Vec<AvailableWifiNetwork>,
}

impl HostWifiTelemetry {
    pub fn unavailable() -> Self {
        Self {
            is_available: false,
            state: String::from("DISCONNECTED"),
            connected_ssid: String::from("Host Helper Offline"),
            signal: String::from("0%"),
            adapter: String::from("Windows WLAN Service (10.0.2.2:19820)"),
            radio: String::from("N/A"),
            auth: String::from("N/A"),
            available_networks: Vec::new(),
        }
    }
}

/// Queries the Windows host Wi-Fi helper running on gateway 10.0.2.2:19820.
pub fn query_host_wifi() -> HostWifiTelemetry {
    let gateway_ip = {
        let cfg = NET_CONFIG.lock();
        if cfg.configured {
            cfg.gateway
        } else {
            [10, 0, 2, 2]
        }
    };

    // Query helper service via TCP port 19820
    match tcp_request(gateway_ip, 19820, b"SCAN\n", 150) {
        Ok(data) => {
            let text = String::from_utf8_lossy(&data);
            if !text.contains("JAINOS_WIFI_V2") && !text.contains("RIFTNET_WIFI_V1") {
                return HostWifiTelemetry::unavailable();
            }

            let mut info = HostWifiTelemetry {
                is_available: true,
                state: String::from("CONNECTED"),
                connected_ssid: String::from("Unknown"),
                signal: String::from("100%"),
                adapter: String::from("Windows Wireless LAN"),
                radio: String::from("802.11ax"),
                auth: String::from("WPA2-Personal"),
                available_networks: Vec::new(),
            };

            let mut in_networks = false;

            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed == "NETWORKS:" {
                    in_networks = true;
                    continue;
                }
                if trimmed == "END" {
                    break;
                }

                if in_networks {
                    // Format: SSID|SIGNAL|AUTH|IS_CONNECTED|IS_SAVED
                    let parts: Vec<&str> = trimmed.split('|').collect();
                    if parts.len() >= 5 {
                        info.available_networks.push(AvailableWifiNetwork {
                            ssid: String::from(parts[0]),
                            signal: String::from(parts[1]),
                            security: String::from(parts[2]),
                            is_connected: parts[3] == "YES",
                            is_saved: parts[4] == "YES",
                        });
                    } else if !trimmed.is_empty() {
                        // Fallback for simple comma or plain string
                        info.available_networks.push(AvailableWifiNetwork {
                            ssid: String::from(trimmed),
                            signal: String::from("70%"),
                            security: String::from("WPA2"),
                            is_connected: false,
                            is_saved: false,
                        });
                    }
                } else {
                    if let Some(val) = trimmed.strip_prefix("STATE:") {
                        info.state = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("CONNECTED_SSID:") {
                        info.connected_ssid = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("SSID:") {
                        info.connected_ssid = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("SIGNAL:") {
                        info.signal = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("ADAPTER:") {
                        info.adapter = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("RADIO:") {
                        info.radio = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("AUTH:") {
                        info.auth = String::from(val);
                    } else if let Some(val) = trimmed.strip_prefix("AVAILABLE:") {
                        if val != "None" && !val.is_empty() {
                            for net_name in val.split(',') {
                                let s = net_name.trim();
                                if !s.is_empty() {
                                    info.available_networks.push(AvailableWifiNetwork {
                                        ssid: String::from(s),
                                        signal: String::from("80%"),
                                        security: String::from("WPA2"),
                                        is_connected: s == info.connected_ssid,
                                        is_saved: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            info
        }
        Err(_) => HostWifiTelemetry::unavailable(),
    }
}

/// Requests the Windows host helper to connect to a specific Wi-Fi SSID.
pub fn request_connect_wifi(ssid: &str) -> bool {
    let gateway_ip = {
        let cfg = NET_CONFIG.lock();
        if cfg.configured {
            cfg.gateway
        } else {
            [10, 0, 2, 2]
        }
    };

    let cmd = format!("CONNECT {}\n", ssid);
    match tcp_request(gateway_ip, 19820, cmd.as_bytes(), 250) {
        Ok(data) => {
            let resp = String::from_utf8_lossy(&data);
            resp.contains("OK")
        }
        Err(_) => false,
    }
}

/// Requests the Windows host helper to disconnect from Wi-Fi.
pub fn request_disconnect_wifi() -> bool {
    let gateway_ip = {
        let cfg = NET_CONFIG.lock();
        if cfg.configured {
            cfg.gateway
        } else {
            [10, 0, 2, 2]
        }
    };

    match tcp_request(gateway_ip, 19820, b"DISCONNECT\n", 200) {
        Ok(data) => {
            let resp = String::from_utf8_lossy(&data);
            resp.contains("OK")
        }
        Err(_) => false,
    }
}
