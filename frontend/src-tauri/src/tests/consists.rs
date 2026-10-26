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

/// Two locomotives and a consist made of them, for the tests that need a working consist
fn pair(app: &std::sync::Arc<crate::state::AppState>) -> (i64, i64, i64) {
    let a = loco(app, "Class 37", 3);
    let b = loco(app, "Class 08", 4);
    let (status, created) = send(app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false), member(b, false)] })));
    assert_eq!(status, 201, "{created}");
    (created["id"].as_i64().unwrap(), a, b)
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
    assert_eq!(created["address"], 1, "the first free consist address is used");
    assert_eq!(created["members"][0]["dcc_address"], 3);
    assert_eq!(created["members"][0]["reversed"], false);
    assert_eq!(created["members"][1]["name"], "Class 08");
    assert_eq!(created["members"][1]["reversed"], true);

    let (status, listed) = send(&app, "GET", "/api/consists", None);
    assert_eq!(status, 200);
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[test]
fn a_consist_needs_two_locomotives() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let (status, value) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Solo", "members": [member(a, false)] })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "A consist needs at least 2 locomotives");
    assert_eq!(send(&app, "POST", "/api/consists", Some(json!({ "name": "Empty", "members": [] }))).0, 422);

    let b = loco(&app, "Class 08", 4);
    let (_, created) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(a, false), member(b, false)] })));
    let id = created["id"].as_i64().unwrap();
    let (status, _) = send(&app, "PATCH", &format!("/api/consists/{id}"), Some(json!({ "members": [member(a, false)] })));
    assert_eq!(status, 422, "a consist cannot be cut down to one locomotive");
}

#[test]
fn name_and_members_are_checked() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let blank = json!({ "name": "   ", "members": [member(a, false), member(b, false)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(blank)).0, 422);
    let twice = json!({ "name": "Twice", "members": [member(a, false), member(a, true)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(twice)).0, 422);
    let missing = json!({ "name": "Ghost", "members": [member(a, false), member(999, false)] });
    assert_eq!(send(&app, "POST", "/api/consists", Some(missing)).0, 404);
}

#[test]
fn duplicate_name_is_refused() {
    let app = app();
    let (_, _, _) = pair(&app);
    let c = loco(&app, "Class 20", 5);
    let d = loco(&app, "Class 47", 6);
    let (status, value) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Pair", "members": [member(c, false), member(d, false)] })));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Consist name already exists");
}

#[test]
fn consist_addresses_default_in_turn_and_must_be_unique() {
    let app = app();
    let a = loco(&app, "Class 37", 3);
    let b = loco(&app, "Class 08", 4);
    let c = loco(&app, "Class 20", 5);
    let d = loco(&app, "Class 47", 6);
    let (_, first) = send(&app, "POST", "/api/consists", Some(json!({ "name": "First", "members": [member(a, false), member(b, false)] })));
    let (_, second) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Second", "members": [member(c, false), member(d, false)] })));
    assert_eq!(first["address"], 1);
    assert_eq!(second["address"], 2, "the next unused address is taken");

    let e = loco(&app, "Class 56", 7);
    let f = loco(&app, "Class 66", 8);
    let taken = json!({ "name": "Third", "address": 1, "members": [member(e, false), member(f, false)] });
    let (status, value) = send(&app, "POST", "/api/consists", Some(taken));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Consist address is already used by another consist");

    for bad in [0, 128] {
        let body = json!({ "name": "Range", "address": bad, "members": [member(e, false), member(f, false)] });
        assert_eq!(send(&app, "POST", "/api/consists", Some(body)).0, 422, "address {bad}");
    }
}

#[test]
fn consist_address_can_be_changed_but_not_shared() {
    let app = app();
    let (first, _, _) = pair(&app);
    let c = loco(&app, "Class 20", 5);
    let d = loco(&app, "Class 47", 6);
    let (_, second) = send(&app, "POST", "/api/consists", Some(json!({ "name": "Second", "members": [member(c, false), member(d, false)] })));
    let second_id = second["id"].as_i64().unwrap();

    let (status, updated) = send(&app, "PATCH", &format!("/api/consists/{first}"), Some(json!({ "address": 5 })));
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["address"], 5);
    let (status, value) = send(&app, "PATCH", &format!("/api/consists/{second_id}"), Some(json!({ "address": 5 })));
    assert_eq!(status, 409, "{value}");
    let (status, _) = send(&app, "PATCH", &format!("/api/consists/{first}"), Some(json!({ "address": 5 })));
    assert_eq!(status, 200, "a consist can keep its own address");
}

#[test]
fn a_locomotive_is_in_one_consist_at_a_time() {
    let app = app();
    let (first, a, b) = pair(&app);
    let c = loco(&app, "Class 20", 5);
    let clash = json!({ "name": "Second", "members": [member(a, false), member(c, false)] });
    let (status, value) = send(&app, "POST", "/api/consists", Some(clash));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "A locomotive is already in another consist");

    // Saving the same members back to their own consist is allowed
    let same = json!({ "members": [member(a, true), member(b, false)] });
    let (status, updated) = send(&app, "PATCH", &format!("/api/consists/{first}"), Some(same));
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["members"][0]["reversed"], true);
}

#[test]
fn update_renames_and_replaces_members() {
    let app = app();
    let (id, a, b) = pair(&app);
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
    let c = loco(&app, "Class 20", 5);
    let (_, created) = send(
        &app,
        "POST",
        "/api/consists",
        Some(json!({ "name": "Trio", "members": [member(a, false), member(b, false), member(c, false)] })),
    );
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/locomotives/{a}"), None).0, 204);
    let (_, after) = send(&app, "GET", &format!("/api/consists/{id}"), None);
    assert_eq!(after["members"].as_array().unwrap().len(), 2);
    assert_eq!(after["members"][0]["dcc_address"], 4);
}

#[test]
fn delete_consist() {
    let app = app();
    let (id, _, _) = pair(&app);
    assert_eq!(send(&app, "DELETE", &format!("/api/consists/{id}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/consists/{id}"), None).0, 404);
    assert_eq!(send(&app, "DELETE", &format!("/api/consists/{id}"), None).0, 404);
}

#[test]
fn drive_and_function_are_refused_without_a_ready_track() {
    let app = app();
    let (id, _, _) = pair(&app);

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

#[test]
fn a_database_from_before_addresses_gets_the_column() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
    conn.execute_batch("CREATE TABLE consists (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        INSERT INTO consists (name, created_at, updated_at) VALUES ('Old', 'x', 'x');")
        .expect("old table");
    crate::db::consists::migrate(&conn).expect("migrate");
    let address: i64 = conn.query_row("SELECT address FROM consists WHERE name = 'Old'", [], |row| row.get(0)).unwrap();
    assert_eq!(address, 1, "existing consists keep address 1");
    crate::db::consists::migrate(&conn).expect("running it twice is harmless");
}
