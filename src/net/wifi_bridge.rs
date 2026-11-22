//! RiftNet Host Wi-Fi Bridge Client for JainOS.
//!
//! Connects over virtual TCP to the Windows host helper service on port 19820
//! (default gateway 10.0.2.2) to retrieve genuine host Wi-Fi network telemetry:
//! connected SSID, signal %, host adapter name, and scanned visible SSIDs.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::net::{tcp_request, NET_CONFIG};

#[derive(Debug, Clone)]
pub struct HostWifiTelemetry {
    pub is_available: bool,
    pub state: String,
    pub connected_ssid: String,
    pub signal: String,
    pub adapter: String,
    pub radio: String,
    pub available_networks: Vec<String>,
}

impl HostWifiTelemetry {
    pub fn unavailable() -> Self {
        Self {
            is_available: false,
            state: String::from("DISCONNECTED"),
            connected_ssid: String::from("Host Helper Offline"),
            signal: String::from("0%"),
            adapter: String::from("Windows Netsh Service (10.0.2.2:19820)"),
            radio: String::from("N/A"),
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
            if !text.contains("RIFTNET_WIFI_V1") {
                return HostWifiTelemetry::unavailable();
            }

            let mut info = HostWifiTelemetry {
                is_available: true,
                state: String::from("CONNECTED"),
                connected_ssid: String::from("Unknown"),
                signal: String::from("100%"),
                adapter: String::from("Windows Wireless LAN"),
                radio: String::from("802.11ax"),
                available_networks: Vec::new(),
            };

            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("STATE:") {
                    info.state = String::from(val);
                } else if let Some(val) = trimmed.strip_prefix("SSID:") {
                    info.connected_ssid = String::from(val);
                } else if let Some(val) = trimmed.strip_prefix("SIGNAL:") {
                    info.signal = String::from(val);
                } else if let Some(val) = trimmed.strip_prefix("ADAPTER:") {
                    info.adapter = String::from(val);
                } else if let Some(val) = trimmed.strip_prefix("RADIO:") {
                    info.radio = String::from(val);
                } else if let Some(val) = trimmed.strip_prefix("AVAILABLE:") {
                    if val != "None" && !val.is_empty() {
                        for net_name in val.split(',') {
                            let s = net_name.trim();
                            if !s.is_empty() {
                                info.available_networks.push(String::from(s));
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
    tcp_request(gateway_ip, 19820, cmd.as_bytes(), 200).is_ok()
}
