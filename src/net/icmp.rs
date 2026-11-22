//! Internet Control Message Protocol (ICMP) for Echo Requests & Replies (Ping).

use alloc::vec::Vec;
use crate::net::ipv4::compute_checksum;

pub const ICMP_TYPE_ECHO_REPLY: u8 = 0;
pub const ICMP_TYPE_ECHO_REQUEST: u8 = 8;

#[derive(Debug, Clone)]
pub struct IcmpEchoHeader {
    pub icmp_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence_number: u16,
}

impl IcmpEchoHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 8 {
            return None;
        }

        let icmp_type = data[0];
        let code = data[1];
        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let identifier = u16::from_be_bytes([data[4], data[5]]);
        let sequence_number = u16::from_be_bytes([data[6], data[7]]);

        Some((
            Self {
                icmp_type,
                code,
                checksum,
                identifier,
                sequence_number,
            },
            &data[8..],
        ))
    }
}

/// Builds an ICMP Echo Request packet.
pub fn build_echo_request(identifier: u16, seq_num: u16, payload: &[u8]) -> Vec<u8> {
    let mut pkt = alloc::vec![0u8; 8 + payload.len()];
    pkt[0] = ICMP_TYPE_ECHO_REQUEST;
    pkt[1] = 0; // Code
    pkt[2..4].copy_from_slice(&[0, 0]); // Zero checksum
    pkt[4..6].copy_from_slice(&identifier.to_be_bytes());
    pkt[6..8].copy_from_slice(&seq_num.to_be_bytes());
    pkt[8..].copy_from_slice(payload);

    let csum = compute_checksum(&pkt);
    pkt[2..4].copy_from_slice(&csum.to_be_bytes());

    pkt
}

/// Builds an ICMP Echo Reply responding to a request.
pub fn build_echo_reply(identifier: u16, seq_num: u16, payload: &[u8]) -> Vec<u8> {
    let mut pkt = alloc::vec![0u8; 8 + payload.len()];
    pkt[0] = ICMP_TYPE_ECHO_REPLY;
    pkt[1] = 0; // Code
    pkt[2..4].copy_from_slice(&[0, 0]); // Zero checksum
    pkt[4..6].copy_from_slice(&identifier.to_be_bytes());
    pkt[6..8].copy_from_slice(&seq_num.to_be_bytes());
    pkt[8..].copy_from_slice(payload);

    let csum = compute_checksum(&pkt);
    pkt[2..4].copy_from_slice(&csum.to_be_bytes());

    pkt
}
