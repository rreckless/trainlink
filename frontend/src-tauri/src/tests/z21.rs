use serde_json::json;

use super::{app, send};
use crate::z21::{self, frames::build_frame, replies};

fn set_connected(app: &std::sync::Arc<crate::state::AppState>, connected: bool) {
    app.inner.lock().unwrap().z21.connected = connected;
}

#[test]
fn status_when_disconnected() {
    let app = app();
    let (status, value) = send(&app, "GET", "/api/z21/status", None);
    assert_eq!(status, 200);
    assert_eq!(value["connected"], false);
    assert_eq!(value["state"], "disconnected");
    assert!(value["central_state"].is_null());
    assert_eq!(value["port"], 21105);
}

#[test]
fn commands_refused_while_disconnected() {
    let app = app();
    assert_eq!(send(&app, "POST", "/api/z21/track-power/on", None).0, 503);
    assert_eq!(send(&app, "POST", "/api/z21/emergency-stop", Some(json!({}))).0, 503);
    assert_eq!(send(&app, "POST", "/api/z21/turnouts/5/throw", Some(json!({ "output": 1 }))).0, 503);
    assert_eq!(send(&app, "POST", "/api/z21/loco/speed", Some(json!({ "address": 3, "speed": 10 }))).0, 503);
    let (status, _) = send(&app, "POST", "/api/z21/emergency-stop", Some(json!({ "confirm": false })));
    assert_eq!(status, 400);
}

#[test]
fn list_inputs_are_checked_before_any_command() {
    let app = app();
    let (status, value) = send(&app, "GET", "/api/z21/turnouts?addresses=x", None);
    assert_eq!(status, 400);
    assert_eq!(value["detail"], "Turnouts must be numbers");
    assert_eq!(send(&app, "GET", "/api/z21/turnouts?addresses=0", None).0, 400);
    assert_eq!(send(&app, "GET", "/api/z21/locos?addresses=10000", None).0, 400);
    let (status, value) = send(&app, "GET", "/api/z21/turnouts?addresses=5,6", None);
    assert_eq!(status, 200);
    assert_eq!(value["states"], json!({ "5": null, "6": null }));
}

#[test]
fn speed_and_throw_checks_in_order() {
    let app = app();
    set_connected(&app, true);
    // Track state is unknown until the Z21 reports it, so commands are blocked
    let (status, value) = send(&app, "POST", "/api/z21/loco/speed", Some(json!({ "address": 3, "speed": 10 })));
    assert_eq!(status, 409);
    assert!(value["detail"].as_str().unwrap().starts_with("Track is not ready (unknown)"));
    let (status, _) = send(&app, "POST", "/api/z21/turnouts/5/throw", Some(json!({ "output": 2 })));
    assert_eq!(status, 422);
}

fn central_state_frame(state: u8) -> Vec<u8> {
    let mut xbus = vec![0x62, 0x22, state];
    xbus.push(xbus.iter().fold(0, |acc, byte| acc ^ byte));
    build_frame(0x0040, &xbus)
}

#[test]
fn central_state_replies_set_the_track_state() {
    let app = app();
    set_connected(&app, true);
    let apply = |frame: &[u8]| {
        let mut inner = app.inner.lock().unwrap();
        z21::apply(&mut inner, frame, std::time::Instant::now());
        inner.z21.track_state()
    };
    assert_eq!(apply(&central_state_frame(0x00)), "ready");
    assert_eq!(apply(&central_state_frame(0x02)), "track_power_off");
    assert_eq!(apply(&central_state_frame(0x04)), "short_circuit");
}

#[test]
fn loco_info_reply_records_speed_direction_and_functions() {
    let app = app();
    set_connected(&app, true);
    // Address 3, 128 steps, UI speed 10 (step 11), forward, F0 and F1 on
    let mut xbus = vec![0xEF, 0x00, 0x03, 0x04, 0x80 | 11, 0x11, 0x00, 0x00, 0x00];
    xbus.push(xbus.iter().fold(0, |acc, byte| acc ^ byte));
    let frame = build_frame(0x0040, &xbus);
    let mut inner = app.inner.lock().unwrap();
    z21::apply(&mut inner, &frame, std::time::Instant::now());
    let state = inner.z21.locos.get(&3).expect("address 3 reported");
    assert_eq!(state, &replies::LocoState { speed: Some(10), direction: "forward", functions: vec![0, 1] });
}

#[test]
fn turnout_and_feedback_replies_are_stored() {
    let app = app();
    let mut turnout = vec![0x43, 0x00, 0x04, 0x01];
    turnout.push(turnout.iter().fold(0, |acc, byte| acc ^ byte));
    let frame = build_frame(0x0040, &turnout);
    let mut inner = app.inner.lock().unwrap();
    z21::apply(&mut inner, &frame, std::time::Instant::now());
    assert_eq!(inner.z21.turnouts.get(&5), Some(&Some(0)));

    // Group 0, module 1 has inputs 1 and 3 occupied
    let frame = build_frame(0x0080, &[0, 0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z21::apply(&mut inner, &frame, std::time::Instant::now());
    assert_eq!(inner.z21.feedback.get(&(1, 1)), Some(&true));
    assert_eq!(inner.z21.feedback.get(&(1, 2)), Some(&false));
    assert_eq!(inner.z21.feedback.get(&(1, 3)), Some(&true));
}

#[test]
fn feedback_frame_is_refused_unless_it_has_ten_module_bytes() {
    assert!(replies::rbus_feedback(0, &[0; 9]).is_none());
    assert_eq!(replies::rbus_feedback(1, &[0; 10]).unwrap().len(), 80);
}

#[test]
fn the_z21_must_be_on_this_computer_or_the_local_network() {
    use crate::z21::is_local_address;
    assert!(is_local_address("10.0.0.111"));
    assert!(is_local_address("192.168.1.20"));
    assert!(is_local_address("172.16.4.5"));
    assert!(is_local_address("127.0.0.1"));
    assert!(is_local_address("localhost"));
    assert!(is_local_address("fd00::5"));
    assert!(!is_local_address("8.8.8.8"));
    assert!(!is_local_address("203.0.113.9"));
    assert!(!is_local_address("example.com"));
}
