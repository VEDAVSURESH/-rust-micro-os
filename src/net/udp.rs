//! User Datagram Protocol (UDP) Packet Handling & Pseudo-Header Checksum.

use alloc::vec::Vec;
use crate::net::ipv4::{compute_checksum, IP_PROTO_UDP};

#[derive(Debug, Clone, Copy)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 8 {
            return None;
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let checksum = u16::from_be_bytes([data[6], data[7]]);

        let payload_len = (length as usize).saturating_sub(8);
        let payload_end = core::cmp::min(8 + payload_len, data.len());

        Some((
            Self {
                src_port,
                dst_port,
                length,
                checksum,
            },
            &data[8..payload_end],
        ))
    }
}

/// Builds a UDP packet including pseudo-header checksum.
pub fn build_udp_packet(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let udp_len = (8 + payload.len()) as u16;
    let mut pkt = alloc::vec![0u8; 8 + payload.len()];

    pkt[0..2].copy_from_slice(&src_port.to_be_bytes());
    pkt[2..4].copy_from_slice(&dst_port.to_be_bytes());
    pkt[4..6].copy_from_slice(&udp_len.to_be_bytes());
    pkt[6..8].copy_from_slice(&[0, 0]); // Zero checksum for calculation
    pkt[8..].copy_from_slice(payload);

    // Compute IPv4 UDP Pseudo-Header Checksum
    // Pseudo-header: src_ip (4B), dst_ip (4B), zero (1B), protocol (1B), udp_len (2B) = 12B
    let mut pseudo = alloc::vec![0u8; 12 + pkt.len()];
    pseudo[0..4].copy_from_slice(&src_ip);
    pseudo[4..8].copy_from_slice(&dst_ip);
    pseudo[8] = 0;
    pseudo[9] = IP_PROTO_UDP;
    pseudo[10..12].copy_from_slice(&udp_len.to_be_bytes());
    pseudo[12..].copy_from_slice(&pkt);

    let csum = compute_checksum(&pseudo);
    let final_csum = if csum == 0 { 0xFFFF } else { csum };
    pkt[6..8].copy_from_slice(&final_csum.to_be_bytes());

    pkt
}
