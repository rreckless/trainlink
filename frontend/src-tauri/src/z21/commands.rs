//! Z21 commands (frames to send) and replies (frames decoded), from the Z21 LAN protocol spec v1.13.
//! Locomotives are 1-9999 and speeds are 0-126 in the UI, sent in the 128-step format.

use super::frames::{build_frame, build_xbus_frame, encode_address};

pub const LAN_SET_BROADCASTFLAGS: u16 = 0x0050;
pub const LAN_RMBUS_DATACHANGED: u16 = 0x0080;
pub const LAN_RMBUS_GETDATA: u16 = 0x0081;
pub const LAN_SYSTEMSTATE_DATACHANGED: u16 = 0x0084;

/// Broadcasts this client needs: driving and switching, R-BUS feedback, and system state
pub const BROADCAST_FLAGS: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0100;
pub const RBUS_GROUPS: [u8; 2] = [0, 1];
pub const RBUS_MODULES_PER_GROUP: usize = 10;
pub const RBUS_INPUTS_PER_MODULE: usize = 8;

const XBUS_LOCO_DRIVE: u8 = 0xE4;
const XBUS_STOP: u8 = 0x80;
const XBUS_STATUS_REQUEST: [u8; 3] = [0x21, 0x24, 0x05];
const XBUS_TRACK_POWER_ON: [u8; 2] = [0x21, 0x81];
pub const XBUS_STATUS_CHANGED: u8 = 0x62;
pub const XBUS_STATUS_DB0: u8 = 0x22;
pub const XBUS_LOCO_INFO_REQUEST: u8 = 0xE3;
pub const XBUS_LOCO_INFO_REPLY: u8 = 0xEF;
pub const XBUS_SET_FUNCTION: u8 = 0xE4;
pub const XBUS_TURNOUT_SET: u8 = 0x53;
pub const XBUS_TURNOUT_INFO: u8 = 0x43;
const SPEED_STEP_MODE_128: u8 = 0x13;
pub const MAX_FUNCTION: u8 = 31;

// Central state bits (status byte in LAN_X status replies and LAN_SYSTEMSTATE_DATACHANGED)
pub const CENTRAL_EMERGENCY_STOP: u8 = 0x01;
pub const CENTRAL_TRACK_OFF: u8 = 0x02;
pub const CENTRAL_SHORT_CIRCUIT: u8 = 0x04;
/// Offset of the central state byte in a LAN_SYSTEMSTATE_DATACHANGED frame
pub const SYSTEMSTATE_CENTRAL_STATE_OFFSET: usize = 16;

/// A 0-126 UI speed as a 128-step byte. Step 0 is stop and step 1 is emergency stop, so UI speed 1 is step 2
pub fn encode_speed_step(speed: i64, forward: bool) -> Result<u8, String> {
    if !(0..=126).contains(&speed) {
        return Err("Speed must be 0-126".into());
    }
    let step = if speed == 0 { 0 } else { speed as u8 + 1 };
    Ok((if forward { 0x80 } else { 0x00 }) | step)
}

pub fn loco_drive(address: i64, speed: i64, forward: bool) -> Result<Vec<u8>, String> {
    let (high, low) = encode_address(address)?;
    let step = encode_speed_step(speed, forward)?;
    Ok(build_xbus_frame(&[XBUS_LOCO_DRIVE, SPEED_STEP_MODE_128, high, low, step]))
}

pub fn emergency_stop() -> Vec<u8> {
    build_xbus_frame(&[XBUS_STOP])
}

pub fn status_request() -> Vec<u8> {
    build_xbus_frame(&XBUS_STATUS_REQUEST)
}

pub fn track_power_on() -> Vec<u8> {
    build_xbus_frame(&XBUS_TRACK_POWER_ON)
}

pub fn broadcast_flags() -> Vec<u8> {
    build_frame(LAN_SET_BROADCASTFLAGS, &BROADCAST_FLAGS.to_le_bytes())
}

pub fn rbus_getdata(group: u8) -> Vec<u8> {
    build_frame(LAN_RMBUS_GETDATA, &[group])
}

pub fn loco_info_request(address: i64) -> Result<Vec<u8>, String> {
    let (high, low) = encode_address(address)?;
    Ok(build_xbus_frame(&[XBUS_LOCO_INFO_REQUEST, 0xF0, high, low]))
}

/// Switch one function. TT is 01 for on, 00 for off, and NNNNNN is the function number
pub fn set_function(address: i64, index: u8, on: bool) -> Result<Vec<u8>, String> {
    if index > MAX_FUNCTION {
        return Err(format!("Function must be 0-{MAX_FUNCTION}"));
    }
    let (high, low) = encode_address(address)?;
    let switch = if on { 0x40 } else { 0x00 };
    Ok(build_xbus_frame(&[XBUS_SET_FUNCTION, 0xF8, high, low, switch | index]))
}

/// Turnout output command (spec 5.1), queued by the Z21. The address is one less than the turnout number
pub fn turnout_set(turnout: i64, output: u8, activate: bool) -> Result<Vec<u8>, String> {
    let (high, low) = turnout_address(turnout)?;
    let flags = 0x80 | 0x20 | if activate { 0x08 } else { 0x00 } | (output & 0x01);
    Ok(build_xbus_frame(&[XBUS_TURNOUT_SET, high, low, flags]))
}

/// Ask for one turnout's output (spec 5.3). The reply is a turnout info frame
pub fn turnout_info_request(turnout: i64) -> Result<Vec<u8>, String> {
    let (high, low) = turnout_address(turnout)?;
    Ok(build_xbus_frame(&[XBUS_TURNOUT_INFO, high, low]))
}

fn turnout_address(turnout: i64) -> Result<(u8, u8), String> {
    if !(1..=2048).contains(&turnout) {
        return Err("Turnout must be 1-2048".into());
    }
    let address = (turnout - 1) as u16;
    Ok(((address >> 8) as u8, (address & 0xFF) as u8))
}
