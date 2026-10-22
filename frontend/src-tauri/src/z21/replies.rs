//! Decoding Z21 replies. Each function takes the frame bytes the way the Z21 sends them

use super::commands::{CENTRAL_EMERGENCY_STOP, CENTRAL_SHORT_CIRCUIT, CENTRAL_TRACK_OFF, RBUS_INPUTS_PER_MODULE, RBUS_MODULES_PER_GROUP};

/// The track state bits, as the Z21 reports them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CentralState {
    pub emergency_stop: bool,
    pub track_power_on: bool,
    pub short_circuit: bool,
}

pub fn central_state(state: u8) -> CentralState {
    CentralState {
        emergency_stop: state & CENTRAL_EMERGENCY_STOP != 0,
        track_power_on: state & CENTRAL_TRACK_OFF == 0,
        short_circuit: state & CENTRAL_SHORT_CIRCUIT != 0,
    }
}

/// A locomotive's last reported state. Speed is None unless the Z21 reported it in the 128-step format
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocoState {
    pub speed: Option<i64>,
    pub direction: &'static str,
    pub functions: Vec<u8>,
}

/// Decode a LAN_X_LOCO_INFO reply. `data` starts at the X-Header (0xEF) and excludes the checksum.
/// Returns the address and the state, or None if the reply is too short
pub fn loco_info(data: &[u8]) -> Option<(i64, LocoState)> {
    if data.len() < 9 || data[0] != 0xEF {
        return None;
    }
    let address = (((data[1] & 0x3F) as i64) << 8) + data[2] as i64;
    let steps = data[3] & 0x07;
    let step = data[4] & 0x7F;
    let speed = (steps == 4).then(|| if step <= 1 { 0 } else { step as i64 - 1 });

    let mut functions = Vec::new();
    let f0_to_f4 = data[5];
    if f0_to_f4 & 0x10 != 0 {
        functions.push(0);
    }
    for (bit, index) in [(0x01u8, 1u8), (0x02, 2), (0x04, 3), (0x08, 4)] {
        if f0_to_f4 & bit != 0 {
            functions.push(index);
        }
    }
    for (group, first) in [(data[6], 5u8), (data[7], 13), (data[8], 21)] {
        for bit in 0..8u8 {
            if group & (1 << bit) != 0 {
                functions.push(first + bit);
            }
        }
    }
    if data.len() > 9 {
        for bit in 0..3u8 {
            if data[9] & (1 << bit) != 0 {
                functions.push(29 + bit);
            }
        }
    }
    functions.sort_unstable();

    let direction = if data[4] & 0x80 != 0 { "forward" } else { "reverse" };
    Some((address, LocoState { speed, direction, functions }))
}

/// Decode a LAN_X_TURNOUT_INFO reply into (turnout, output). The output is None unless ZZ is 01 or 10
pub fn turnout_info(msb: u8, lsb: u8, zz: u8) -> (i64, Option<u8>) {
    let output = match zz & 0x03 {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    };
    let turnout = (((msb as i64) << 8) + lsb as i64) + 1;
    (turnout, output)
}

/// Decode the 10 status bytes of one R-BUS group. Each byte is a module, with one bit per input.
/// Returns (module, input number 1-8, occupied) for every input
pub fn rbus_feedback(group: u8, status: &[u8]) -> Option<Vec<((i64, i64), bool)>> {
    if status.len() != RBUS_MODULES_PER_GROUP {
        return None;
    }
    let mut states = Vec::with_capacity(RBUS_MODULES_PER_GROUP * RBUS_INPUTS_PER_MODULE);
    for (index, byte) in status.iter().enumerate() {
        let module = group as i64 * RBUS_MODULES_PER_GROUP as i64 + index as i64 + 1;
        for bit in 0..RBUS_INPUTS_PER_MODULE {
            states.push(((module, bit as i64 + 1), byte & (1 << bit) != 0));
        }
    }
    Some(states)
}
