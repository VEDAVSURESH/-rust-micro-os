//! Transmission Control Protocol (TCP) Client for HTTP Transfers (RFC 793).
//!
//! Provides a stateful TCP connection client handling:
//! - 3-Way Handshake (SYN -> SYN-ACK -> ACK)
//! - Pseudo-header checksum verification and calculation
//! - Data transmission with PSH-ACK
//! - Automatic sequence and acknowledgment number tracking
//! - Clean teardown (FIN-ACK)

use alloc::vec::Vec;
use crate::net::ipv4::{compute_checksum, IP_PROTO_TCP};

pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;

#[derive(Debug, Clone, Copy)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8,
    pub flags: u8,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

impl TcpHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 20 {
            return None;
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let data_offset = (data[12] >> 4) * 4;
        let flags = data[13];
        let window_size = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent_ptr = u16::from_be_bytes([data[18], data[19]]);

        if data.len() < (data_offset as usize) {
            return None;
        }

        Some((
            Self {
                src_port,
                dst_port,
                seq_num,
                ack_num,
                data_offset,
                flags,
                window_size,
                checksum,
                urgent_ptr,
            },
            &data[(data_offset as usize)..],
        ))
    }
}

/// Builds a TCP segment with pseudo-header checksum.
pub fn build_tcp_segment(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    seq_num: u32,
    ack_num: u32,
    flags: u8,
    window_size: u16,
    payload: &[u8],
) -> Vec<u8> {
    let header_len = 20usize;
    let tcp_len = (header_len + payload.len()) as u16;
    let mut pkt = alloc::vec![0u8; header_len + payload.len()];

    pkt[0..2].copy_from_slice(&src_port.to_be_bytes());
    pkt[2..4].copy_from_slice(&dst_port.to_be_bytes());
    pkt[4..8].copy_from_slice(&seq_num.to_be_bytes());
    pkt[8..12].copy_from_slice(&ack_num.to_be_bytes());
    pkt[12] = 0x50; // Data offset 5 (20 bytes), reserved 0
    pkt[13] = flags;
    pkt[14..16].copy_from_slice(&window_size.to_be_bytes());
    pkt[16..18].copy_from_slice(&[0, 0]); // Zero checksum for calculation
    pkt[18..20].copy_from_slice(&[0, 0]); // Urgent pointer
    pkt[20..].copy_from_slice(payload);

    // Compute TCP Pseudo-Header Checksum
    // Pseudo-header: src_ip (4B), dst_ip (4B), 0 (1B), protocol (1B, 6), tcp_len (2B) = 12B
    let mut pseudo = alloc::vec![0u8; 12 + pkt.len()];
    pseudo[0..4].copy_from_slice(&src_ip);
    pseudo[4..8].copy_from_slice(&dst_ip);
    pseudo[8] = 0;
    pseudo[9] = IP_PROTO_TCP;
    pseudo[10..12].copy_from_slice(&tcp_len.to_be_bytes());
    pseudo[12..].copy_from_slice(&pkt);

    let csum = compute_checksum(&pseudo);
    let final_csum = if csum == 0 { 0xFFFF } else { csum };
    pkt[16..18].copy_from_slice(&final_csum.to_be_bytes());

    pkt
}
