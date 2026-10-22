//! The Z21 command station link: a UDP socket, a reader thread that applies replies to the shared state,
//! and the state the API reads (track power, turnouts, locomotives, R-BUS feedback)

pub mod commands;
pub mod frames;
pub mod replies;

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::db;
use crate::state::{AppState, Inner};
use commands::{
    broadcast_flags, rbus_getdata, status_request, LAN_RMBUS_DATACHANGED, LAN_SYSTEMSTATE_DATACHANGED,
    RBUS_GROUPS, SYSTEMSTATE_CENTRAL_STATE_OFFSET, XBUS_STATUS_CHANGED, XBUS_STATUS_DB0, XBUS_LOCO_INFO_REPLY,
    XBUS_TURNOUT_INFO,
};
use frames::LAN_X_HEADER;
use replies::{CentralState, LocoState};

pub const DEFAULT_HOST: &str = "10.0.0.111";
pub const DEFAULT_PORT: u16 = 21105;
/// How long the reader waits for a reply before checking whether it should stop
const READ_TIMEOUT: Duration = Duration::from_millis(500);

pub struct Z21 {
    pub host: String,
    pub port: u16,
    pub connected: bool,
    pub central_state: Option<CentralState>,
    /// (module, input) -> occupied
    pub feedback: BTreeMap<(i64, i64), bool>,
    /// turnout number -> output as last reported (None when the Z21 gave no output)
    pub turnouts: BTreeMap<i64, Option<u8>>,
    pub locos: BTreeMap<i64, LocoState>,
    pub last_heartbeat: String,
    socket: Option<Arc<UdpSocket>>,
    stop: Option<Arc<AtomicBool>>,
}

impl Z21 {
    pub fn new() -> Z21 {
        Z21 {
            host: DEFAULT_HOST.into(),
            port: DEFAULT_PORT,
            connected: false,
            central_state: None,
            feedback: BTreeMap::new(),
            turnouts: BTreeMap::new(),
            locos: BTreeMap::new(),
            last_heartbeat: db::now(),
            socket: None,
            stop: None,
        }
    }

    /// Overall track state: disconnected, unknown, short_circuit, track_power_off, or ready
    pub fn track_state(&self) -> &'static str {
        if !self.connected {
            return "disconnected";
        }
        match self.central_state {
            None => "unknown",
            Some(state) if state.short_circuit => "short_circuit",
            Some(state) if !state.track_power_on => "track_power_off",
            Some(_) => "ready",
        }
    }

    /// Send a frame. Returns false if there is no socket
    pub fn send(&self, frame: &[u8]) -> bool {
        match &self.socket {
            Some(socket) => socket.send(frame).is_ok(),
            None => false,
        }
    }

    pub fn status_json(&self) -> Value {
        json!({
            "connected": self.connected,
            "state": self.track_state(),
            "central_state": self.central_state.map(|state| json!({
                "emergency_stop": state.emergency_stop,
                "track_power_on": state.track_power_on,
                "short_circuit": state.short_circuit,
            })),
            "host": self.host,
            "port": self.port,
            "last_heartbeat": self.last_heartbeat,
        })
    }

    /// Close the socket and forget what the Z21 reported
    pub fn disconnect(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::SeqCst);
        }
        self.socket = None;
        self.connected = false;
        self.central_state = None;
        self.feedback.clear();
        self.turnouts.clear();
        self.locos.clear();
    }
}

/// Open the socket to the Z21 and start the reader. UDP has no handshake, so this confirms nothing itself;
/// the first reply from the Z21 does. Does not hold the lock while the socket is opened
pub fn connect(app: &Arc<AppState>, host: &str, port: u16) -> bool {
    let socket = match UdpSocket::bind("0.0.0.0:0").and_then(|socket| {
        socket.connect((host, port))?;
        socket.set_read_timeout(Some(READ_TIMEOUT))?;
        Ok(socket)
    }) {
        Ok(socket) => Arc::new(socket),
        Err(_) => return false,
    };

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(previous) = inner.z21.stop.take() {
            previous.store(true, Ordering::SeqCst);
        }
        inner.z21.host = host.into();
        inner.z21.port = port;
        inner.z21.socket = Some(socket.clone());
        inner.z21.stop = Some(stop.clone());
        inner.z21.connected = true;
        inner.z21.central_state = None;
    }
    spawn_reader(app.clone(), socket.clone(), stop);

    // Ask for status, then subscribe to broadcasts, which are per client and so set after every connect
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    inner.z21.send(&status_request());
    inner.z21.send(&broadcast_flags());
    for group in RBUS_GROUPS {
        inner.z21.send(&rbus_getdata(group));
    }
    true
}

fn spawn_reader(app: Arc<AppState>, socket: Arc<UdpSocket>, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        while !stop.load(Ordering::SeqCst) {
            match socket.recv(&mut buffer) {
                Ok(length) => {
                    let mut inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    apply(&mut inner, &buffer[..length], Instant::now());
                }
                Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => break,
            }
        }
    });
}

/// Apply one frame from the Z21 to the state, and feed locomotive speeds to moving time
pub fn apply(inner: &mut Inner, frame: &[u8], now: Instant) {
    if frame.len() < 4 {
        return;
    }
    let header = u16::from_le_bytes([frame[2], frame[3]]);
    inner.z21.last_heartbeat = db::now();
    let Inner { db, z21, moving } = inner;

    if header == LAN_X_HEADER && frame.len() >= 7 && frame[4] == XBUS_STATUS_CHANGED && frame[5] == XBUS_STATUS_DB0 {
        set_central_state(z21, frame[6]);
    } else if header == LAN_X_HEADER && frame.len() >= 13 && frame[4] == XBUS_LOCO_INFO_REPLY {
        if let Some((address, state)) = replies::loco_info(&frame[4..frame.len() - 1]) {
            moving.observe(db, address, state.speed, now);
            z21.locos.insert(address, state);
        }
    } else if header == LAN_X_HEADER && frame.len() >= 9 && frame[4] == XBUS_TURNOUT_INFO {
        let (turnout, output) = replies::turnout_info(frame[5], frame[6], frame[7]);
        z21.turnouts.insert(turnout, output);
    } else if header == LAN_SYSTEMSTATE_DATACHANGED && frame.len() > SYSTEMSTATE_CENTRAL_STATE_OFFSET {
        set_central_state(z21, frame[SYSTEMSTATE_CENTRAL_STATE_OFFSET]);
    } else if header == LAN_RMBUS_DATACHANGED && frame.len() >= 15 {
        if let Some(states) = replies::rbus_feedback(frame[4], &frame[5..15]) {
            z21.feedback.extend(states);
        }
    }
}

fn set_central_state(z21: &mut Z21, byte: u8) {
    z21.central_state = Some(replies::central_state(byte));
}
