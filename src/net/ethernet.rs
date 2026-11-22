//! Ethernet II Framing.

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

#[derive(Debug, Clone, Copy)]
pub struct EthernetHeader {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ethertype: u16,
}

impl EthernetHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 14 {
            return None;
        }

        let mut dst_mac = [0u8; 6];
        let mut src_mac = [0u8; 6];
        dst_mac.copy_from_slice(&data[0..6]);
        src_mac.copy_from_slice(&data[6..12]);
        let ethertype = u16::from_be_bytes([data[12], data[13]]);

        Some((
            Self {
                dst_mac,
                src_mac,
                ethertype,
            },
            &data[14..],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) {
        out[0..6].copy_from_slice(&self.dst_mac);
        out[6..12].copy_from_slice(&self.src_mac);
        let et_bytes = self.ethertype.to_be_bytes();
        out[12] = et_bytes[0];
        out[13] = et_bytes[1];
    }
}
