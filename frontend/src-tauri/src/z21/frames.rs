//! Z21 LAN frames: the 4-byte header and the X-Bus wrapper with its XOR checksum.
//! Frame layout (little-endian): DataLen (2 bytes, whole frame), Header (2 bytes), Data.

pub const MIN_ADDRESS: i64 = 1;
pub const MAX_ADDRESS: i64 = 9999;
pub const LAN_X_HEADER: u16 = 0x0040;

pub fn xor_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0, |acc, byte| acc ^ byte)
}

pub fn build_frame(header: u16, data: &[u8]) -> Vec<u8> {
    let length = (4 + data.len()) as u16;
    let mut frame = Vec::with_capacity(4 + data.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&header.to_le_bytes());
    frame.extend_from_slice(data);
    frame
}

/// An X-Bus command in a LAN_X frame, with its checksum appended
pub fn build_xbus_frame(xbus: &[u8]) -> Vec<u8> {
    let mut data = xbus.to_vec();
    data.push(xor_checksum(xbus));
    build_frame(LAN_X_HEADER, &data)
}

/// A DCC address as (high byte, low byte). Addresses above 127 are long addresses with the 0xC0 flag
pub fn encode_address(address: i64) -> Result<(u8, u8), String> {
    if !(MIN_ADDRESS..=MAX_ADDRESS).contains(&address) {
        return Err(format!("Address must be {MIN_ADDRESS}-{MAX_ADDRESS}"));
    }
    if address < 128 {
        return Ok((0x00, address as u8));
    }
    Ok((0xC0 | ((address >> 8) as u8 & 0x3F), (address & 0xFF) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_xor_of_all_bytes() {
        assert_eq!(xor_checksum(&[0x21, 0x24, 0x05]), 0x21 ^ 0x24 ^ 0x05);
    }

    #[test]
    fn xbus_frame_is_length_header_then_command_and_checksum() {
        let frame = build_xbus_frame(&[0x21, 0x24, 0x05]);
        assert_eq!(frame, vec![0x08, 0x00, 0x40, 0x00, 0x21, 0x24, 0x05, 0x21 ^ 0x24 ^ 0x05]);
    }

    #[test]
    fn short_and_long_addresses_encode_as_the_spec_says() {
        assert_eq!(encode_address(3), Ok((0x00, 0x03)));
        assert_eq!(encode_address(127), Ok((0x00, 0x7F)));
        assert_eq!(encode_address(128), Ok((0xC0, 0x80)));
        assert_eq!(encode_address(9999), Ok((0xC0 | 0x27, 0x0F)));
        assert!(encode_address(0).is_err());
        assert!(encode_address(10000).is_err());
    }
}
