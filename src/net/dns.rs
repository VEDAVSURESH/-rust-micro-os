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

/// Skips a domain name in DNS wire format starting at `offset`.
///
/// Handles uncompressed label sequences, 2-byte RFC 1035 compression pointers (`0xC0XX`),
/// and mixed label/pointer names (e.g. `\x03www\xC0\x0C`).
/// Returns the offset immediately following the name within the record, or `None` if malformed.
pub fn skip_name(data: &[u8], mut offset: usize) -> Option<usize> {
    let mut iterations = 0;
    while offset < data.len() {
        iterations += 1;
        if iterations > 128 {
            // Prevent cyclic / malformed pointer infinite loops
            return None;
        }

        let byte = data[offset];
        if byte == 0 {
            // Null label terminator
            return Some(offset + 1);
        } else if (byte & 0xC0) == 0xC0 {
            // 2-byte compression pointer
            if offset + 2 > data.len() {
                return None;
            }
            return Some(offset + 2);
        } else {
            // Standard length-prefixed label (1..63 bytes)
            let len = (byte & 0x3F) as usize;
            if offset + 1 + len > data.len() {
                return None;
            }
            offset += 1 + len;
        }
    }
    None
}

/// Parses a DNS response to extract the first IPv4 (A) record address.
///
/// Adheres strictly to RFC 1035:
/// - Verifies 12-byte header minimum.
/// - Matches expected Transaction ID.
/// - Validates QR flag (bit 15 = 1) and RCODE (bits 3:0 == 0 / NoError).
/// - Safely skips all question records.
/// - Traverses answer records, skipping CNAME or non-A records to find an A record.
/// - Fully bounds-checked against packet buffer length.
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
        offset = skip_name(data, offset)?;
        if offset + 4 > data.len() {
            return None;
        }
        offset += 4; // Skip QTYPE (2B) + QCLASS (2B)
    }

    // Parse answers
    for _ in 0..ancount {
        if offset >= data.len() {
            break;
        }

        // Skip NAME field in the resource record
        offset = skip_name(data, offset)?;

        // Resource record header: TYPE (2B) + CLASS (2B) + TTL (4B) + RDLENGTH (2B) = 10 bytes
        if offset + 10 > data.len() {
            return None;
        }

        let atype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let _aclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        let _ttl = u32::from_be_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        let rdlength = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
        offset += 10;

        if offset + rdlength > data.len() {
            return None;
        }

        // TYPE 1 = A Record (IPv4 Address, 4 bytes)
        if atype == 1 && rdlength == 4 {
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&data[offset..offset + 4]);
            return Some(ip);
        }

        // Skip RDATA for non-A records (such as CNAME or MX) and continue searching
        offset += rdlength;
    }

    None
}
