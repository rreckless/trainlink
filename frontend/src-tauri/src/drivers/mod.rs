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

/// Why a driver did not send a command
#[derive(Clone, Debug, PartialEq)]
pub enum DriverError {
    /// The command is not one the protocol can carry, such as a speed above 126. The message says why
    Invalid(String),
    /// There is no connection to the station
    NotConnected,
    /// The command could not be sent
    Failed,
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverError::Invalid(message) => write!(f, "{message}"),
            DriverError::NotConnected => write!(f, "Not connected to the command station"),
            DriverError::Failed => write!(f, "Could not send the command to the command station"),
        }
    }
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
    /// The state of the track: disconnected, lost, unknown, short_circuit, track_power_off, or ready
    fn track_state(&self) -> &'static str;
    /// Ask the station for its status. The answer comes back as a reply
    fn request_status(&self) -> Result<(), DriverError>;
    /// Ask for a locomotive's speed, direction and functions
    fn request_loco_info(&self, address: i64) -> Result<(), DriverError>;
    /// Ask for a turnout's state
    fn request_turnout_info(&self, turnout: i64) -> Result<(), DriverError>;
    /// Set a locomotive's speed and direction
    fn drive(&mut self, address: i64, speed: i64, forward: bool) -> Result<(), DriverError>;
    /// Switch one of a locomotive's functions on or off
    fn set_function(&mut self, address: i64, index: u8, on: bool) -> Result<(), DriverError>;
    /// Throw a turnout's output on (activate) or off
    fn throw_turnout(&mut self, turnout: i64, output: u8, activate: bool) -> Result<(), DriverError>;
    /// Stop every locomotive
    fn emergency_stop(&mut self) -> Result<(), DriverError>;
    /// Switch the track power on or off
    fn track_power(&mut self, on: bool) -> Result<(), DriverError>;
}

impl Z21 {
    /// Send a command the protocol built, or say why it could not be sent. A build error is a value the protocol
    /// cannot carry, and nothing is sent for it
    fn send_command(&self, frame: Result<Vec<u8>, String>) -> Result<(), DriverError> {
        let frame = frame.map_err(DriverError::Invalid)?;
        if !self.connected {
            return Err(DriverError::NotConnected);
        }
        if self.send(&frame) {
            Ok(())
        } else {
            Err(DriverError::Failed)
        }
    }
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

    fn track_state(&self) -> &'static str {
        Z21::track_state(self)
    }

    fn request_status(&self) -> Result<(), DriverError> {
        self.send_command(Ok(crate::z21::commands::status_request()))
    }

    fn request_loco_info(&self, address: i64) -> Result<(), DriverError> {
        self.send_command(crate::z21::commands::loco_info_request(address))
    }

    fn request_turnout_info(&self, turnout: i64) -> Result<(), DriverError> {
        self.send_command(crate::z21::commands::turnout_info_request(turnout))
    }

    fn drive(&mut self, address: i64, speed: i64, forward: bool) -> Result<(), DriverError> {
        self.send_command(crate::z21::commands::loco_drive(address, speed, forward))
    }

    fn set_function(&mut self, address: i64, index: u8, on: bool) -> Result<(), DriverError> {
        self.send_command(crate::z21::commands::set_function(address, index, on))
    }

    fn throw_turnout(&mut self, turnout: i64, output: u8, activate: bool) -> Result<(), DriverError> {
        self.send_command(crate::z21::commands::turnout_set(turnout, output, activate))
    }

    fn emergency_stop(&mut self) -> Result<(), DriverError> {
        self.send_command(Ok(crate::z21::commands::emergency_stop()))
    }

    fn track_power(&mut self, on: bool) -> Result<(), DriverError> {
        let frame = if on { crate::z21::commands::track_power_on() } else { crate::z21::commands::track_power_off() };
        self.send_command(Ok(frame))
    }
}

/// Whether a station type has a driver
pub fn is_supported(station_type: &str) -> bool {
    SUPPORTED_TYPES.contains(&station_type)
}
