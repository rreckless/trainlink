//! What the connected Z21 is and which features its firmware has. Hardware types and the firmware that added each feature
//! are from the Z21 LAN protocol specification (LAN_GET_HWINFO, and the revision history)

/// The header of LAN_GET_HWINFO, the request and its reply
pub const LAN_GET_HWINFO: u16 = 0x001A;

/// The hardware type and firmware version a Z21 reports for LAN_GET_HWINFO
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HardwareInfo {
    pub hw_type: u32,
    /// The firmware version in BCD: 0x0143 is 1.43
    pub firmware: u16,
}

/// The firmware that added reading CVs on the main track (POM read byte), per the specification's revision history
const POM_READ_FROM: u16 = 0x0122;

impl HardwareInfo {
    /// The name of the hardware, or None for a type the specification does not list
    pub fn model(&self) -> Option<&'static str> {
        Some(match self.hw_type {
            0x0200 => "Z21 (black, 2012)",
            0x0201 => "Z21 (black, 2013)",
            0x0202 => "SmartRail",
            0x0203 => "Z21 starter set (white, 2013)",
            0x0204 => "Z21 start",
            0x0205 => "Z21 single booster",
            0x0206 => "Z21 dual booster",
            0x0211 => "Z21 XL series",
            0x0212 => "Z21 XL booster",
            0x0301 => "Z21 switch decoder",
            0x0302 => "Z21 signal decoder",
            _ => return None,
        })
    }

    /// The firmware as the Z21 shows it, for example 1.43
    pub fn firmware_text(&self) -> String {
        format!("{:x}.{:02x}", self.firmware >> 8, self.firmware & 0xff)
    }

    /// The features this hardware and firmware have. Only the command stations the specification lists are assumed to
    /// have any; for anything else, nothing is claimed
    pub fn features(&self) -> Features {
        let known_command_station = self.model().is_some() && !(0x0301..=0x0302).contains(&self.hw_type);
        Features { pom_read: known_command_station && self.firmware >= POM_READ_FROM }
    }
}

/// The features one command station has
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Features {
    /// Reading a CV of a locomotive on the main track
    pub pom_read: bool,
}

impl Features {
    /// The names of the features that are present, for the status
    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.pom_read {
            names.push("pom_read");
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_is_shown_from_its_bcd_digits() {
        assert_eq!(HardwareInfo { hw_type: 0x0200, firmware: 0x0120 }.firmware_text(), "1.20");
        assert_eq!(HardwareInfo { hw_type: 0x0200, firmware: 0x0143 }.firmware_text(), "1.43");
    }

    #[test]
    fn pom_read_needs_firmware_1_22_on_a_listed_command_station() {
        let z21 = |hw_type, firmware| HardwareInfo { hw_type, firmware }.features().pom_read;
        assert!(!z21(0x0200, 0x0120));
        assert!(z21(0x0200, 0x0122));
        assert!(z21(0x0211, 0x0143));
        assert!(!z21(0x0301, 0x0143), "a decoder is not a command station");
        assert!(!z21(0x9999, 0x0143), "an unlisted type claims nothing");
    }
}
