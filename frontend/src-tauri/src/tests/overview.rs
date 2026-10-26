use serde_json::json;

use super::{app, layout, send};
use crate::library::library;

/// The id of a turnout in the track library, as the layout saves it
fn turnout_id() -> String {
    library().list(None, None, None).into_iter().find(|piece| piece["type"] == "turnout").expect("a turnout in the library")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Save two turnouts on a layout: one with a DCC address, one without
fn with_turnouts(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64) {
    let piece = turnout_id();
    let pieces = json!([
        { "id": "t1", "piece_id": piece, "x": 10, "y": 10, "rotation": 0, "flip": false, "dcc_address": 12 },
        { "id": "t2", "piece_id": piece, "x": 40, "y": 10, "rotation": 0, "flip": false }
    ]);
    let body = json!({ "elements": { "track_system": "Code 55", "track_pieces": pieces } });
    let (status, value) = send(app, "PATCH", &format!("/api/layouts/{layout_id}"), Some(body));
    assert_eq!(status, 200, "{value}");
}

#[test]
fn turnouts_list_every_turnout_with_its_address_state() {
    let app = app();
    let id = layout(&app, "Main");
    with_turnouts(&app, id);
    let (status, rows) = send(&app, "GET", "/api/overview/turnouts", None);
    assert_eq!(status, 200);
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["layout_name"], "Main");
    assert_eq!(rows[0]["dcc_address"], 12);
    assert_eq!(rows[0]["has_address"], true);
    assert_eq!(rows[1]["dcc_address"], serde_json::Value::Null);
    assert_eq!(rows[1]["has_address"], false);
}

#[test]
fn health_lists_turnouts_without_addresses_and_blocks_without_sensors() {
    let app = app();
    let id = layout(&app, "Main");
    with_turnouts(&app, id);
    send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "Platform 1", "x": 0, "y": 0 })));
    let (status, health) = send(&app, "GET", "/api/overview/health", None);
    assert_eq!(status, 200);
    assert_eq!(health["counts"]["turnouts"], 2);
    assert_eq!(health["counts"]["blocks"], 1);
    let turnouts = health["turnouts_without_address"].as_array().unwrap();
    assert_eq!(turnouts.len(), 1, "only the turnout with no address is listed");
    assert_eq!(turnouts[0]["piece_id"], turnout_id());
    let blocks = health["blocks_without_sensor"].as_array().unwrap();
    assert_eq!(blocks[0]["name"], "Platform 1");
    assert_eq!(health["counts"]["missing"], 2);
}

#[test]
fn an_empty_project_has_nothing_missing() {
    let app = app();
    let (status, health) = send(&app, "GET", "/api/overview/health", None);
    assert_eq!(status, 200);
    assert_eq!(health["counts"]["missing"], 0);
    assert_eq!(health["turnouts_without_address"].as_array().unwrap().len(), 0);
}

#[test]
fn legs_without_track_saved_on_a_layout_are_listed_on_health() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let legs = json!([{ "turnout": "TOR1", "leg": "C", "point": "P3" }]);
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{layout_id}"), Some(json!({ "elements": { "no_track_legs": legs } })));
    assert_eq!(status, 200, "{value}");

    let (_, health) = send(&app, "GET", "/api/overview/health", None);
    assert_eq!(health["legs_without_track"][0]["layout"], "Main");
    assert_eq!(health["legs_without_track"][0]["turnout"], "TOR1");
    assert_eq!(health["legs_without_track"][0]["leg"], "C");
    assert_eq!(health["counts"]["missing"], 1);
}
