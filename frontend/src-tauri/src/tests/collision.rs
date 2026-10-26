use std::time::Instant;

use serde_json::json;

use super::{app, layout, send};
use crate::z21::replies::CentralState;

fn ready(app: &std::sync::Arc<crate::state::AppState>) {
    let mut inner = app.inner.lock().unwrap();
    inner.z21.connected = true;
    inner.z21.central_state = Some(CentralState { emergency_stop: false, track_power_on: true, short_circuit: false });
}

fn block(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64, name: &str) -> i64 {
    let (_, value) = send(app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": name, "x": 0, "y": 0 })));
    value["id"].as_i64().unwrap()
}

fn loco(app: &std::sync::Arc<crate::state::AppState>, name: &str, address: i64) -> i64 {
    let (_, value) = send(app, "POST", "/api/locomotives", Some(json!({ "name": name, "dcc_address": address, "locomotive_type": "diesel" })));
    value["id"].as_i64().unwrap()
}

fn route(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64, name: &str, blocks: &[i64]) -> i64 {
    let (status, value) = send(app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": name, "block_sequence": blocks })));
    assert_eq!(status, 201, "{value}");
    value["id"].as_i64().unwrap()
}

fn start(app: &std::sync::Arc<crate::state::AppState>, route_id: i64, loco_id: i64) -> (u16, serde_json::Value) {
    send(app, "POST", &format!("/api/routes/{route_id}/run"), Some(json!({ "locomotive_id": loco_id })))
}

fn tick(app: &std::sync::Arc<crate::state::AppState>) {
    let mut inner = app.inner.lock().unwrap();
    crate::automation::tick(&mut inner, Instant::now()).unwrap();
}

fn state_of(app: &std::sync::Arc<crate::state::AppState>, id: i64) -> serde_json::Value {
    send(app, "GET", &format!("/api/routes/{id}"), None).1
}

#[test]
fn a_block_one_run_holds_is_not_given_to_another_run() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let (a, b, c) = (block(&app, layout_id, "A"), block(&app, layout_id, "B"), block(&app, layout_id, "C"));
    let first = route(&app, layout_id, "First", &[a, b]);
    let second = route(&app, layout_id, "Second", &[c, b]);
    assert_eq!(start(&app, first, loco(&app, "One", 3)).0, 200);

    let (status, waiting) = start(&app, second, loco(&app, "Two", 4));
    assert_eq!(status, 200, "{waiting}");
    assert_eq!(waiting["is_running"], true);
    assert_eq!(waiting["waiting_for_block"], b, "the second run waits for B, which the first holds");
    assert_eq!(state_of(&app, first)["waiting_for_block"], serde_json::Value::Null);
}

#[test]
fn a_run_starts_in_a_block_another_run_holds_only_if_it_is_free() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let (a, b) = (block(&app, layout_id, "A"), block(&app, layout_id, "B"));
    let first = route(&app, layout_id, "First", &[a, b]);
    let second = route(&app, layout_id, "Second", &[a]);
    assert_eq!(start(&app, first, loco(&app, "One", 3)).0, 200);
    let (status, value) = start(&app, second, loco(&app, "Two", 4));
    assert_eq!(status, 409, "{value}");
    assert_eq!(value["detail"], "The first block is in use by the run First");
}

#[test]
fn a_run_does_not_take_an_occupied_block_it_has_not_claimed() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let (a, b) = (block(&app, layout_id, "A"), block(&app, layout_id, "B"));
    send(&app, "PATCH", &format!("/api/blocks/{b}"), Some(json!({ "state": "occupied" })));
    let run = route(&app, layout_id, "Blocked", &[a, b]);
    let (status, started) = start(&app, run, loco(&app, "One", 3));
    assert_eq!(status, 200, "{started}");
    assert_eq!(started["waiting_for_block"], b, "B is occupied by something that is not this train");
    tick(&app);
    let after = state_of(&app, run);
    assert_eq!(after["current_block_index"], 0, "the train did not move into B");
    assert_eq!(after["is_running"], true);
}

#[test]
fn a_waiting_run_moves_on_once_the_block_is_released() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let (a, b, c) = (block(&app, layout_id, "A"), block(&app, layout_id, "B"), block(&app, layout_id, "C"));
    let first = route(&app, layout_id, "First", &[a, b]);
    let second = route(&app, layout_id, "Second", &[c, b]);
    start(&app, first, loco(&app, "One", 3));
    start(&app, second, loco(&app, "Two", 4));

    send(&app, "PATCH", &format!("/api/blocks/{b}"), Some(json!({ "state": "occupied" })));
    tick(&app);
    assert_eq!(state_of(&app, first)["current_block_index"], 1, "the first train is in B");
    assert_eq!(state_of(&app, second)["waiting_for_block"], b, "the second run still waits for B");

    send(&app, "POST", &format!("/api/routes/{first}/stop"), None);
    send(&app, "PATCH", &format!("/api/blocks/{b}"), Some(json!({ "state": "free" })));
    tick(&app);
    assert_eq!(state_of(&app, second)["waiting_for_block"], serde_json::Value::Null, "B is now the second run's");
}

#[test]
fn a_cycle_of_runs_waiting_on_each_other_is_broken_by_stopping_the_newer_run() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let (a, b, c, d) = (block(&app, layout_id, "A"), block(&app, layout_id, "B"), block(&app, layout_id, "C"), block(&app, layout_id, "D"));
    let older = route(&app, layout_id, "Older", &[a, b]);
    let newer = route(&app, layout_id, "Newer", &[c, d]);
    start(&app, older, loco(&app, "One", 3));
    start(&app, newer, loco(&app, "Two", 4));

    // Each run is made to wait for the block the other holds
    {
        let mut inner = app.inner.lock().unwrap();
        inner.runs.get_mut(&older).unwrap().waiting_for = Some(d);
        inner.runs.get_mut(&newer).unwrap().waiting_for = Some(b);
    }
    tick(&app);
    assert_eq!(state_of(&app, older)["is_running"], true, "the older run keeps going");
    assert_eq!(state_of(&app, newer)["is_running"], false, "the newer run is stopped");
    let (_, notices) = send(&app, "GET", "/api/routes/notices", None);
    assert!(notices.as_array().unwrap().iter().any(|text| text.as_str().unwrap().starts_with("Stopped Newer")));
}
