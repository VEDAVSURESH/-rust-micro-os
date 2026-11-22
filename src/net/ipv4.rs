//! Internet Protocol Version 4 (IPv4) Packet Handling & Checksum Calculation.

use alloc::vec::Vec;

pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags_fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub header_checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

impl Ipv4Header {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 20 {
            return None;
        }

        let version_ihl = data[0];
        let ihl = (version_ihl & 0x0F) as usize * 4;
        if data.len() < ihl || ihl < 20 {
            return None;
        }

        let dscp_ecn = data[1];
        let total_length = u16::from_be_bytes([data[2], data[3]]);
        let identification = u16::from_be_bytes([data[4], data[5]]);
        let flags_fragment_offset = u16::from_be_bytes([data[6], data[7]]);
        let ttl = data[8];
        let protocol = data[9];
        let header_checksum = u16::from_be_bytes([data[10], data[11]]);

        let mut src_ip = [0u8; 4];
        src_ip.copy_from_slice(&data[12..16]);
        let mut dst_ip = [0u8; 4];
        dst_ip.copy_from_slice(&data[16..20]);

        let payload_len = (total_length as usize).saturating_sub(ihl);
        let payload_end = core::cmp::min(ihl + payload_len, data.len());

        Some((
            Self {
                version_ihl,
                dscp_ecn,
                total_length,
                identification,
                flags_fragment_offset,
                ttl,
                protocol,
                header_checksum,
                src_ip,
                dst_ip,
            },
            &data[ihl..payload_end],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) {
        out[0] = self.version_ihl;
        out[1] = self.dscp_ecn;
        out[2..4].copy_from_slice(&self.total_length.to_be_bytes());
        out[4..6].copy_from_slice(&self.identification.to_be_bytes());
        out[6..8].copy_from_slice(&self.flags_fragment_offset.to_be_bytes());
        out[8] = self.ttl;
        out[9] = self.protocol;
        out[10..12].copy_from_slice(&self.header_checksum.to_be_bytes());
        out[12..16].copy_from_slice(&self.src_ip);
        out[16..20].copy_from_slice(&self.dst_ip);
    }
}

/// Computes the standard Internet 16-bit one's complement checksum (RFC 1071).
pub fn compute_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        sum = sum.wrapping_add(word);
        i += 2;
    }

    if i < data.len() {
        let word = u16::from_be_bytes([data[i], 0]) as u32;
        sum = sum.wrapping_add(word);
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Builds an IPv4 packet encapsulating `payload`.
pub fn build_ipv4_packet(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    protocol: u8,
    identification: u16,
    payload: &[u8],
) -> Vec<u8> {
    let total_len = (20 + payload.len()) as u16;
    let mut pkt = alloc::vec![0u8; 20 + payload.len()];

    pkt[0] = 0x45; // Version 4, IHL 5 (20 bytes)
    pkt[1] = 0x00; // DSCP/ECN
    pkt[2..4].copy_from_slice(&total_len.to_be_bytes());
    pkt[4..6].copy_from_slice(&identification.to_be_bytes());
    pkt[6..8].copy_from_slice(&0x4000u16.to_be_bytes()); // Flags: Don't Fragment
    pkt[8] = 64;   // TTL
    pkt[9] = protocol;
    pkt[10..12].copy_from_slice(&[0, 0]); // Zero checksum for calculation
    pkt[12..16].copy_from_slice(&src_ip);
    pkt[16..20].copy_from_slice(&dst_ip);

    // Compute checksum over 20-byte header
    let checksum = compute_checksum(&pkt[0..20]);
    pkt[10..12].copy_from_slice(&checksum.to_be_bytes());

    // Copy payload
    pkt[20..].copy_from_slice(payload);

    pkt
}
