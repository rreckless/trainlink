//! The Z21 command station link: a UDP socket, a reader thread that applies replies to the shared state,
//! and the state the API reads (track power, turnouts, locomotives, R-BUS feedback)

pub mod commands;
pub mod features;
pub mod frames;
pub mod replies;

use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::db;
use crate::state::{AppState, Inner};
use commands::{
    XBUS_CV_NACK_SC_OR_NACK, XBUS_CV_RESULT,
    broadcast_flags, rbus_getdata, status_request, LAN_RMBUS_DATACHANGED, LAN_SYSTEMSTATE_DATACHANGED,
    RBUS_GROUPS, SYSTEMSTATE_CENTRAL_STATE_OFFSET, XBUS_STATUS_CHANGED, XBUS_STATUS_DB0, XBUS_LOCO_INFO_REPLY,
    XBUS_TURNOUT_INFO, XBUS_VERSION_REPLY, hardware_info_request, version_request,
};
use features::LAN_GET_HWINFO;
use features::HardwareInfo;
use frames::LAN_X_HEADER;
use replies::{CentralState, LocoState};

pub const DEFAULT_HOST: &str = "10.0.0.111";
pub const DEFAULT_PORT: u16 = 21105;

/// Whether the address is this computer or a private network address. The app talks only to a Z21 there, so a
/// request cannot point it at a computer elsewhere on the internet. A host name other than localhost is refused
pub fn is_local_address(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
        Err(_) => false,
    }
}
/// How long the reader waits for a reply before checking whether it should stop
const READ_TIMEOUT: Duration = Duration::from_millis(500);
/// How often the reader sends a status request. The spec asks each client to be in touch with the Z21 at least once a
/// minute, or the Z21 drops it from its list of clients and stops sending it changes
const KEEP_ALIVE_EVERY: Duration = Duration::from_secs(10);
/// How long without any frame from the Z21 before the link counts as lost. Commands are refused while it is lost, and
/// the watchdog reconnects
const LOST_AFTER: Duration = Duration::from_secs(30);
/// How often the watchdog checks the link
pub const WATCH_EVERY: Duration = Duration::from_secs(1);

/// How many feedback reports are kept while the app runs
const REPORTS_KEPT: usize = 200;

/// A feedback input that changed state, as the Z21 reported it
#[derive(Debug, Clone)]
pub struct FeedbackReport {
    pub module: i64,
    pub input: i64,
    pub occupied: bool,
    pub at: String,
}

pub struct Z21 {
    pub host: String,
    pub port: u16,
    pub connected: bool,
    pub central_state: Option<CentralState>,
    /// (module, input) -> occupied
    pub feedback: BTreeMap<(i64, i64), bool>,
    /// Input changes, oldest first, kept in memory only while the app runs
    pub reports: VecDeque<FeedbackReport>,
    /// turnout number -> output as last reported (None when the Z21 gave no output)
    pub turnouts: BTreeMap<i64, Option<u8>>,
    pub locos: BTreeMap<i64, LocoState>,
    /// The X-Bus version and command station ID the Z21 reported, from the version request sent on connecting
    pub version: Option<XbusVersion>,
    /// The hardware type and firmware the Z21 reported, from LAN_GET_HWINFO sent on connecting
    pub hardware: Option<HardwareInfo>,
    pub last_heartbeat: String,
    /// When the last frame from the Z21 arrived, or when the socket was opened
    pub last_reply: Instant,
    /// The programming operation in progress and the last one that finished
    pub programming: Programming,
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
            reports: VecDeque::new(),
            turnouts: BTreeMap::new(),
            locos: BTreeMap::new(),
            version: None,
            hardware: None,
            last_heartbeat: db::now(),
            last_reply: Instant::now(),
            socket: None,
            stop: None,
            programming: Programming::default(),
        }
    }

    /// No frame from the Z21 for longer than LOST_AFTER
    pub fn is_lost(&self, now: Instant) -> bool {
        now.duration_since(self.last_reply) > LOST_AFTER
    }

    /// Overall track state: disconnected, lost, unknown, short_circuit, track_power_off, or ready
    pub fn track_state(&self) -> &'static str {
        if !self.connected {
            return "disconnected";
        }
        if self.is_lost(Instant::now()) {
            return "lost";
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
            "xbus_version": self.version.map(|version| format!("{}.{}", version.xbus >> 4, version.xbus & 0x0f)),
            "station_id": self.version.map(|version| version.station_id),
            "model": self.hardware.and_then(|hardware| hardware.model()),
            "firmware": self.hardware.map(|hardware| hardware.firmware_text()),
            "features": self.hardware.map(|hardware| hardware.features().names()).unwrap_or_default(),
        })
    }

    /// Store what the Z21 reported for each input. A report is kept when an input changes, or when an input is first
    /// seen occupied. The starting state of the other inputs is not reported, since a status frame covers 80 of them
    pub fn record_feedback(&mut self, states: Vec<((i64, i64), bool)>) {
        for ((module, input), occupied) in states {
            let report = match self.feedback.get(&(module, input)) {
                Some(&before) => before != occupied,
                None => occupied,
            };
            if report {
                self.reports.push_back(FeedbackReport { module, input, occupied, at: db::now() });
                if self.reports.len() > REPORTS_KEPT {
                    self.reports.pop_front();
                }
            }
            self.feedback.insert((module, input), occupied);
        }
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
        self.version = None;
        self.hardware = None;
    }
}

/// What the Z21 reported about itself: the X-Bus version (0x40 is 4.0) and the command station ID (0x12 is the Z21 family)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XbusVersion {
    pub xbus: u8,
    pub station_id: u8,
}

/// Open the socket to the Z21 and start the reader. UDP has no handshake, so this confirms nothing itself;
/// the first reply from the Z21 does. Does not hold the lock while the socket is opened
pub fn connect(app: &Arc<AppState>, host: &str, port: u16) -> bool {
    connect_with_keep_alive(app, host, port, KEEP_ALIVE_EVERY)
}

/// Connect, sending a status request to keep the link up every `keep_alive`
pub fn connect_with_keep_alive(app: &Arc<AppState>, host: &str, port: u16, keep_alive: Duration) -> bool {
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
        inner.z21.last_reply = Instant::now();
    }
    spawn_reader(app.clone(), socket.clone(), stop, keep_alive);

    // Ask for status, then subscribe to broadcasts, which are per client and so set after every connect
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    inner.z21.send(&status_request());
    inner.z21.send(&version_request());
    inner.z21.send(&hardware_info_request());
    inner.z21.send(&broadcast_flags());
    for group in RBUS_GROUPS {
        inner.z21.send(&rbus_getdata(group));
    }
    true
}

/// Reads replies and applies them, and sends the status request that keeps the link up. A socket error does not end
/// the reader: a Z21 that is not there can refuse a send, and the watchdog reconnects if no replies come
fn spawn_reader(app: Arc<AppState>, socket: Arc<UdpSocket>, stop: Arc<AtomicBool>, keep_alive: Duration) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        let mut last_poll = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            if last_poll.elapsed() >= keep_alive {
                let _ = socket.send(&status_request());
                last_poll = Instant::now();
            }
            match socket.recv(&mut buffer) {
                Ok(length) => {
                    let mut inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    apply(&mut inner, &buffer[..length], Instant::now());
                }
                Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => std::thread::sleep(READ_TIMEOUT),
            }
        }
    });
}

/// Reconnect if the link has been lost. Returns true if a reconnect was tried. Nothing happens after a disconnect, or
/// while the link is fine. A reconnect that does not open a socket is tried again on the next check
pub fn link_check(app: &Arc<AppState>, now: Instant) -> bool {
    let (host, port) = {
        let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !inner.z21.connected || !inner.z21.is_lost(now) {
            return false;
        }
        (inner.z21.host.clone(), inner.z21.port)
    };
    connect(app, &host, port);
    true
}

/// Check the link every WATCH_EVERY, for as long as the app runs
pub fn spawn_watch(app: Arc<AppState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(WATCH_EVERY);
        link_check(&app, Instant::now());
    });
}

/// Apply one frame from the Z21 to the state, and feed locomotive speeds to moving time
pub fn apply(inner: &mut Inner, frame: &[u8], now: Instant) {
    if frame.len() < 4 {
        return;
    }
    let header = u16::from_le_bytes([frame[2], frame[3]]);
    inner.z21.last_heartbeat = db::now();
    inner.z21.last_reply = now;
    let Inner { db, z21, moving, .. } = inner;
    if header == LAN_GET_HWINFO && frame.len() >= 12 {
        // Reply: hardware type and firmware version, both 32 bits little-endian in the spec, the firmware as BCD
        z21.hardware = Some(HardwareInfo {
            hw_type: u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]),
            firmware: u16::from_le_bytes([frame[8], frame[9]]),
        });
    }

    if header == LAN_X_HEADER && frame.len() >= 7 && frame[4] == XBUS_STATUS_CHANGED && frame[5] == XBUS_STATUS_DB0 {
        set_central_state(z21, frame[6]);
    } else if header == LAN_X_HEADER && frame.len() >= 9 && frame[4] == XBUS_VERSION_REPLY && frame[5] == 0x21 {
        z21.version = Some(XbusVersion { xbus: frame[6], station_id: frame[7] });
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
    } else if header == LAN_X_HEADER && frame.len() >= 6 && frame[4] == XBUS_CV_NACK_SC_OR_NACK {
        // 0x12: the programming track is short-circuited. 0x13: the decoder did not acknowledge
        let error = if frame[5] == 0x12 {
            "The programming track is short-circuited"
        } else {
            "The decoder did not acknowledge the command"
        };
        z21.programming.finish_with_error(error);
    } else if header == LAN_X_HEADER && frame.len() >= 10 && frame[4] == XBUS_CV_RESULT && frame[5] == 0x14 {
        let cv = ((frame[6] as u16) << 8 | frame[7] as u16) + 1;
        z21.programming.finish_with_value(cv, frame[8]);
    } else if header == LAN_RMBUS_DATACHANGED && frame.len() >= 15 {
        if let Some(states) = replies::rbus_feedback(frame[4], &frame[5..15]) {
            z21.record_feedback(states);
        }
    }
}

/// Programming operations: one at a time, and the last result is kept until the next one starts
#[derive(Debug, Clone, Default)]
pub struct Programming {
    pub pending: Option<Pending>,
    pub last: Option<Outcome>,
}

/// An operation waiting for the Z21's answer
#[derive(Debug, Clone)]
pub struct Pending {
    pub cv: u16,
    pub kind: &'static str,
    pub value: Option<u8>,
    pub started: Instant,
}

/// The answer to an operation: a value for a read, the value written for a write, or an error
#[derive(Debug, Clone)]
pub struct Outcome {
    pub cv: u16,
    pub kind: &'static str,
    pub value: Option<u8>,
    pub error: Option<String>,
}

impl Programming {
    pub fn start(&mut self, cv: u16, kind: &'static str, value: Option<u8>, now: Instant) {
        self.pending = Some(Pending { cv, kind, value, started: now });
    }

    /// The Z21 answered a read or a write with a value
    pub fn finish_with_value(&mut self, cv: u16, value: u8) {
        let kind = self.pending.as_ref().map_or("read", |pending| pending.kind);
        self.pending = None;
        self.last = Some(Outcome { cv, kind, value: Some(value), error: None });
    }

    /// The Z21 refused, or no answer came
    pub fn finish_with_error(&mut self, error: &str) {
        if let Some(pending) = self.pending.take() {
            self.last = Some(Outcome { cv: pending.cv, kind: pending.kind, value: None, error: Some(error.into()) });
        }
    }
}

fn set_central_state(z21: &mut Z21, byte: u8) {
    z21.central_state = Some(replies::central_state(byte));
}
