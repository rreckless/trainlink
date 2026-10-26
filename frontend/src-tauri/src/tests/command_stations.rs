use serde_json::json;

use super::{app, detail, send};

#[test]
fn create_defaults_to_a_z21_and_lists_it() {
    let app = app();
    let (status, created) = send(&app, "POST", "/api/command-stations", Some(json!({ "name": " Roco Z21 ", "port": "10.0.0.111:21105" })));
    assert_eq!(status, 201, "{created}");
    assert_eq!(created["name"], "Roco Z21");
    assert_eq!(created["station_type"], "z21");
    assert_eq!(created["port"], "10.0.0.111:21105");
    assert_eq!(created["settings"], json!({}));
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "GET", &format!("/api/command-stations/{id}"), None).1["name"], "Roco Z21");
    let (status, listed) = send(&app, "GET", "/api/command-stations", None);
    assert_eq!(status, 200);
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[test]
fn port_must_be_a_local_host_and_a_port_number() {
    let app = app();
    for bad in ["10.0.0.111", "10.0.0.111:0", "10.0.0.111:70000", "8.8.8.8:21105", "not-an-address:21105"] {
        let body = json!({ "name": "Z21", "port": bad });
        let (status, value) = send(&app, "POST", "/api/command-stations", Some(body));
        assert_eq!(status, 422, "{bad}: {value}");
    }
}

#[test]
fn name_and_type_are_checked() {
    let app = app();
    let blank = json!({ "name": "  ", "port": "10.0.0.111:21105" });
    assert_eq!(send(&app, "POST", "/api/command-stations", Some(blank)).0, 422);
    let unknown = json!({ "name": "ECoS", "station_type": "ecos", "port": "10.0.0.111:15471" });
    let (status, value) = send(&app, "POST", "/api/command-stations", Some(unknown));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "Station type must be one of: z21");
}

#[test]
fn duplicate_name_is_refused() {
    let app = app();
    let body = json!({ "name": "Z21", "port": "10.0.0.111:21105" });
    assert_eq!(send(&app, "POST", "/api/command-stations", Some(body.clone())).0, 201);
    let (status, value) = send(&app, "POST", "/api/command-stations", Some(body));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Command station name already exists");
}

#[test]
fn update_changes_the_port_and_settings() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/command-stations", Some(json!({ "name": "Z21", "port": "10.0.0.111:21105" })));
    let id = created["id"].as_i64().unwrap();
    let (status, updated) = send(
        &app,
        "PATCH",
        &format!("/api/command-stations/{id}"),
        Some(json!({ "port": "192.168.1.20:21105", "settings": { "track_power_on_connect": true } })),
    );
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["port"], "192.168.1.20:21105");
    assert_eq!(updated["settings"]["track_power_on_connect"], true);
    assert_eq!(send(&app, "PATCH", &format!("/api/command-stations/{id}"), Some(json!({ "port": "8.8.8.8:21105" }))).0, 422);
    assert_eq!(send(&app, "GET", "/api/command-stations/999", None).0, 404);
}

#[test]
fn a_station_in_use_cannot_be_deleted() {
    let app = app();
    let (_, station) = send(&app, "POST", "/api/command-stations", Some(json!({ "name": "Z21", "port": "10.0.0.111:21105" })));
    let station_id = station["id"].as_i64().unwrap();
    let loco = json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel", "command_station_id": station_id });
    assert_eq!(send(&app, "POST", "/api/locomotives", Some(loco)).0, 201);

    let (status, value) = send(&app, "DELETE", &format!("/api/command-stations/{station_id}"), None);
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Command station is used by 1 locomotive");

    let (_, spare) = send(&app, "POST", "/api/command-stations", Some(json!({ "name": "Spare", "port": "10.0.0.112:21105" })));
    let spare_id = spare["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/command-stations/{spare_id}"), None).0, 204);
    assert_eq!(send(&app, "DELETE", &format!("/api/command-stations/{spare_id}"), None).0, 404);
}
