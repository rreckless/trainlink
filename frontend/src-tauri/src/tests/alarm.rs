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

fn set_state(app: &std::sync::Arc<crate::state::AppState>, block_id: i64, state: &str) {
    send(app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "state": state })));
}

fn tick(app: &std::sync::Arc<crate::state::AppState>) {
    let mut inner = app.inner.lock().unwrap();
    crate::automation::tick(&mut inner, Instant::now()).unwrap();
}

fn alarms(app: &std::sync::Arc<crate::state::AppState>) -> Vec<serde_json::Value> {
    send(app, "GET", "/api/routes/alarms", None).1.as_array().unwrap().clone()
}

#[test]
fn a_train_no_run_expects_raises_an_alarm_and_clears_when_it_leaves() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let ghost = block(&app, layout_id, "Siding");
    set_state(&app, ghost, "occupied");
    tick(&app);
    let raised = alarms(&app);
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0]["block_id"], ghost);
    assert_eq!(raised[0]["message"], "Unexpected train in Siding: no run has claimed it");

    set_state(&app, ghost, "free");
    tick(&app);
    assert!(alarms(&app).is_empty(), "the alarm clears when the block is free");
}

#[test]
fn a_train_a_run_is_taking_raises_no_alarm() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A");
    let b = block(&app, layout_id, "B");
    let (_, run) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Run", "block_sequence": [a, b] })));
    let run_id = run["id"].as_i64().unwrap();
    let (_, loco) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "One", "dcc_address": 3, "locomotive_type": "diesel" })));
    send(&app, "POST", &format!("/api/routes/{run_id}/run"), Some(json!({ "locomotive_id": loco["id"] })));
    set_state(&app, b, "occupied");
    tick(&app);
    assert!(alarms(&app).is_empty(), "B was claimed by the run before the train entered it");
}

#[test]
fn a_train_left_in_a_block_when_its_run_ends_raises_no_alarm() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A");
    let b = block(&app, layout_id, "B");
    let (_, run) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Ends", "block_sequence": [a, b] })));
    let run_id = run["id"].as_i64().unwrap();
    let (_, loco) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "One", "dcc_address": 3, "locomotive_type": "diesel" })));
    send(&app, "POST", &format!("/api/routes/{run_id}/run"), Some(json!({ "locomotive_id": loco["id"] })));
    set_state(&app, b, "occupied");
    tick(&app);
    send(&app, "POST", &format!("/api/routes/{run_id}/stop"), None);
    tick(&app);
    assert!(alarms(&app).is_empty(), "the train was expected when it arrived");
}

#[test]
fn an_alarm_can_be_dismissed_once() {
    let app = app();
    ready(&app);
    let layout_id = layout(&app, "Main");
    let ghost = block(&app, layout_id, "Siding");
    set_state(&app, ghost, "occupied");
    tick(&app);
    assert_eq!(send(&app, "POST", &format!("/api/routes/alarms/{ghost}/dismiss"), None).0, 204);
    assert!(alarms(&app).is_empty());
    assert_eq!(send(&app, "POST", &format!("/api/routes/alarms/{ghost}/dismiss"), None).0, 404);
}
