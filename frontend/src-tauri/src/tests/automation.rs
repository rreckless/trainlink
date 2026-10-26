use std::time::{Duration, Instant};

use serde_json::json;

use super::{app, detail, layout, send};
use crate::z21::replies::CentralState;

fn block(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64, name: &str, max_speed: i64) -> i64 {
    let (_, value) = send(app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": name, "x": 0, "y": 0, "max_speed": max_speed })));
    value["id"].as_i64().unwrap()
}

/// A Z21 that is connected with track power on, so runs are allowed
fn ready(app: &std::sync::Arc<crate::state::AppState>) {
    let mut inner = app.inner.lock().unwrap();
    inner.z21.connected = true;
    inner.z21.central_state = Some(CentralState { emergency_stop: false, track_power_on: true, short_circuit: false });
}

fn loco(app: &std::sync::Arc<crate::state::AppState>) -> i64 {
    let (_, value) = send(app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    value["id"].as_i64().unwrap()
}

/// Mark a block occupied, as a train entering it would
fn occupy(app: &std::sync::Arc<crate::state::AppState>, block_id: i64) {
    let (status, value) = send(app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "state": "occupied" })));
    assert_eq!(status, 200, "{value}");
}

fn tick(app: &std::sync::Arc<crate::state::AppState>, at: Instant) {
    let mut inner = app.inner.lock().unwrap();
    crate::automation::tick(&mut inner, at).unwrap();
}

fn route_state(app: &std::sync::Arc<crate::state::AppState>, id: i64) -> serde_json::Value {
    send(app, "GET", &format!("/api/routes/{id}"), None).1
}

#[test]
fn a_run_moves_from_block_to_block_as_the_train_enters_each() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let c = block(&app, layout_id, "C", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Up", "block_sequence": [a, b, c] })));
    let route_id = route["id"].as_i64().unwrap();
    let loco_id = loco(&app);

    let (status, started) = send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id })));
    assert_eq!(status, 200, "{started}");
    assert_eq!(started["is_running"], true);
    assert_eq!(started["current_block_index"], 0);

    let now = Instant::now();
    tick(&app, now);
    assert_eq!(route_state(&app, route_id)["current_block_index"], 0, "nothing entered yet");

    occupy(&app, b);
    tick(&app, now + Duration::from_secs(1));
    assert_eq!(route_state(&app, route_id)["current_block_index"], 1, "the train entered B");

    occupy(&app, c);
    tick(&app, now + Duration::from_secs(2));
    let finished = route_state(&app, route_id);
    assert_eq!(finished["is_running"], false, "the route ends at its last block");
}

#[test]
fn a_dwell_holds_the_train_before_it_moves_on() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let c = block(&app, layout_id, "C", 60);
    let (status, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Dwell", "block_sequence": [a, b, c], "dwell_times": [0, 5, 0] })));
    assert_eq!(status, 201, "{route}");
    let route_id = route["id"].as_i64().unwrap();
    send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco(&app) })));

    let now = Instant::now();
    occupy(&app, b);
    tick(&app, now);
    occupy(&app, c);
    tick(&app, now + Duration::from_secs(2));
    assert_eq!(route_state(&app, route_id)["current_block_index"], 1, "still dwelling in B");

    tick(&app, now + Duration::from_secs(6));
    assert_eq!(route_state(&app, route_id)["current_block_index"], 2, "moves on after the dwell");
}

#[test]
fn auto_return_turns_the_train_round_at_the_end() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Shuttle", "block_sequence": [a, b], "auto_return": true })));
    let route_id = route["id"].as_i64().unwrap();
    send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco(&app) })));

    let now = Instant::now();
    occupy(&app, b);
    tick(&app, now);
    assert_eq!(route_state(&app, route_id)["is_running"], true, "auto-return keeps the route running");

    // Back in A: the train is in A once B is clear and A is occupied
    send(&app, "PATCH", &format!("/api/blocks/{b}"), Some(json!({ "state": "free" })));
    occupy(&app, a);
    tick(&app, now + Duration::from_secs(1));
    let back = route_state(&app, route_id);
    assert_eq!(back["is_running"], true);
    assert_eq!(back["current_block_index"], 0, "the train is back at the start");
}

#[test]
fn stop_ends_a_run_and_a_run_cannot_start_twice() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Stop", "block_sequence": [a, b] })));
    let route_id = route["id"].as_i64().unwrap();
    let loco_id = loco(&app);
    send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id })));

    let (status, value) = send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id })));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "The route is already running");

    let (status, stopped) = send(&app, "POST", &format!("/api/routes/{route_id}/stop"), None);
    assert_eq!(status, 200);
    assert_eq!(stopped["is_running"], false);
    assert_eq!(send(&app, "POST", &format!("/api/routes/{route_id}/stop"), None).0, 409);
}

#[test]
fn a_run_needs_a_ready_track_an_enabled_route_and_a_locomotive() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Gated", "block_sequence": [a] })));
    let route_id = route["id"].as_i64().unwrap();
    let loco_id = loco(&app);
    assert_eq!(send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id }))).0, 503);

    ready(&app);
    send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "enabled": false })));
    let (status, value) = send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id })));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "The route is disabled");
    send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "enabled": true })));
    assert_eq!(send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": 999 }))).0, 404);
}

#[test]
fn a_run_that_was_not_started_in_this_session_is_stopped() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Lost", "block_sequence": [a] })));
    let route_id = route["id"].as_i64().unwrap();
    send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco(&app) })));
    app.inner.lock().unwrap().runs.clear();
    tick(&app, Instant::now());
    assert_eq!(route_state(&app, route_id)["is_running"], false);
}

#[test]
fn speed_and_dwell_options_are_checked_against_the_blocks() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let body = |profile: serde_json::Value| json!({ "layout_id": layout_id, "name": "Opts", "block_sequence": [a, b], "speed_profile": profile });
    assert_eq!(send(&app, "POST", "/api/routes", Some(body(json!([40])))).0, 422, "one entry per block");
    assert_eq!(send(&app, "POST", "/api/routes", Some(body(json!([40, 200])))).0, 422, "speeds stop at 126");
    let (status, route) = send(&app, "POST", "/api/routes", Some(body(json!([40, null])))).clone();
    assert_eq!(status, 201, "{route}");
    assert_eq!(route["speed_profile"], json!([40, null]), "null means the block's own speed");
    let route_id = route["id"].as_i64().unwrap();
    let (status, _) = send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "dwell_times": [1, -2] })));
    assert_eq!(status, 422);
    let (status, _) = send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "auto_return": true, "dwell_times": [1, 3] })));
    assert_eq!(status, 200);
}

#[test]
fn a_running_route_cannot_be_changed() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A", 60);
    let b = block(&app, layout_id, "B", 60);
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Busy", "block_sequence": [a, b] })));
    let route_id = route["id"].as_i64().unwrap();
    send(&app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco(&app) })));
    let (status, value) = send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "block_sequence": [a] })));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Stop the route before changing its blocks or options");
}
