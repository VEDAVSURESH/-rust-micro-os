//! Address Resolution Protocol (ARP) for IPv4 over Ethernet.

use crate::net::ethernet::{EthernetHeader, BROADCAST_MAC, ETHERTYPE_ARP};
use crate::sync::Spinlock;

pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

#[derive(Debug, Clone, Copy)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub opcode: u16,
    pub sender_mac: [u8; 6],
    pub sender_ip: [u8; 4],
    pub target_mac: [u8; 6],
    pub target_ip: [u8; 4],
}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }

        let htype = u16::from_be_bytes([data[0], data[1]]);
        let ptype = u16::from_be_bytes([data[2], data[3]]);
        let hlen = data[4];
        let plen = data[5];
        let opcode = u16::from_be_bytes([data[6], data[7]]);

        let mut sender_mac = [0u8; 6];
        sender_mac.copy_from_slice(&data[8..14]);
        let mut sender_ip = [0u8; 4];
        sender_ip.copy_from_slice(&data[14..18]);

        let mut target_mac = [0u8; 6];
        target_mac.copy_from_slice(&data[18..24]);
        let mut target_ip = [0u8; 4];
        target_ip.copy_from_slice(&data[24..28]);

        Some(Self {
            htype,
            ptype,
            hlen,
            plen,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    pub fn to_bytes(&self) -> [u8; 28] {
        let mut out = [0u8; 28];
        out[0..2].copy_from_slice(&self.htype.to_be_bytes());
        out[2..4].copy_from_slice(&self.ptype.to_be_bytes());
        out[4] = self.hlen;
        out[5] = self.plen;
        out[6..8].copy_from_slice(&self.opcode.to_be_bytes());
        out[8..14].copy_from_slice(&self.sender_mac);
        out[14..18].copy_from_slice(&self.sender_ip);
        out[18..24].copy_from_slice(&self.target_mac);
        out[24..28].copy_from_slice(&self.target_ip);
        out
    }
}

/// A cache entry mapping an IPv4 address to an Ethernet MAC address.
#[derive(Debug, Clone, Copy)]
pub struct ArpEntry {
    pub ip: [u8; 4],
    pub mac: [u8; 6],
}

pub struct ArpTable {
    entries: [Option<ArpEntry>; 16],
}

impl ArpTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; 16],
        }
    }

    pub fn insert(&mut self, ip: [u8; 4], mac: [u8; 6]) {
        // Update existing entry if present
        for entry in self.entries.iter_mut() {
            if let Some(ref mut e) = entry {
                if e.ip == ip {
                    e.mac = mac;
                    return;
                }
            }
        }
        // Insert into first free slot
        for entry in self.entries.iter_mut() {
            if entry.is_none() {
                *entry = Some(ArpEntry { ip, mac });
                return;
            }
        }
        // If full, overwrite slot 0
        self.entries[0] = Some(ArpEntry { ip, mac });
    }

    pub fn lookup(&self, ip: [u8; 4]) -> Option<[u8; 6]> {
        for entry in self.entries.iter().flatten() {
            if entry.ip == ip {
                return Some(entry.mac);
            }
        }
        None
    }
}

pub static ARP_TABLE: Spinlock<ArpTable> = Spinlock::new(ArpTable::new());

/// Sends an ARP request packet broadcast over Ethernet for `target_ip`.
pub fn send_arp_request(src_mac: [u8; 6], src_ip: [u8; 4], target_ip: [u8; 4]) {
    let eth_hdr = EthernetHeader {
        dst_mac: BROADCAST_MAC,
        src_mac,
        ethertype: ETHERTYPE_ARP,
    };

    let arp_pkt = ArpPacket {
        htype: 1, // Ethernet
        ptype: 0x0800, // IPv4
        hlen: 6,
        plen: 4,
        opcode: ARP_OP_REQUEST,
        sender_mac: src_mac,
        sender_ip: src_ip,
        target_mac: [0u8; 6],
        target_ip,
    };

    let mut frame = [0u8; 60];
    eth_hdr.write_to(&mut frame[0..14]);
    frame[14..42].copy_from_slice(&arp_pkt.to_bytes());

    let _ = crate::e1000::send_packet(&frame);
}

/// Sends an ARP reply packet responding to `request`.
pub fn send_arp_reply(src_mac: [u8; 6], src_ip: [u8; 4], target_mac: [u8; 6], target_ip: [u8; 4]) {
    let eth_hdr = EthernetHeader {
        dst_mac: target_mac,
        src_mac,
        ethertype: ETHERTYPE_ARP,
    };

    let arp_pkt = ArpPacket {
        htype: 1,
        ptype: 0x0800,
        hlen: 6,
        plen: 4,
        opcode: ARP_OP_REPLY,
        sender_mac: src_mac,
        sender_ip: src_ip,
        target_mac,
        target_ip,
    };

    let mut frame = [0u8; 60];
    eth_hdr.write_to(&mut frame[0..14]);
    frame[14..42].copy_from_slice(&arp_pkt.to_bytes());

    let _ = crate::e1000::send_packet(&frame);
}
