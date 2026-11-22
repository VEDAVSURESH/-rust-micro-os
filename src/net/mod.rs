//! JainOS Native Networking Stack & Internet Subsystem.
//!
//! Provides bare-metal support for:
//! - Ethernet II framing
//! - Address Resolution Protocol (ARP) & live cache
//! - Internet Protocol v4 (IPv4)
//! - User Datagram Protocol (UDP)
//! - Dynamic Host Configuration Protocol (DHCP) auto-configuration
//! - Domain Name System (DNS) hostname resolution
//! - Internet Control Message Protocol (ICMP) ping
//! - Transmission Control Protocol (TCP) HTTP curl client
//! - Interface diagnostics (ifconfig)

pub mod arp;
pub mod dhcp;
pub mod dns;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod tcp;
pub mod udp;
pub mod wifi_bridge;

use alloc::string::String;
use alloc::vec::Vec;
use crate::net::arp::{send_arp_reply, send_arp_request, ArpPacket, ARP_OP_REQUEST, ARP_TABLE};
use crate::net::ethernet::{EthernetHeader, BROADCAST_MAC, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use crate::net::ipv4::{build_ipv4_packet, Ipv4Header, IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP};
use crate::sync::Spinlock;
use crate::{log_info, log_warn, println, serial_println};

#[derive(Debug, Clone, Copy)]
pub struct NetworkConfig {
    pub mac: [u8; 6],
    pub ip: [u8; 4],
    pub subnet: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [u8; 4],
    pub configured: bool,
}

impl NetworkConfig {
    pub const fn unconfigured() -> Self {
        Self {
            mac: [0; 6],
            ip: [0, 0, 0, 0],
            subnet: [0, 0, 0, 0],
            gateway: [0, 0, 0, 0],
            dns: [0, 0, 0, 0],
            configured: false,
        }
    }
}

pub static NET_CONFIG: Spinlock<NetworkConfig> = Spinlock::new(NetworkConfig::unconfigured());

pub static TCP_RX_QUEUE: Spinlock<Vec<Vec<u8>>> = Spinlock::new(Vec::new());
pub static ICMP_RX_QUEUE: Spinlock<Vec<Vec<u8>>> = Spinlock::new(Vec::new());
pub static DNS_RX_QUEUE: Spinlock<Vec<Vec<u8>>> = Spinlock::new(Vec::new());

/// Polls for incoming network packets and dispatches ARP, ICMP, TCP and UDP.
pub fn poll() {
    while let Some(packet) = crate::e1000::receive_packet() {
        process_incoming_packet(&packet);
    }
}

/// Central packet dispatcher routing incoming Ethernet frames.
fn process_incoming_packet(data: &[u8]) {
    let (eth_hdr, eth_payload) = match EthernetHeader::parse(data) {
        Some(res) => res,
        None => return,
    };

    let cfg = *NET_CONFIG.lock();

    match eth_hdr.ethertype {
        ETHERTYPE_ARP => {
            if let Some(arp) = ArpPacket::parse(eth_payload) {
                // Learn sender IP -> MAC mapping
                ARP_TABLE.lock().insert(arp.sender_ip, arp.sender_mac);

                if arp.opcode == ARP_OP_REQUEST && cfg.configured && arp.target_ip == cfg.ip {
                    // Send ARP reply
                    send_arp_reply(cfg.mac, cfg.ip, arp.sender_mac, arp.sender_ip);
                }
            }
        }
        ETHERTYPE_IPV4 => {
            if let Some((ip_hdr, ip_payload)) = Ipv4Header::parse(eth_payload) {
                if cfg.configured && (ip_hdr.dst_ip == cfg.ip || ip_hdr.dst_ip == [255, 255, 255, 255]) {
                    match ip_hdr.protocol {
                        IP_PROTO_ICMP => {
                            if let Some((echo_hdr, echo_data)) = icmp::IcmpEchoHeader::parse(ip_payload) {
                                if echo_hdr.icmp_type == icmp::ICMP_TYPE_ECHO_REQUEST {
                                    // Reply to incoming ping request
                                    let reply_payload = icmp::build_echo_reply(echo_hdr.identifier, echo_hdr.sequence_number, echo_data);
                                    send_ipv4_packet(ip_hdr.src_ip, IP_PROTO_ICMP, &reply_payload);
                                } else if echo_hdr.icmp_type == icmp::ICMP_TYPE_ECHO_REPLY {
                                    let mut q = ICMP_RX_QUEUE.lock();
                                    if q.len() < 16 {
                                        q.push(alloc::vec::Vec::from(data));
                                    }
                                }
                            }
                        }
                        IP_PROTO_TCP => {
                            let mut q = TCP_RX_QUEUE.lock();
                            if q.len() < 32 {
                                q.push(alloc::vec::Vec::from(data));
                            }
                        }
                        IP_PROTO_UDP => {
                            let mut q = DNS_RX_QUEUE.lock();
                            if q.len() < 16 {
                                q.push(alloc::vec::Vec::from(data));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}

/// Sends an IPv4 packet, automatically resolving the target MAC via ARP.
pub fn send_ipv4_packet(dst_ip: [u8; 4], protocol: u8, payload: &[u8]) -> bool {
    let cfg = *NET_CONFIG.lock();
    if !cfg.configured && protocol != IP_PROTO_UDP {
        return false;
    }

    // Determine next hop IP:
    // If destination is broadcast or 0.0.0.0, use broadcast MAC
    let (target_mac, _next_hop_ip) = if dst_ip == [255, 255, 255, 255] || dst_ip == [0, 0, 0, 0] {
        (BROADCAST_MAC, dst_ip)
    } else {
        // Check if destination is on local subnet
        let is_local = (dst_ip[0] & cfg.subnet[0] == cfg.ip[0] & cfg.subnet[0])
            && (dst_ip[1] & cfg.subnet[1] == cfg.ip[1] & cfg.subnet[1])
            && (dst_ip[2] & cfg.subnet[2] == cfg.ip[2] & cfg.subnet[2])
            && (dst_ip[3] & cfg.subnet[3] == cfg.ip[3] & cfg.subnet[3]);

        // In QEMU/VirtualBox slirp NAT, the DNS server (10.0.2.3) is an internal alias
        // on the same subnet that does not respond to ARP independently; routing to gateway (10.0.2.2)
        // routes correctly to the host resolver. For other local IPs, target them directly.
        let hop_ip = if is_local && dst_ip != cfg.dns {
            dst_ip
        } else {
            cfg.gateway
        };

        // Query ARP cache without holding spinlock across polling / task yields
        let cached_mac = ARP_TABLE.lock().lookup(hop_ip);
        let mac = match cached_mac {
            Some(m) => m,
            None => {
                // Send ARP request and poll for reply
                send_arp_request(cfg.mac, cfg.ip, hop_ip);
                let start_tick = crate::timer::current_ticks();
                let mut found = None;

                while crate::timer::current_ticks().saturating_sub(start_tick) < 30 {
                    poll();
                    if let Some(m) = ARP_TABLE.lock().lookup(hop_ip) {
                        found = Some(m);
                        break;
                    }
                    crate::task::yield_now();
                }

                match found {
                    Some(m) => m,
                    None => {
                        // Fallback: If hop_ip ARP fails, try gateway MAC if known, else broadcast
                        if let Some(gw_mac) = ARP_TABLE.lock().lookup(cfg.gateway) {
                            gw_mac
                        } else {
                            BROADCAST_MAC
                        }
                    }
                }
            }
        };

        (mac, hop_ip)
    };

    let ip_pkt = build_ipv4_packet(cfg.ip, dst_ip, protocol, 0x1234, payload);

    let eth_hdr = EthernetHeader {
        dst_mac: target_mac,
        src_mac: cfg.mac,
        ethertype: ETHERTYPE_IPV4,
    };

    let mut frame = alloc::vec![0u8; 14 + ip_pkt.len()];
    eth_hdr.write_to(&mut frame[0..14]);
    frame[14..].copy_from_slice(&ip_pkt);

    crate::e1000::send_packet(&frame).is_ok()
}

/// Initializes network configuration via DHCP.
pub fn init() -> bool {
    let mac = match crate::e1000::mac_address() {
        Some(m) => m,
        None => return false,
    };

    {
        let mut cfg = NET_CONFIG.lock();
        cfg.mac = mac;
    }

    log_info!("Initiating DHCP auto-configuration for eth0...");

    let xid = 0x3910_2800u32;
    let discover_msg = dhcp::build_dhcp_discover(xid, mac);
    let udp_discover = udp::build_udp_packet(
        [0, 0, 0, 0],
        [255, 255, 255, 255],
        dhcp::DHCP_CLIENT_PORT,
        dhcp::DHCP_SERVER_PORT,
        &discover_msg,
    );

    // Send DHCP Discover broadcast
    let eth_hdr = EthernetHeader {
        dst_mac: BROADCAST_MAC,
        src_mac: mac,
        ethertype: ETHERTYPE_IPV4,
    };
    let ip_pkt = build_ipv4_packet([0, 0, 0, 0], [255, 255, 255, 255], IP_PROTO_UDP, 0x0001, &udp_discover);
    let mut frame = alloc::vec![0u8; 14 + ip_pkt.len()];
    eth_hdr.write_to(&mut frame[0..14]);
    frame[14..].copy_from_slice(&ip_pkt);
    let _ = crate::e1000::send_packet(&frame);

    // Wait for DHCP Offer
    let mut lease = None;
    let start_tick = crate::timer::current_ticks();

    while crate::timer::current_ticks().saturating_sub(start_tick) < 200 {
        if let Some(rx_data) = crate::e1000::receive_packet() {
            if let Some((eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                if eth.ethertype == ETHERTYPE_IPV4 {
                    if let Some((_ip, ip_payload)) = Ipv4Header::parse(eth_payload) {
                        if let Some((udp_h, udp_payload)) = udp::UdpHeader::parse(ip_payload) {
                            if udp_h.dst_port == dhcp::DHCP_CLIENT_PORT {
                                if let Some((msg_type, l)) = dhcp::parse_dhcp_response(udp_payload, xid) {
                                    if msg_type == dhcp::DHCP_MSG_OFFER {
                                        lease = Some(l);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        crate::task::yield_now();
    }

    let offered = match lease {
        Some(l) => l,
        None => {
            log_warn!("DHCP Discover timed out (using fallback static IP 10.0.2.15).");
            // Fallback to standard VirtualBox/QEMU NAT parameters
            let mut cfg = NET_CONFIG.lock();
            cfg.ip = [10, 0, 2, 15];
            cfg.subnet = [255, 255, 255, 0];
            cfg.gateway = [10, 0, 2, 2];
            cfg.dns = [10, 0, 2, 3];
            cfg.configured = true;
            return true;
        }
    };

    // Send DHCP Request
    let request_msg = dhcp::build_dhcp_request(xid, mac, offered.client_ip, offered.server_ip);
    let udp_request = udp::build_udp_packet(
        [0, 0, 0, 0],
        [255, 255, 255, 255],
        dhcp::DHCP_CLIENT_PORT,
        dhcp::DHCP_SERVER_PORT,
        &request_msg,
    );
    let ip_req = build_ipv4_packet([0, 0, 0, 0], [255, 255, 255, 255], IP_PROTO_UDP, 0x0002, &udp_request);
    let mut req_frame = alloc::vec![0u8; 14 + ip_req.len()];
    eth_hdr.write_to(&mut req_frame[0..14]);
    req_frame[14..].copy_from_slice(&ip_req);
    let _ = crate::e1000::send_packet(&req_frame);

    // Wait for DHCP Ack
    let mut ack_received = false;
    let req_tick = crate::timer::current_ticks();

    while crate::timer::current_ticks().saturating_sub(req_tick) < 200 {
        if let Some(rx_data) = crate::e1000::receive_packet() {
            if let Some((eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                if eth.ethertype == ETHERTYPE_IPV4 {
                    if let Some((_ip, ip_payload)) = Ipv4Header::parse(eth_payload) {
                        if let Some((udp_h, udp_payload)) = udp::UdpHeader::parse(ip_payload) {
                            if udp_h.dst_port == dhcp::DHCP_CLIENT_PORT {
                                if let Some((msg_type, _)) = dhcp::parse_dhcp_response(udp_payload, xid) {
                                    if msg_type == dhcp::DHCP_MSG_ACK {
                                        ack_received = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        crate::task::yield_now();
    }

    if !ack_received {
        log_warn!("DHCP ACK wait elapsed, applying offered lease parameters.");
    }

    let mut cfg = NET_CONFIG.lock();
    cfg.ip = offered.client_ip;
    cfg.subnet = offered.subnet_mask;
    cfg.gateway = offered.router;
    cfg.dns = offered.dns_server;
    cfg.configured = true;

    log_info!(
        "DHCP Lease Acquired: IP={}.{}.{}.{} Subnet={}.{}.{}.{} Gateway={}.{}.{}.{} DNS={}.{}.{}.{}",
        cfg.ip[0], cfg.ip[1], cfg.ip[2], cfg.ip[3],
        cfg.subnet[0], cfg.subnet[1], cfg.subnet[2], cfg.subnet[3],
        cfg.gateway[0], cfg.gateway[1], cfg.gateway[2], cfg.gateway[3],
        cfg.dns[0], cfg.dns[1], cfg.dns[2], cfg.dns[3]
    );

    // Pre-seed ARP table with Gateway
    send_arp_request(cfg.mac, cfg.ip, cfg.gateway);

    true
}

/// Parses an IPv4 address string like "10.0.2.15" into `[u8; 4]`.
pub fn parse_ipv4_addr(s: &str) -> Option<[u8; 4]> {
    let mut parts = s.split('.');
    let a: u8 = parts.next()?.parse().ok()?;
    let b: u8 = parts.next()?.parse().ok()?;
    let c: u8 = parts.next()?.parse().ok()?;
    let d: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_none() {
        Some([a, b, c, d])
    } else {
        None
    }
}

/// Resolves a domain name or IPv4 address string to an IPv4 address.
pub fn resolve_host(host: &str) -> Option<[u8; 4]> {
    if let Some(ip) = parse_ipv4_addr(host) {
        return Some(ip);
    }

    let cfg = *NET_CONFIG.lock();
    if !cfg.configured {
        return None;
    }

    // Dynamic transaction ID to distinguish distinct requests
    let tx_id = 0x5000u16 ^ (crate::timer::current_ticks() as u16 & 0x0FFF);
    let query_payload = dns::build_dns_query(tx_id, host);
    let src_port = 49152 + (crate::timer::current_ticks() as u16 % 10000);
    let udp_query = udp::build_udp_packet(cfg.ip, cfg.dns, src_port, dns::DNS_PORT, &query_payload);

    // Attempt DNS query with 2 tries (1.5 seconds per try)
    for _attempt in 0..2 {
        // Clear DNS RX queue BEFORE transmitting to receive only the new reply
        DNS_RX_QUEUE.lock().clear();

        if !send_ipv4_packet(cfg.dns, IP_PROTO_UDP, &udp_query) {
            continue;
        }

        let start_tick = crate::timer::current_ticks();
        while crate::timer::current_ticks().saturating_sub(start_tick) < 150 {
            poll();
            let pkt_opt = {
                let mut q = DNS_RX_QUEUE.lock();
                if q.is_empty() {
                    None
                } else {
                    Some(q.remove(0))
                }
            };

            if let Some(rx_data) = pkt_opt {
                if let Some((_eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                    if let Some((_ip, ip_payload)) = Ipv4Header::parse(eth_payload) {
                        if let Some((udp_h, udp_payload)) = udp::UdpHeader::parse(ip_payload) {
                            if udp_h.src_port == dns::DNS_PORT && udp_h.dst_port == src_port {
                                if let Some(resolved_ip) = dns::parse_dns_response(udp_payload, tx_id) {
                                    return Some(resolved_ip);
                                }
                            }
                        }
                    }
                }
            }
            crate::task::yield_now();
        }
    }

    None
}

/// Implementation of the `ifconfig` terminal command.
pub fn print_ifconfig() {
    let cfg = *NET_CONFIG.lock();
    let dev_opt = crate::e1000::E1000.lock();

    println!("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500");
    serial_println!("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500");

    if cfg.configured {
        println!(
            "        inet {}.{}.{}.{}  netmask {}.{}.{}.{}  broadcast 255.255.255.255",
            cfg.ip[0], cfg.ip[1], cfg.ip[2], cfg.ip[3],
            cfg.subnet[0], cfg.subnet[1], cfg.subnet[2], cfg.subnet[3]
        );
        serial_println!(
            "        inet {}.{}.{}.{}  netmask {}.{}.{}.{}  broadcast 255.255.255.255",
            cfg.ip[0], cfg.ip[1], cfg.ip[2], cfg.ip[3],
            cfg.subnet[0], cfg.subnet[1], cfg.subnet[2], cfg.subnet[3]
        );
        println!(
            "        gateway {}.{}.{}.{}  dns {}.{}.{}.{}",
            cfg.gateway[0], cfg.gateway[1], cfg.gateway[2], cfg.gateway[3],
            cfg.dns[0], cfg.dns[1], cfg.dns[2], cfg.dns[3]
        );
        serial_println!(
            "        gateway {}.{}.{}.{}  dns {}.{}.{}.{}",
            cfg.gateway[0], cfg.gateway[1], cfg.gateway[2], cfg.gateway[3],
            cfg.dns[0], cfg.dns[1], cfg.dns[2], cfg.dns[3]
        );
    } else {
        println!("        inet unconfigured (DHCP pending)");
        serial_println!("        inet unconfigured (DHCP pending)");
    }

    println!(
        "        ether {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  txqueuelen 1000  (Ethernet)",
        cfg.mac[0], cfg.mac[1], cfg.mac[2], cfg.mac[3], cfg.mac[4], cfg.mac[5]
    );
    serial_println!(
        "        ether {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  txqueuelen 1000  (Ethernet)",
        cfg.mac[0], cfg.mac[1], cfg.mac[2], cfg.mac[3], cfg.mac[4], cfg.mac[5]
    );

    if let Some(ref dev) = *dev_opt {
        println!(
            "        RX packets {}  bytes {} | TX packets {}  bytes {}",
            dev.rx_packets, dev.rx_bytes, dev.tx_packets, dev.tx_bytes
        );
        serial_println!(
            "        RX packets {}  bytes {} | TX packets {}  bytes {}",
            dev.rx_packets, dev.rx_bytes, dev.tx_packets, dev.tx_bytes
        );
    }
}

/// Implementation of the `ping` terminal command.
pub fn ping(target_str: &str) {
    let target_ip = match resolve_host(target_str) {
        Some(ip) => ip,
        None => {
            println!("ping: unknown host: {}", target_str);
            serial_println!("ping: unknown host: {}", target_str);
            return;
        }
    };

    println!(
        "PING {} ({}.{}.{}.{}): 56 data bytes",
        target_str, target_ip[0], target_ip[1], target_ip[2], target_ip[3]
    );
    serial_println!(
        "PING {} ({}.{}.{}.{}): 56 data bytes",
        target_str, target_ip[0], target_ip[1], target_ip[2], target_ip[3]
    );

    let mut transmitted = 0;
    let mut received = 0;

    for seq in 1..=4 {
        let echo_pkt = icmp::build_echo_request(0x1234, seq, b"JainOS Ping Payload Probe 2026");
        let start_ticks = crate::timer::current_ticks();

        if send_ipv4_packet(target_ip, IP_PROTO_ICMP, &echo_pkt) {
            transmitted += 1;
            let mut got_reply = false;

            while crate::timer::current_ticks().saturating_sub(start_ticks) < 100 {
                poll();
                let pkt_opt = {
                    let mut q = ICMP_RX_QUEUE.lock();
                    if q.is_empty() { None } else { Some(q.remove(0)) }
                };

                if let Some(rx_data) = pkt_opt {
                    if let Some((_eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                        if let Some((ip_hdr, ip_payload)) = Ipv4Header::parse(eth_payload) {
                            if ip_hdr.src_ip == target_ip && ip_hdr.protocol == IP_PROTO_ICMP {
                                if let Some((echo_h, _)) = icmp::IcmpEchoHeader::parse(ip_payload) {
                                    if echo_h.icmp_type == icmp::ICMP_TYPE_ECHO_REPLY && echo_h.sequence_number == seq {
                                        let rtt_ms = (crate::timer::current_ticks().saturating_sub(start_ticks)) * 10;
                                        println!(
                                            "64 bytes from {}.{}.{}.{}: icmp_seq={} ttl={} time={} ms",
                                            target_ip[0], target_ip[1], target_ip[2], target_ip[3],
                                            seq, ip_hdr.ttl, rtt_ms
                                        );
                                        serial_println!(
                                            "64 bytes from {}.{}.{}.{}: icmp_seq={} ttl={} time={} ms",
                                            target_ip[0], target_ip[1], target_ip[2], target_ip[3],
                                            seq, ip_hdr.ttl, rtt_ms
                                        );
                                        received += 1;
                                        got_reply = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                crate::task::yield_now();
            }

            if !got_reply {
                println!("Request timeout for icmp_seq {}", seq);
                serial_println!("Request timeout for icmp_seq {}", seq);
            }
        }

        // Wait 50 ticks (500ms) between pings
        let sleep_until = crate::timer::current_ticks() + 50;
        while crate::timer::current_ticks() < sleep_until {
            poll();
            crate::task::yield_now();
        }
    }

    println!("--- {} ping statistics ---", target_str);
    serial_println!("--- {} ping statistics ---", target_str);
    let loss = if transmitted > 0 {
        ((transmitted - received) * 100) / transmitted
    } else {
        100
    };
    println!(
        "{} packets transmitted, {} packets received, {}% packet loss",
        transmitted, received, loss
    );
    serial_println!(
        "{} packets transmitted, {} packets received, {}% packet loss",
        transmitted, received, loss
    );
}

/// Sends arbitrary data over a single-shot TCP connection and collects response bytes.
pub fn tcp_request(target_ip: [u8; 4], port: u16, payload: &[u8], timeout_ticks: u64) -> Result<Vec<u8>, &'static str> {
    let cfg = *NET_CONFIG.lock();
    if !cfg.configured {
        return Err("Network unconfigured");
    }

    let src_port = 49152 + (crate::timer::current_ticks() as u16 % 10000);
    let mut my_seq = 1000u32;
    let mut my_ack = 0u32;

    TCP_RX_QUEUE.lock().clear();

    // 1. Send TCP SYN
    let syn_pkt = tcp::build_tcp_segment(
        cfg.ip,
        target_ip,
        src_port,
        port,
        my_seq,
        0,
        tcp::TCP_FLAG_SYN,
        64240,
        &[],
    );
    if !send_ipv4_packet(target_ip, IP_PROTO_TCP, &syn_pkt) {
        return Err("Failed to transmit SYN packet");
    }
    my_seq += 1;

    // 2. Wait for SYN-ACK
    let mut connected = false;
    let start_tick = crate::timer::current_ticks();
    let syn_timeout = timeout_ticks.min(250).max(50);

    while crate::timer::current_ticks().saturating_sub(start_tick) < syn_timeout {
        poll();
        let pkt_opt = {
            let mut q = TCP_RX_QUEUE.lock();
            if q.is_empty() { None } else { Some(q.remove(0)) }
        };

        if let Some(rx_data) = pkt_opt {
            if let Some((_eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                if let Some((ip_hdr, ip_payload)) = Ipv4Header::parse(eth_payload) {
                    if ip_hdr.src_ip == target_ip && ip_hdr.protocol == IP_PROTO_TCP {
                        if let Some((tcp_h, _)) = tcp::TcpHeader::parse(ip_payload) {
                            if tcp_h.dst_port == src_port && (tcp_h.flags & (tcp::TCP_FLAG_SYN | tcp::TCP_FLAG_ACK)) == (tcp::TCP_FLAG_SYN | tcp::TCP_FLAG_ACK) {
                                my_ack = tcp_h.seq_num + 1;
                                connected = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        crate::task::yield_now();
    }

    if !connected {
        return Err("TCP connection timed out");
    }

    // 3. Send TCP ACK
    let ack_pkt = tcp::build_tcp_segment(
        cfg.ip,
        target_ip,
        src_port,
        port,
        my_seq,
        my_ack,
        tcp::TCP_FLAG_ACK,
        64240,
        &[],
    );
    send_ipv4_packet(target_ip, IP_PROTO_TCP, &ack_pkt);

    // 4. Send Payload if present
    if !payload.is_empty() {
        let psh_pkt = tcp::build_tcp_segment(
            cfg.ip,
            target_ip,
            src_port,
            port,
            my_seq,
            my_ack,
            tcp::TCP_FLAG_PSH | tcp::TCP_FLAG_ACK,
            64240,
            payload,
        );
        send_ipv4_packet(target_ip, IP_PROTO_TCP, &psh_pkt);
        my_seq += payload.len() as u32;
    }

    // 5. Receive Response
    let mut response_body = Vec::new();
    let rx_start = crate::timer::current_ticks();

    while crate::timer::current_ticks().saturating_sub(rx_start) < timeout_ticks {
        poll();
        let pkt_opt = {
            let mut q = TCP_RX_QUEUE.lock();
            if q.is_empty() { None } else { Some(q.remove(0)) }
        };

        if let Some(rx_data) = pkt_opt {
            if let Some((_eth, eth_payload)) = EthernetHeader::parse(&rx_data) {
                if let Some((ip_hdr, ip_payload)) = Ipv4Header::parse(eth_payload) {
                    if ip_hdr.src_ip == target_ip && ip_hdr.protocol == IP_PROTO_TCP {
                        if let Some((tcp_h, tcp_payload)) = tcp::TcpHeader::parse(ip_payload) {
                            if tcp_h.dst_port == src_port {
                                if !tcp_payload.is_empty() {
                                    response_body.extend_from_slice(tcp_payload);
                                    my_ack = tcp_h.seq_num + tcp_payload.len() as u32;

                                    // Send ACK for data
                                    let data_ack = tcp::build_tcp_segment(
                                        cfg.ip,
                                        target_ip,
                                        src_port,
                                        port,
                                        my_seq,
                                        my_ack,
                                        tcp::TCP_FLAG_ACK,
                                        64240,
                                        &[],
                                    );
                                    send_ipv4_packet(target_ip, IP_PROTO_TCP, &data_ack);
                                }

                                if (tcp_h.flags & tcp::TCP_FLAG_FIN) != 0 {
                                    my_ack = tcp_h.seq_num + 1;
                                    // Send FIN-ACK
                                    let fin_ack = tcp::build_tcp_segment(
                                        cfg.ip,
                                        target_ip,
                                        src_port,
                                        port,
                                        my_seq,
                                        my_ack,
                                        tcp::TCP_FLAG_FIN | tcp::TCP_FLAG_ACK,
                                        64240,
                                        &[],
                                    );
                                    send_ipv4_packet(target_ip, IP_PROTO_TCP, &fin_ack);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        crate::task::yield_now();
    }

    Ok(response_body)
}

/// Performs an HTTP GET request and returns (status_code, headers, body).
pub fn http_get(url: &str) -> Result<(u16, String, String), String> {
    let raw = url.trim();
    let path_start = raw.find("://").map(|i| i + 3).unwrap_or(0);
    let host_and_path = &raw[path_start..];

    let (host, path) = match host_and_path.find('/') {
        Some(idx) => (&host_and_path[..idx], &host_and_path[idx..]),
        None => (host_and_path, "/"),
    };

    let (host_clean, port) = match host.find(':') {
        Some(idx) => (&host[..idx], host[idx + 1..].parse::<u16>().unwrap_or(80)),
        None => (host, 80),
    };

    let target_ip = resolve_host(host_clean)
        .ok_or_else(|| alloc::format!("Could not resolve host: {}", host_clean))?;

    let req_str = alloc::format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: JainOS/0.3.0 (Rust-BareMetal)\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, host_clean
    );

    let raw_resp = tcp_request(target_ip, port, req_str.as_bytes(), 400)
        .map_err(|e| alloc::format!("TCP error: {}", e))?;

    if raw_resp.is_empty() {
        return Err(String::from("Empty reply from server"));
    }

    let resp_str = String::from_utf8_lossy(&raw_resp);

    // Split headers and body
    let (head_part, body_part) = if let Some(pos) = resp_str.find("\r\n\r\n") {
        (&resp_str[..pos], &resp_str[pos + 4..])
    } else if let Some(pos) = resp_str.find("\n\n") {
        (&resp_str[..pos], &resp_str[pos + 2..])
    } else {
        (resp_str.as_ref(), "")
    };

    let status_code = head_part
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            parts.next()?; // HTTP/1.1
            parts.next()?.parse::<u16>().ok()
        })
        .unwrap_or(200);

    Ok((status_code, String::from(head_part), String::from(body_part)))
}

/// Implementation of the `curl` terminal command.
pub fn curl(url: &str) {
    let raw = url.trim();
    let path_start = raw.find("://").map(|i| i + 3).unwrap_or(0);
    let host_and_path = &raw[path_start..];

    let (host, _path) = match host_and_path.find('/') {
        Some(idx) => (&host_and_path[..idx], &host_and_path[idx..]),
        None => (host_and_path, "/"),
    };

    let (host_clean, port) = match host.find(':') {
        Some(idx) => (&host[..idx], host[idx + 1..].parse::<u16>().unwrap_or(80)),
        None => (host, 80),
    };

    println!("Connecting to {} (port {})...", host_clean, port);
    serial_println!("Connecting to {} (port {})...", host_clean, port);

    let target_ip = match resolve_host(host_clean) {
        Some(ip) => ip,
        None => {
            println!("curl: Could not resolve host: {}", host_clean);
            serial_println!("curl: Could not resolve host: {}", host_clean);
            return;
        }
    };

    println!(
        "Connecting to {}.{}.{}.{}:{}...",
        target_ip[0], target_ip[1], target_ip[2], target_ip[3], port
    );
    serial_println!(
        "Connecting to {}.{}.{}.{}:{}...",
        target_ip[0], target_ip[1], target_ip[2], target_ip[3], port
    );

    match http_get(url) {
        Ok((_status, headers, body)) => {
            println!("{}\r\n\r\n{}", headers, body);
            serial_println!("{}\r\n\r\n{}", headers, body);
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("curl: Failed to connect to host (connection timed out)");
                serial_println!("curl: Failed to connect to host (connection timed out)");
            } else if e.contains("Empty reply") {
                println!("curl: Empty reply from server");
                serial_println!("curl: Empty reply from server");
            } else {
                println!("curl: {}", e);
                serial_println!("curl: {}", e);
            }
        }
    }
}
