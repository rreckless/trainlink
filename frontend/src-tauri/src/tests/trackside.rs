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

#[test]
fn a_block_group_is_saved_trimmed_and_can_be_cleared() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let (_, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 0, "y": 0 })));
    let id = block["id"].as_i64().unwrap();
    assert!(block["group_name"].is_null());

    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": "  Up main " })));
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["group_name"], "Up main");

    let (_, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": "   " })));
    assert!(value["group_name"].is_null(), "blank text clears the group");

    let (_, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": "Yard" })));
    assert_eq!(value["group_name"], "Yard");
    let (_, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": null })));
    assert!(value["group_name"].is_null());
}

#[test]
fn a_block_group_name_is_at_most_40_characters_and_must_be_text() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let (_, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 0, "y": 0 })));
    let id = block["id"].as_i64().unwrap();
    let long = "x".repeat(41);
    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": long })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "A group name can be at most 40 characters");
    assert_eq!(send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "group_name": 7 }))).0, 422);
}

#[test]
fn a_database_from_before_block_groups_gets_the_column() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
    conn.execute_batch("CREATE TABLE blocks (id INTEGER PRIMARY KEY, layout_id INTEGER, name TEXT NOT NULL, next_block_id INTEGER);
        CREATE TABLE block_feedbacks (block_id INTEGER NOT NULL, feedback_id INTEGER NOT NULL, PRIMARY KEY (block_id, feedback_id));
        INSERT INTO blocks (layout_id, name) VALUES (1, 'Old');")
        .expect("old table");
    crate::db::trackside::migrate(&conn).expect("migrate");
    let group: Option<String> = conn.query_row("SELECT group_name FROM blocks WHERE name = 'Old'", [], |row| row.get(0)).unwrap();
    assert_eq!(group, None);
    crate::db::trackside::migrate(&conn).expect("running it twice is harmless");
}

#[test]
fn sensor_roles_are_kept_when_the_sensors_change_and_checked() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let (_, one) = send(&app, "POST", "/api/feedbacks", Some(json!({ "layout_id": layout_id, "name": "F1", "x": 1, "y": 1, "feedback_number": 1 })));
    let (_, two) = send(&app, "POST", "/api/feedbacks", Some(json!({ "layout_id": layout_id, "name": "F2", "x": 2, "y": 2, "feedback_number": 2 })));
    let (one_id, two_id) = (one["id"].as_i64().unwrap(), two["id"].as_i64().unwrap());
    let (_, block) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 0, "y": 0 })));
    let id = block["id"].as_i64().unwrap();
    send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_ids": [one_id, two_id] })));

    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_roles": { one_id.to_string(): "entry", two_id.to_string(): "exit" } })));
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["feedback_roles"][one_id.to_string()], "entry");
    assert_eq!(value["feedback_roles"][two_id.to_string()], "exit");

    let (_, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_ids": [one_id] })));
    assert_eq!(value["feedback_roles"][one_id.to_string()], "entry", "a sensor that stays keeps its role");
    assert!(value["feedback_roles"][two_id.to_string()].is_null(), "a sensor that goes loses its role");

    let (_, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_roles": { one_id.to_string(): null } })));
    assert!(value["feedback_roles"][one_id.to_string()].is_null(), "null clears the role");

    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_roles": { one_id.to_string(): "side" } })));
    assert_eq!(status, 422, "{value}");
    assert_eq!(detail(&value), "Sensor role must be one of: entry, middle, exit");

    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{id}"), Some(json!({ "feedback_roles": { two_id.to_string(): "middle" } })));
    assert_eq!(status, 422, "{value}");
    assert_eq!(detail(&value), "Only a sensor the block reads from can have a role");
}
