//! The Z21 link against a fake Z21 on this machine. Commands are checked byte for byte against the Python
//! protocol code, and replies are sent back the way the Z21 sends them. No packet reaches the real command station

use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::{app, detail, send};
use crate::state::AppState;
use crate::z21::commands::{self, status_request};
use crate::z21::{self, frames::build_frame};

/// Frames the fake Z21 received, and a way to reply to the app
struct FakeZ21 {
    port: u16,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
    reply: Arc<Mutex<Option<std::net::SocketAddr>>>,
    socket: Arc<UdpSocket>,
}

fn fake_z21() -> FakeZ21 {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").expect("bind fake Z21"));
    socket.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let port = socket.local_addr().expect("address").port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let reply = Arc::new(Mutex::new(None));
    let (socket_clone, received_clone, reply_clone) = (socket.clone(), received.clone(), reply.clone());
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        loop {
            match socket_clone.recv_from(&mut buffer) {
                Ok((length, from)) => {
                    *reply_clone.lock().unwrap() = Some(from);
                    received_clone.lock().unwrap().push(buffer[..length].to_vec());
                }
                Err(_) => {
                    if Arc::strong_count(&socket_clone) <= 1 {
                        break;
                    }
                }
            }
        }
    });
    FakeZ21 { port, received, reply, socket }
}

impl FakeZ21 {
    /// Send a frame to the app as the Z21 would
    fn say(&self, frame: &[u8]) {
        // The app's address is known once it has sent its first frame, so wait for that
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let to = *self.reply.lock().unwrap();
            if let Some(to) = to {
                self.socket.send_to(frame, to).expect("send to app");
                return;
            }
            assert!(Instant::now() < deadline, "the app never sent a frame to the fake Z21");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn saw(&self, expected: &[u8]) -> bool {
        self.received.lock().unwrap().iter().any(|frame| frame == expected)
    }

    /// Wait until the fake has received this frame, or give up after two seconds
    fn wait_for(&self, expected: &[u8]) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.saw(expected) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn xbus_frame(header: u16, xbus: &[u8]) -> Vec<u8> {
    let mut data = xbus.to_vec();
    data.push(xbus.iter().fold(0, |acc, byte| acc ^ byte));
    build_frame(header, &data)
}

fn central_state(state: u8) -> Vec<u8> {
    xbus_frame(0x0040, &[0x62, 0x22, state])
}

fn track_state(app: &Arc<AppState>) -> String {
    app.inner.lock().unwrap().z21.track_state().to_string()
}

#[test]
fn connecting_asks_for_status_and_subscribes_to_broadcasts() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    assert!(fake.wait_for(&[0x08, 0x00, 0x40, 0x00, 0x21, 0x24, 0x05, 0x00]), "status request");
    assert!(fake.wait_for(&[0x08, 0x00, 0x50, 0x00, 0x03, 0x01, 0x00, 0x00]), "broadcast flags");
    assert!(fake.wait_for(&[0x05, 0x00, 0x81, 0x00, 0x00]), "feedback group 0");
    assert!(fake.wait_for(&[0x05, 0x00, 0x81, 0x00, 0x01]), "feedback group 1");
    assert!(fake.wait_for(&[0x07, 0x00, 0x40, 0x00, 0x21, 0x21, 0x00]), "X-Bus version");
    assert_eq!(track_state(&app), "unknown");
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));
}

#[test]
fn track_commands_reach_the_z21_with_the_python_bytes() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));

    let (status, _) = send(&app, "POST", "/api/z21/loco/speed", Some(json!({ "address": 3, "speed": 50, "direction": true })));
    assert_eq!(status, 200);
    assert!(fake.wait_for(&[0x0A, 0x00, 0x40, 0x00, 0xE4, 0x13, 0x00, 0x03, 0xB3, 0x47]), "loco drive");

    let (status, _) = send(&app, "POST", "/api/z21/loco/function", Some(json!({ "address": 3, "function": 0, "on": true })));
    assert_eq!(status, 200);
    assert!(fake.wait_for(&[0x0A, 0x00, 0x40, 0x00, 0xE4, 0xF8, 0x00, 0x03, 0x40, 0x5F]), "function");

    let (status, _) = send(&app, "POST", "/api/z21/emergency-stop", Some(json!({})));
    assert_eq!(status, 200);
    assert!(fake.wait_for(&[0x06, 0x00, 0x40, 0x00, 0x80, 0x80]), "emergency stop");
}

#[test]
fn a_throw_is_switched_on_then_off_after_the_throw_time() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));

    let (status, body) = send(&app, "POST", "/api/z21/turnouts/25/throw", Some(json!({ "output": 1 })));
    assert_eq!(status, 200, "{body}");
    assert!(fake.wait_for(&[0x09, 0x00, 0x40, 0x00, 0x53, 0x00, 0x18, 0xA9, 0xE2]), "activate");
    assert!(fake.wait_for(&[0x09, 0x00, 0x40, 0x00, 0x53, 0x00, 0x18, 0xA1, 0xEA]), "deactivate");
}

#[test]
fn replies_update_turnouts_locomotives_and_track_power() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));

    // Turnout 25 reports output 1 (ZZ = 10)
    fake.say(&xbus_frame(0x0040, &[0x43, 0x00, 0x18, 0x02]));
    let (_, turnouts) = send(&app, "GET", "/api/z21/turnouts?addresses=25", None);
    assert!(wait_until(|| {
        let (_, value) = send(&app, "GET", "/api/z21/turnouts?addresses=25", None);
        value["states"]["25"] == json!(1)
    }), "turnout state {turnouts}");

    // Locomotive 3 at UI speed 10, forward, with F0 on
    fake.say(&xbus_frame(0x0040, &[0xEF, 0x00, 0x03, 0x04, 0x80 | 11, 0x10, 0x00, 0x00, 0x00]));
    assert!(wait_until(|| {
        let (_, value) = send(&app, "GET", "/api/z21/locos?addresses=3", None);
        value["states"]["3"]["speed"] == json!(10)
    }));
    let (_, locos) = send(&app, "GET", "/api/z21/locos?addresses=3", None);
    assert_eq!(locos["states"]["3"], json!({ "speed": 10, "direction": "forward", "functions": [0] }));

    // Short circuit blocks track power on and turnout commands
    fake.say(&central_state(0x04));
    assert!(wait_until(|| track_state(&app) == "short_circuit"));
    let (status, _) = send(&app, "POST", "/api/z21/track-power/on", None);
    assert_eq!(status, 409);
    let (status, _) = send(&app, "POST", "/api/z21/turnouts/25/throw", Some(json!({ "output": 0 })));
    assert_eq!(status, 409);
}

#[test]
fn disconnecting_forgets_what_the_z21_reported() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));
    let (status, value) = send(&app, "POST", "/api/z21/disconnect", None);
    assert_eq!((status, value["status"].as_str()), (200, Some("disconnected")));
    assert_eq!(track_state(&app), "disconnected");
    let (status, _) = send(&app, "POST", "/api/z21/loco/speed", Some(json!({ "address": 3, "speed": 10 })));
    assert_eq!(status, 503);
}

#[test]
fn a_turnout_poll_answers_with_the_state_the_z21_sends_back_to_it() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    fake.say(&central_state(0x00));
    assert!(wait_until(|| track_state(&app) == "ready"));

    // The Z21 answers the app's request for turnout 25 with output 1 (ZZ = 10), as it does on the LAN
    let request = xbus_frame(0x0040, &[0x43, 0x00, 0x18]);
    let answer = xbus_frame(0x0040, &[0x43, 0x00, 0x18, 0x02]);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert!(fake.wait_for(&request));
            fake.say(&answer);
        });
        let (_, turnouts) = send(&app, "GET", "/api/z21/turnouts?addresses=25", None);
        assert_eq!(turnouts["states"]["25"], json!(1), "the first answer already carries the Z21's reply");
    });
}

#[test]
fn the_app_keeps_polling_the_z21_so_it_stays_on_its_client_list() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect_with_keep_alive(&app, "127.0.0.1", fake.port, Duration::from_millis(50)));
    let status = status_request();
    let polls = || fake.received.lock().unwrap().iter().filter(|frame| frame.as_slice() == status.as_slice()).count();
    // The first status request comes with the connection. The rest are polls, sent with no reply from the Z21
    assert!(wait_until(|| polls() >= 3), "status requests keep coming");
}

#[test]
fn a_link_silent_for_half_a_minute_is_lost_and_refuses_commands() {
    let app = app();
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.connected = true;
        z21::apply(&mut inner, &central_state(0x00), Instant::now());
    }
    assert_eq!(track_state(&app), "ready");

    app.inner.lock().unwrap().z21.last_reply = Instant::now() - Duration::from_secs(31);
    assert_eq!(track_state(&app), "lost");
    let (status, value) = send(&app, "POST", "/api/programming/read", Some(json!({ "cv": 29 })));
    assert_eq!(status, 409, "{value}");
    assert!(detail(&value).contains("(lost)"), "{value}");

    // Any frame from the Z21 brings the link back
    {
        let mut inner = app.inner.lock().unwrap();
        z21::apply(&mut inner, &central_state(0x00), Instant::now());
    }
    assert_eq!(track_state(&app), "ready");
}

#[test]
fn the_watchdog_reconnects_a_lost_link_to_the_same_z21() {
    let fake = fake_z21();
    let app = app();
    assert!(z21::connect_with_keep_alive(&app, "127.0.0.1", fake.port, Duration::from_secs(60)));
    let status = status_request();
    let asked = || fake.received.lock().unwrap().iter().filter(|frame| frame.as_slice() == status.as_slice()).count();
    let before = asked();
    assert!(!z21::link_check(&app, Instant::now()), "a healthy link is left alone");

    app.inner.lock().unwrap().z21.last_reply = Instant::now() - Duration::from_secs(31);
    assert!(z21::link_check(&app, Instant::now()));
    assert!(wait_until(|| asked() > before), "the new socket asks the Z21 for its status");
    assert_eq!(track_state(&app), "unknown", "connected again, waiting for the Z21's track state");
}

#[test]
fn nothing_is_reconnected_after_a_disconnect() {
    let app = app();
    app.inner.lock().unwrap().z21.last_reply = Instant::now() - Duration::from_secs(60);
    assert!(!z21::link_check(&app, Instant::now()));
}

#[test]
fn track_power_off_and_the_version_request_are_the_frames_the_spec_gives() {
    // Spec 2.3 and 2.5: X-Header 0x21 with DB0 0x80 for power off, and 0x21 0x21 for the version, each with its XOR
    assert_eq!(commands::track_power_off(), vec![0x07, 0x00, 0x40, 0x00, 0x21, 0x80, 0xA1]);
    assert_eq!(commands::version_request(), vec![0x07, 0x00, 0x40, 0x00, 0x21, 0x21, 0x00]);
}

#[test]
fn the_version_the_z21_reports_is_kept_and_shown_in_the_status() {
    let app = app();
    assert!(send(&app, "GET", "/api/z21/status", None).1["xbus_version"].is_null());
    {
        let mut inner = app.inner.lock().unwrap();
        // Reply: X-Bus version 4.0, command station ID 0x12, the Z21 family
        z21::apply(&mut inner, &xbus_frame(0x0040, &[0x63, 0x21, 0x40, 0x12]), Instant::now());
    }
    let (_, value) = send(&app, "GET", "/api/z21/status", None);
    assert_eq!(value["xbus_version"], "4.0");
    assert_eq!(value["station_id"], 0x12);
}

#[test]
fn track_power_can_be_cut_on_a_connected_z21() {
    let fake = fake_z21();
    let app = app();
    assert_eq!(send(&app, "POST", "/api/z21/track-power/off", None).0, 503, "not connected yet");
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    let (status, value) = send(&app, "POST", "/api/z21/track-power/off", None);
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["status"], "track_power_off_sent");
    assert!(fake.wait_for(&[0x07, 0x00, 0x40, 0x00, 0x21, 0x80, 0xA1]), "track power off reaches the Z21");
}
