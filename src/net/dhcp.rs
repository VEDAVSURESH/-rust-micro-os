//! Dynamic Host Configuration Protocol (DHCP) Client.
//!
//! Automatically obtains IP configuration (IP, Subnet Mask, Gateway, DNS)
//! via DHCP Discover, Offer, Request, and Ack exchanges (RFC 2131).

use alloc::vec::Vec;

pub const DHCP_CLIENT_PORT: u16 = 68;
pub const DHCP_SERVER_PORT: u16 = 67;

pub const DHCP_MAGIC_COOKIE: [u8; 4] = [0x63, 0x82, 0x53, 0x63];

pub const DHCP_MSG_DISCOVER: u8 = 1;
pub const DHCP_MSG_OFFER: u8 = 2;
pub const DHCP_MSG_REQUEST: u8 = 3;
pub const DHCP_MSG_ACK: u8 = 5;

#[derive(Debug, Clone, Copy)]
pub struct DhcpLease {
    pub client_ip: [u8; 4],
    pub subnet_mask: [u8; 4],
    pub router: [u8; 4],
    pub dns_server: [u8; 4],
    pub server_ip: [u8; 4],
}

/// Builds a DHCP Discover message.
pub fn build_dhcp_discover(xid: u32, mac: [u8; 6]) -> Vec<u8> {
    let mut msg = alloc::vec![0u8; 300];

    msg[0] = 1; // BOOTREQUEST
    msg[1] = 1; // Ethernet
    msg[2] = 6; // MAC length
    msg[3] = 0; // Hops

    msg[4..8].copy_from_slice(&xid.to_be_bytes());
    msg[8..10].copy_from_slice(&[0, 0]); // Secs
    msg[10..12].copy_from_slice(&0x8000u16.to_be_bytes()); // Broadcast flag

    // ciaddr, yiaddr, siaddr, giaddr all 0
    msg[28..34].copy_from_slice(&mac); // chaddr

    // Magic Cookie at offset 236
    msg[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    // Options:
    let mut opt = 240;

    // Option 53: DHCP Message Type = Discover (1)
    msg[opt] = 53;
    msg[opt + 1] = 1;
    msg[opt + 2] = DHCP_MSG_DISCOVER;
    opt += 3;

    // Option 55: Parameter Request List (1=Subnet, 3=Router, 6=DNS)
    msg[opt] = 55;
    msg[opt + 1] = 3;
    msg[opt + 2] = 1; // Subnet Mask
    msg[opt + 3] = 3; // Router
    msg[opt + 4] = 6; // DNS
    opt += 5;

    // Option 255: End
    msg[opt] = 255;
    opt += 1;

    // Ensure RFC 1542 minimum 300-byte BOOTP packet size
    msg.resize(core::cmp::max(opt, 300), 0);
    msg
}

/// Builds a DHCP Request message.
pub fn build_dhcp_request(
    xid: u32,
    mac: [u8; 6],
    requested_ip: [u8; 4],
    server_ip: [u8; 4],
) -> Vec<u8> {
    let mut msg = alloc::vec![0u8; 300];

    msg[0] = 1; // BOOTREQUEST
    msg[1] = 1; // Ethernet
    msg[2] = 6; // MAC length
    msg[3] = 0; // Hops

    msg[4..8].copy_from_slice(&xid.to_be_bytes());
    msg[8..10].copy_from_slice(&[0, 0]);
    msg[10..12].copy_from_slice(&0x8000u16.to_be_bytes()); // Broadcast

    msg[28..34].copy_from_slice(&mac);
    msg[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    let mut opt = 240;

    // Option 53: DHCP Message Type = Request (3)
    msg[opt] = 53;
    msg[opt + 1] = 1;
    msg[opt + 2] = DHCP_MSG_REQUEST;
    opt += 3;

    // Option 50: Requested IP
    msg[opt] = 50;
    msg[opt + 1] = 4;
    msg[opt + 2..opt + 6].copy_from_slice(&requested_ip);
    opt += 6;

    // Option 54: Server Identifier
    msg[opt] = 54;
    msg[opt + 1] = 4;
    msg[opt + 2..opt + 6].copy_from_slice(&server_ip);
    opt += 6;

    // Option 55: Parameter Request List
    msg[opt] = 55;
    msg[opt + 1] = 3;
    msg[opt + 2] = 1;
    msg[opt + 3] = 3;
    msg[opt + 4] = 6;
    opt += 5;

    // Option 255: End
    msg[opt] = 255;
    opt += 1;

    // Ensure RFC 1542 minimum 300-byte BOOTP packet size
    msg.resize(core::cmp::max(opt, 300), 0);
    msg
}

/// Parses a DHCP response (Offer or Ack).
pub fn parse_dhcp_response(data: &[u8], expected_xid: u32) -> Option<(u8, DhcpLease)> {
    if data.len() < 244 {
        return None;
    }

    // Must be BOOTREPLY (2)
    if data[0] != 2 {
        return None;
    }

    let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if xid != expected_xid {
        return None;
    }

    // Check Magic Cookie at offset 236
    if data[236..240] != DHCP_MAGIC_COOKIE {
        return None;
    }

    let mut client_ip = [0u8; 4];
    client_ip.copy_from_slice(&data[16..20]); // yiaddr (Your IP)

    let mut subnet_mask = [255, 255, 255, 0];
    let mut router = [0u8; 4];
    let mut dns_server = [8, 8, 8, 8];
    let mut server_ip = [0u8; 4];
    let mut msg_type = 0u8;

    // Parse Options starting at offset 240
    let mut idx = 240;
    while idx < data.len() {
        let opt = data[idx];
        if opt == 255 {
            break; // End
        }
        if opt == 0 {
            idx += 1; // Pad
            continue;
        }

        if idx + 1 >= data.len() {
            break;
        }
        let len = data[idx + 1] as usize;
        let val_start = idx + 2;
        let val_end = val_start + len;
        if val_end > data.len() {
            break;
        }

        let val = &data[val_start..val_end];
        match opt {
            53 if len >= 1 => msg_type = val[0],
            1 if len >= 4 => subnet_mask.copy_from_slice(&val[0..4]),
            3 if len >= 4 => router.copy_from_slice(&val[0..4]),
            6 if len >= 4 => dns_server.copy_from_slice(&val[0..4]),
            54 if len >= 4 => server_ip.copy_from_slice(&val[0..4]),
            _ => {}
        }

        idx = val_end;
    }

    Some((
        msg_type,
        DhcpLease {
            client_ip,
            subnet_mask,
            router,
            dns_server,
            server_ip,
        },
    ))
}
