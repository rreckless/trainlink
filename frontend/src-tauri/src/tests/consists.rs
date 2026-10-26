use serde_json::{json, Value};

use super::{app, detail, send};

fn loco(app: &std::sync::Arc<crate::state::AppState>, name: &str, address: i64) -> i64 {
    let body = json!({ "name": name, "dcc_address": address, "locomotive_type": "diesel" });
    let (status, value) = send(app, "POST", "/api/locomotives", Some(body));
    assert_eq!(status, 201, "{value}");
    value["id"].as_i64().unwrap()
}

fn member(id: i64, reversed: bool) -> Value {
    json!({ "locomotive_id": id, "reversed": reversed })
}

fn set_connected(app: &std::sync::Arc<crate::state::AppState>, connected: bool) {
    app.inner.lock().unwrap().z21.connected = connected;
}

#[test]
fn create_keeps_member_order_and_direction() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let body = json!({ "name": " Pair ", "members": [member(a, false), member(b, true)] });
    let (status, created) = send(&app, "POST", "/api/consists", Some(body));
    assert_eq!(status, 201, "{created}");
    assert_eq!(created["name"], "Pair");
    assert_eq!(created["members"][0]["dcc_address"], 3);
    assert_eq!(created["members"][0]["reversed"], false);
    assert_eq!(created["members"][1]["name"], "Class 08");
    assert_eq!(created["members"][1]["reversed"], true);

    let (status, listed) = send(&app, "GET", "/api/consists", None);
    assert_eq!(status, 200);
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[test]
fn name_and_members_are_checked() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let blank = json!({ "name": "   ", "members": [member(a, false)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(blank)).0, 422);
    let none = json!({ "name": "Empty", "members": [] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(none)).0, 422);
    let twice = json!({ "name": "Twice", "members": [member(a, false), member(a, true)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(twice)).0, 422);
    let missing = json!({ "name": "Ghost", "members": [member(999, false)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(missing)).0, 404);
}

#[test]
fn duplicate_name_is_refused() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let first = json!({ "name": "Pair", "members": [member(a, false)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(first)).0, 201);
    let second = json!({ "name": "Pair", "members": [member(b, false)] });
    let (status, value) = send(&app, "POST", "/api/consists", Some(second));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Consist name already exists");
}

#[test]
fn a_locomotive_is_in_one_consist_at_a_time() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let (_, first) = send(&app, "POST", "/api/consists", Some(json!({ "name": "First", "members": [member(a, false)] })));
    let first_id = first["id"].as_i64().unwrap();

    let clash = json!({ "name": "Second", "members": [member(a, false), member(b, false)] });
    let (status, value) = send(&app, "POST", "/api/consists", Some(clash));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "A locomotive is already in another consist");

    // Saving the same members back to their own consist is allowed
    let same = json!({ "members": [member(a, true)] });
    let (status, updated) = send(&app, "PATCH", &format!("/api/consists/{first_id}"), Some(same));
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["members"][0]["reversed"], true);
}

#[test]
fn update_renames_and_replaces_members() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let (_, created) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false)] })));
    let id = created["id"].as_i64().unwrap();

    let (status, updated) = send(
        &app,
        "PATCH",
        &format!("/api/consists/{id}"),
        Some(json!({ "name": "Renamed", "members": [member(b, false), member(a, true)] })),
    );
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["name"], "Renamed");
    assert_eq!(updated["members"][0]["dcc_address"], 4, "the first member leads");
    assert_eq!(updated["members"][1]["reversed"], true);
    assert_eq!(send(&app, "PATCH", "/api/consists/999", Some(json!({ "name": "X" }))).0, 404);
}

#[test]
fn deleting_a_locomotive_takes_it_out_of_its_consist() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let (_, created) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false), member(b, false)] })));
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/locomotives/{a}"), None).0, 204);
    let (_, after) = send(&app, "GET", &format!("/api/consists/{id}"), None);
    assert_eq!(after["members"].as_array().unwrap().len(), 1);
    assert_eq!(after["members"][0]["dcc_address"], 4);
}

#[test]
fn delete_consist() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let (_, created) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false)] })));
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/consists/{id}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/consists/{id}"), None).0, 404);
    assert_eq!(send(&app, "DELETE", &format!("/api/consists/{id}"), None).0, 404);
}

#[test]
fn drive_and_function_are_refused_without_a_ready_track() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let (_, created) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false)] })));
    let id = created["id"].as_i64().unwrap();

    let speed = format!("/api/consists/{id}/speed");
    assert_eq!(send(&app, "POST", &speed, Some(json!({ "speed": 10 }))).0, 503);
    set_connected(&app, true);
    let (status, value) = send(&app, "POST", &speed, Some(json!({ "speed": 10 })));
    assert_eq!(status, 409);
    assert!(detail(&value).starts_with("Track is not ready (unknown)"));
    assert_eq!(send(&app, "POST", &speed, Some(json!({ "speed": 127 }))).0, 400);

    let function = format!("/api/consists/{id}/function");
    assert_eq!(send(&app, "POST", &function, Some(json!({ "function": 32, "on": true }))).0, 422);
    let (status, _) = send(&app, "POST", &function, Some(json!({ "function": 1, "on": true })));
    assert_eq!(status, 409);
}
