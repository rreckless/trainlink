//! Command station drivers. Each station type speaks its own protocol, and the app drives a station through the Driver
//! trait. The station types TrainLink supports are listed here, and a station is only saved with one of them

use std::sync::Arc;

use crate::state::AppState;
use crate::z21::{self, Z21};

/// The station types with a driver, as a command station stores them
pub const SUPPORTED_TYPES: [&str; 1] = ["z21"];

/// One occupancy input a station reports: its module and input number, and whether it is occupied
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedbackInput {
    pub module: i64,
    pub input: i64,
    pub occupied: bool,
}

/// What every command station driver does. Speeds are 0 to 126 and addresses 1 to 9999, as the DCC standard gives them
pub trait Driver {
    /// Open a connection to the station at this host and port, and start reading its replies. Returns false if no
    /// connection could be opened. A driver that is not connected refuses commands
    fn connect(app: &Arc<AppState>, host: &str, port: u16) -> bool
    where
        Self: Sized;
    /// Close the connection, and forget what the station reported
    fn disconnect(&mut self);
    /// The station type this driver speaks, as stored on a command station
    fn station_type(&self) -> &'static str;
    /// Whether a connection to the station is open
    fn connected(&self) -> bool;
    /// Ask the station for the state of its occupancy inputs. The answers come back as replies
    fn request_feedback(&self);
    /// The occupancy inputs the station has reported
    fn feedback_inputs(&self) -> Vec<FeedbackInput>;
    /// The features the station has, by name
    fn features(&self) -> Vec<&'static str>;
    /// Set a locomotive's speed and direction
    fn drive(&mut self, address: i64, speed: i64, forward: bool) -> Result<(), String>;
    /// Switch one of a locomotive's functions on or off
    fn set_function(&mut self, address: i64, index: u8, on: bool) -> Result<(), String>;
    /// Stop every locomotive
    fn emergency_stop(&mut self) -> Result<(), String>;
    /// Switch the track power on or off
    fn track_power(&mut self, on: bool) -> Result<(), String>;
}

impl Driver for Z21 {
    fn connect(app: &Arc<AppState>, host: &str, port: u16) -> bool {
        z21::connect(app, host, port)
    }

    fn disconnect(&mut self) {
        Z21::disconnect(self)
    }

    fn station_type(&self) -> &'static str {
        "z21"
    }

    fn connected(&self) -> bool {
        self.connected
    }

    fn request_feedback(&self) {
        if self.connected {
            for group in crate::z21::commands::RBUS_GROUPS {
                self.send(&crate::z21::commands::rbus_getdata(group));
            }
        }
    }

    fn feedback_inputs(&self) -> Vec<FeedbackInput> {
        self.feedback.iter().map(|((module, input), occupied)| FeedbackInput { module: *module, input: *input, occupied: *occupied }).collect()
    }

    fn features(&self) -> Vec<&'static str> {
        self.hardware.map(|hardware| hardware.features().names()).unwrap_or_default()
    }

    fn drive(&mut self, address: i64, speed: i64, forward: bool) -> Result<(), String> {
        let frame = crate::z21::commands::loco_drive(address, speed, forward)?;
        self.send_or_error(&frame)
    }

    fn set_function(&mut self, address: i64, index: u8, on: bool) -> Result<(), String> {
        let frame = crate::z21::commands::set_function(address, index, on)?;
        self.send_or_error(&frame)
    }

    fn emergency_stop(&mut self) -> Result<(), String> {
        if !self.connected {
            return Err("Z21 not connected".into());
        }
        self.send_or_error(&crate::z21::commands::emergency_stop())
    }

    fn track_power(&mut self, on: bool) -> Result<(), String> {
        let frame = if on { crate::z21::commands::track_power_on() } else { crate::z21::commands::track_power_off() };
        self.send_or_error(&frame)
    }
}

/// Whether a station type has a driver
pub fn is_supported(station_type: &str) -> bool {
    SUPPORTED_TYPES.contains(&station_type)
}
