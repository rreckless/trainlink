use serde_json::json;

use super::{app, detail, layout, send};

#[test]
fn block_defaults_and_state_sets_occupied() {
    let app = app();
    let id = layout(&app, "Main");
    let (status, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "Platform 1", "x": 10, "y": 20 })));
    assert_eq!(status, 201);
    assert_eq!(block["state"], "free");
    assert_eq!(block["occupied"], false);
    assert_eq!(block["max_speed"], 100);
    assert_eq!(block["signal_ids"], json!([]));

    let block_id = block["id"].as_i64().unwrap();
    let (_, occupied) = send(&app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "state": "occupied" })));
    assert_eq!((occupied["state"].as_str(), occupied["occupied"].as_bool()), (Some("occupied"), Some(true)));
    let (_, free) = send(&app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "state": "free" })));
    assert_eq!(free["occupied"], false);
}

#[test]
fn block_state_must_be_a_known_state() {
    let app = app();
    let id = layout(&app, "Main");
    let (_, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "A", "x": 0, "y": 0 })));
    let block_id = block["id"].as_i64().unwrap();
    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "state": "broken" })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "State must be one of: free, occupied, reserved");
}

#[test]
fn blocks_filter_by_layout_and_delete() {
    let app = app();
    let id = layout(&app, "Main");
    let (_, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "A", "x": 0, "y": 0 })));
    assert_eq!(send(&app, "GET", &format!("/api/blocks?layout_id={id}"), None).1.as_array().unwrap().len(), 1);
    assert_eq!(send(&app, "GET", "/api/blocks?layout_id=999", None).1, json!([]));
    let block_id = block["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/blocks/{block_id}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/blocks/{block_id}"), None).0, 404);
}

#[test]
fn signals_default_to_stop_and_check_their_type_and_aspect() {
    let app = app();
    let id = layout(&app, "Main");
    let signal = json!({ "layout_id": id, "name": "Home", "x": 5, "y": 5, "signal_type": "main" });
    let (status, created) = send(&app, "POST", "/api/signals", Some(signal.clone()));
    assert_eq!(status, 201);
    assert_eq!(created["aspect"], "stop");
    let signal_id = created["id"].as_i64().unwrap();

    let (status, _) = send(&app, "PATCH", &format!("/api/signals/{signal_id}"), Some(json!({ "aspect": "go" })));
    assert_eq!(status, 200);
    let (status, value) = send(&app, "PATCH", &format!("/api/signals/{signal_id}"), Some(json!({ "aspect": "purple" })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Aspect must be one of: caution, go, stop");

    let mut home = signal;
    home["signal_type"] = json!("home");
    let (status, value) = send(&app, "POST", "/api/signals", Some(home));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Signal type must be one of: main, warning");
}

#[test]
fn feedbacks_create_and_set_state() {
    let app = app();
    let id = layout(&app, "Main");
    let body = json!({ "layout_id": id, "name": "Sensor", "x": 1, "y": 2, "feedback_number": 7 });
    let (status, created) = send(&app, "POST", "/api/feedbacks", Some(body));
    assert_eq!(status, 201);
    assert_eq!(created["state"], false);
    let feedback_id = created["id"].as_i64().unwrap();
    let (_, updated) = send(&app, "PATCH", &format!("/api/feedbacks/{feedback_id}"), Some(json!({ "state": true })));
    assert_eq!(updated["state"], true);
}

#[test]
fn routes_check_their_blocks_and_refuse_running_state() {
    let app = app();
    let id = layout(&app, "Main");
    let (_, first) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "A", "x": 0, "y": 0 })));
    let first_id = first["id"].as_i64().unwrap();
    let route = json!({ "layout_id": id, "name": "Up line", "block_sequence": [first_id] });
    let (status, created) = send(&app, "POST", "/api/routes", Some(route));
    assert_eq!(status, 201);
    assert_eq!(created["is_running"], false);
    assert_eq!(created["enabled"], true);
    let route_id = created["id"].as_i64().unwrap();

    let (status, value) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": id, "name": "X", "block_sequence": [first_id, 999] })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Blocks not in layout 1: [999]");

    let (status, value) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": id, "name": "X", "block_sequence": [] })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Route needs at least one block");

    let (status, _) = send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "is_running": true })));
    assert_eq!(status, 400);
    let (status, _) = send(&app, "PATCH", &format!("/api/routes/{route_id}"), Some(json!({ "block_sequence": [999] })));
    assert_eq!(status, 400);
}
