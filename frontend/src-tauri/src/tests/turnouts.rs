//! Turnout settings: the switching time, manual, position feedback, group and notes kept on each placed turnout

use std::time::{Duration, Instant};

use serde_json::json;

use super::fake_z21::fake_z21;
use super::{app, detail, layout, send};
use crate::library::library;
use crate::z21::commands;
use crate::z21::{self, frames::build_frame};

/// The id of a turnout and of a track piece in the library, as a layout saves them
fn piece_ids() -> (String, String) {
    let pieces = library().list(None, None, None);
    let turnout = pieces.iter().find(|piece| piece["type"] == "turnout").expect("a turnout in the library")["id"].as_str().unwrap().to_string();
    (turnout, "peco-n-sl-300f".to_string())
}

/// A layout with a turnout that has an address (t1), a turnout without one (t2), and a track piece (s1)
fn with_pieces(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64) {
    let (turnout, track) = piece_ids();
    let pieces = json!([
        { "id": "t1", "piece_id": turnout, "x": 10, "y": 10, "rotation": 0, "flip": false, "dcc_address": 12 },
        { "id": "t2", "piece_id": turnout, "x": 40, "y": 10, "rotation": 0, "flip": false },
        { "id": "s1", "piece_id": track, "x": 70, "y": 10, "rotation": 0, "flip": false }
    ]);
    let (status, value) = send(app, "PATCH", &format!("/api/layouts/{layout_id}"), Some(json!({ "elements": { "track_system": "Code 55", "track_pieces": pieces } })));
    assert_eq!(status, 200, "{value}");
}

#[test]
fn a_turnout_has_the_defaults_until_its_settings_are_changed() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    let (_, rows) = send(&app, "GET", "/api/overview/turnouts", None);
    let first = &rows[0];
    assert_eq!(first["placed_id"], "t1");
    assert_eq!(first["settings"]["throw_ms"], 300);
    assert_eq!(first["settings"]["manual"], false);
    assert_eq!(first["settings"]["feedback"], false);
    assert_eq!(first["settings"]["group"], "");
    assert_eq!(first["settings"]["notes"], "");
    assert!(first["settings"]["command_station"].is_null(), "no station until one is chosen");
    assert!(["left", "right"].contains(&first["settings"]["turnout_type"].as_str().unwrap()), "the type comes from the library's hand");
    assert!(first["position"].is_null());
}

#[test]
fn only_the_settings_sent_are_changed_and_they_show_in_the_list() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    let (status, value) = send(
        &app,
        "PATCH",
        &format!("/api/layouts/{id}/turnouts/t1"),
        Some(json!({ "throw_ms": 600, "group": " Yard ", "notes": "  spring is stiff  " })),
    );
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["settings"]["throw_ms"], 600);
    assert_eq!(value["settings"]["group"], "Yard");
    assert_eq!(value["settings"]["notes"], "spring is stiff");
    assert_eq!(value["settings"]["manual"], false, "a setting not sent keeps its value");

    let (_, rows) = send(&app, "GET", "/api/overview/turnouts", None);
    assert_eq!(rows[0]["settings"]["throw_ms"], 600);
    assert_eq!(rows[0]["dcc_address"], 12, "the address is unchanged");
    assert_eq!(rows[1]["settings"]["throw_ms"], 300, "the other turnout is unchanged");
}

#[test]
fn settings_out_of_their_limits_are_refused() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    let path = format!("/api/layouts/{id}/turnouts/t1");
    let (status, value) = send(&app, "PATCH", &path, Some(json!({ "throw_ms": 10 })));
    assert_eq!(status, 422, "{value}");
    assert_eq!(detail(&value), "Switching time must be 50 to 5000 ms");
    assert_eq!(send(&app, "PATCH", &path, Some(json!({ "throw_ms": 6000 }))).0, 422);
    assert_eq!(send(&app, "PATCH", &path, Some(json!({ "group": "x".repeat(41) }))).0, 422);
    assert_eq!(send(&app, "PATCH", &path, Some(json!({ "notes": "x".repeat(501) }))).0, 422);
}

#[test]
fn only_a_turnout_on_the_layout_can_be_changed() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/s1"), Some(json!({ "manual": true }))).0, 404, "a track piece is not a turnout");
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t9"), Some(json!({ "manual": true }))).0, 404);
    assert_eq!(send(&app, "PATCH", "/api/layouts/999/turnouts/t1", Some(json!({ "manual": true }))).0, 404);
}

#[test]
fn a_throw_takes_the_switching_time_set_on_its_turnout() {
    let fake = fake_z21();
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t1"), Some(json!({ "throw_ms": 800 })));
    assert!(z21::connect(&app, "127.0.0.1", fake.port));
    {
        // Track power on, so the track is ready to throw a turnout
        let mut inner = app.inner.lock().unwrap();
        z21::apply(&mut inner, &build_frame(0x0040, &[0x62, 0x22, 0x00, 0x40]), Instant::now());
    }

    let started = Instant::now();
    let (status, value) = send(&app, "POST", "/api/z21/turnouts/12/throw", Some(json!({ "output": 0 })));
    assert_eq!(status, 200, "{value}");
    // The default is 300 ms, so waiting at least 800 ms shows the turnout's own time was used
    assert!(started.elapsed() >= Duration::from_millis(800), "took {:?}", started.elapsed());
    app.inner.lock().unwrap().z21.disconnect();
}

/// A command station on this device, with the address of the fake Z21
fn station(app: &std::sync::Arc<crate::state::AppState>, name: &str, port: &str) {
    let (status, value) = send(app, "POST", "/api/command-stations", Some(json!({ "name": name, "port": port })));
    assert_eq!(status, 201, "{value}");
}

/// Connect the app to the fake Z21, and set its track to ready
fn connect_ready(app: &std::sync::Arc<crate::state::AppState>, fake: &super::fake_z21::FakeZ21) {
    assert!(z21::connect(app, "127.0.0.1", fake.port));
    let mut inner = app.inner.lock().unwrap();
    z21::apply(&mut inner, &build_frame(0x0040, &[0x62, 0x22, 0x00, 0x40]), Instant::now());
}

#[test]
fn a_turnout_type_is_one_of_the_plan_types() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t2"), Some(json!({ "turnout_type": "three-way" })));
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["settings"]["turnout_type"], "three-way");
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t2"), Some(json!({ "turnout_type": "hexagon" })));
    assert_eq!(status, 422);
    assert!(detail(&value).starts_with("Turnout type must be one of: straight, left, right, three-way"), "{value}");
}

#[test]
fn a_turnout_names_a_command_station_this_device_has() {
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    station(&app, "Shed", "10.0.0.111:21105");
    let path = format!("/api/layouts/{id}/turnouts/t1");
    let (status, value) = send(&app, "PATCH", &path, Some(json!({ "command_station": "Yard" })));
    assert_eq!(status, 422, "{value}");
    assert_eq!(detail(&value), "No command station called Yard on this device");
    assert_eq!(send(&app, "PATCH", &path, Some(json!({ "command_station": "Shed" }))).1["settings"]["command_station"], "Shed");
    assert!(send(&app, "PATCH", &path, Some(json!({ "command_station": null }))).1["settings"]["command_station"].is_null());
}

#[test]
fn a_throw_is_refused_when_its_command_station_is_not_the_one_connected() {
    let fake = fake_z21();
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    station(&app, "Yard", "10.0.0.112:21105");
    send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t1"), Some(json!({ "command_station": "Yard" })));
    connect_ready(&app, &fake);
    let (status, value) = send(&app, "POST", "/api/z21/turnouts/12/throw", Some(json!({ "output": 0 })));
    assert_eq!(status, 409, "{value}");
    assert_eq!(detail(&value), "Turnout 12 is on command station Yard, which is not the one connected");
    app.inner.lock().unwrap().z21.disconnect();
}

#[test]
fn a_throw_goes_ahead_on_the_command_station_it_is_set_to() {
    let fake = fake_z21();
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    station(&app, "Shed", &format!("127.0.0.1:{}", fake.port));
    send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t1"), Some(json!({ "command_station": "Shed", "throw_ms": 50 })));
    connect_ready(&app, &fake);
    let (status, value) = send(&app, "POST", "/api/z21/turnouts/12/throw", Some(json!({ "output": 0 })));
    assert_eq!(status, 200, "{value}");
    app.inner.lock().unwrap().z21.disconnect();
}

#[test]
fn a_turnout_with_position_feedback_asks_for_its_position_after_a_throw_and_lists_it() {
    let fake = fake_z21();
    let app = app();
    let id = layout(&app, "Main");
    with_pieces(&app, id);
    send(&app, "PATCH", &format!("/api/layouts/{id}/turnouts/t1"), Some(json!({ "feedback": true, "throw_ms": 50 })));
    connect_ready(&app, &fake);
    let (status, value) = send(&app, "POST", "/api/z21/turnouts/12/throw", Some(json!({ "output": 0 })));
    assert_eq!(status, 200, "{value}");
    assert!(fake.wait_for(&commands::turnout_info_request(12).unwrap()), "the Z21 is asked for the turnout's position");

    app.inner.lock().unwrap().z21.turnouts.insert(12, Some(2));
    let (_, rows) = send(&app, "GET", "/api/overview/turnouts", None);
    assert_eq!(rows[0]["position"], 2, "the reported position is listed");
    assert!(rows[1]["position"].is_null(), "a turnout without feedback shows no position");
    app.inner.lock().unwrap().z21.disconnect();
}
