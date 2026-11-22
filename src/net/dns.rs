//! Domain Name System (DNS) Client Resolver (RFC 1035).
//!
//! Encodes standard A-record DNS queries over UDP port 53 and parses
//! server responses to resolve domain names to IPv4 addresses.

use alloc::vec::Vec;

pub const DNS_PORT: u16 = 53;

/// Encodes a domain name string into DNS wire format labels (e.g. "example.com" -> "\x07example\x03com\x00").
pub fn encode_domain_name(domain: &str) -> Vec<u8> {
    let mut encoded = Vec::new();
    for part in domain.split('.') {
        if part.is_empty() {
            continue;
        }
        encoded.push(part.len() as u8);
        encoded.extend_from_slice(part.as_bytes());
    }
    encoded.push(0); // Zero terminator
    encoded
}

/// Builds a DNS query packet for an A record.
pub fn build_dns_query(id: u16, domain: &str) -> Vec<u8> {
    let mut pkt = Vec::new();

    // 1. DNS Header (12 bytes)
    pkt.extend_from_slice(&id.to_be_bytes()); // Transaction ID
    pkt.extend_from_slice(&0x0100u16.to_be_bytes()); // Flags: Standard query, recursion desired (RD=1)
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT (Questions = 1)
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT (Answers = 0)
    pkt.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT (Authority = 0)
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT (Additional = 0)

    // 2. Question Section
    let qname = encode_domain_name(domain);
    pkt.extend_from_slice(&qname);
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QTYPE = 1 (A Record)
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = 1 (IN - Internet)

    pkt
}

/// Parses a DNS response to extract the first IPv4 (A) record address.
pub fn parse_dns_response(data: &[u8], expected_id: u16) -> Option<[u8; 4]> {
    if data.len() < 12 {
        return None;
    }

    let id = u16::from_be_bytes([data[0], data[1]]);
    if id != expected_id {
        return None;
    }

    let flags = u16::from_be_bytes([data[2], data[3]]);
    // Check QR bit (bit 15 must be 1 for response) and RCODE (bits 3:0 must be 0 for NoError)
    if (flags & 0x8000) == 0 || (flags & 0x000F) != 0 {
        return None;
    }

    let qdcount = u16::from_be_bytes([data[4], data[5]]) as usize;
    let ancount = u16::from_be_bytes([data[6], data[7]]) as usize;
    if ancount == 0 {
        return None;
    }

    let mut offset = 12;

    // Skip question sections
    for _ in 0..qdcount {
        while offset < data.len() {
            let len = data[offset] as usize;
            if len == 0 {
                offset += 1;
                break;
            }
            if (len & 0xC0) == 0xC0 {
                offset += 2; // Pointer
                break;
            }
            offset += 1 + len;
        }
        offset += 4; // Skip QTYPE (2B) + QCLASS (2B)
    }

    // Parse answers
    for _ in 0..ancount {
        if offset >= data.len() {
            break;
        }

        // Skip NAME field (can be pointer or label series)
        if (data[offset] & 0xC0) == 0xC0 {
            offset += 2; // Compression pointer
        } else {
            while offset < data.len() {
                let len = data[offset] as usize;
                if len == 0 {
                    offset += 1;
                    break;
                }
                offset += 1 + len;
            }
        }

        if offset + 10 > data.len() {
            break;
        }

        let atype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let _aclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        let _ttl = u32::from_be_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]);
        let rdlength = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
        offset += 10;

        if atype == 1 && rdlength == 4 && offset + 4 <= data.len() {
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&data[offset..offset + 4]);
            return Some(ip);
        }

        offset += rdlength;
    }

    None
}
