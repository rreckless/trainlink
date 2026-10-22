use serde_json::json;

use super::{app, detail, layout, send};

#[test]
fn create_layout_starts_with_no_tracks() {
    let app = app();
    let body = json!({ "name": "Yard", "width": 100, "height": 50, "scale": "N", "description": "Test" });
    let (status, value) = send(&app, "POST", "/api/layouts", Some(body));
    assert_eq!(status, 201);
    assert_eq!(value["elements"], json!({ "tracks": [] }));
    assert_eq!(value["description"], "Test");
    assert!(value["created_at"].is_string());
}

#[test]
fn duplicate_names_and_bad_sizes_are_refused() {
    let app = app();
    layout(&app, "Main");
    let (status, value) = send(&app, "POST", "/api/layouts", Some(json!({ "name": "Main", "width": 1, "height": 1, "scale": "N" })));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "A layout with this name already exists");
    let (status, _) = send(&app, "POST", "/api/layouts", Some(json!({ "name": "Flat", "width": 0, "height": 1, "scale": "N" })));
    assert_eq!(status, 400);
}

#[test]
fn list_and_get_and_missing() {
    let app = app();
    let id = layout(&app, "Main");
    let (_, list) = send(&app, "GET", "/api/layouts", None);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (status, value) = send(&app, "GET", &format!("/api/layouts/{id}"), None);
    assert_eq!((status, value["name"].as_str()), (200, Some("Main")));
    let (status, value) = send(&app, "GET", "/api/layouts/999", None);
    assert_eq!(status, 404);
    assert_eq!(detail(&value), "Layout 999 not found");
}

#[test]
fn rename_trims_and_refuses_blank_or_taken_names() {
    let app = app();
    let id = layout(&app, "Main");
    layout(&app, "Other");
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "name": "  Yard  " })));
    assert_eq!((status, value["name"].as_str()), (200, Some("Yard")));
    let (status, _) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "name": "   " })));
    assert_eq!(status, 400);
    let (status, _) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "name": "Other" })));
    assert_eq!(status, 409);
    let (status, _) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "name": null })));
    assert_eq!(status, 400);
}

#[test]
fn delete_empty_layout_and_refuse_when_it_has_blocks() {
    let app = app();
    let id = layout(&app, "Main");
    let (status, _) = send(&app, "POST", "/api/blocks", Some(json!({ "layout_id": id, "name": "A", "x": 0, "y": 0 })));
    assert_eq!(status, 201);
    let (status, value) = send(&app, "DELETE", &format!("/api/layouts/{id}"), None);
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Layout still has blocks. Delete them first");

    let empty = layout(&app, "Empty");
    assert_eq!(send(&app, "DELETE", &format!("/api/layouts/{empty}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/layouts/{empty}"), None).0, 404);
}

#[test]
fn deleting_a_layout_removes_its_track() {
    let app = app();
    let id = layout(&app, "Main");
    let elements = json!({ "tracks": [{ "x1": 0, "y1": 0, "x2": 40, "y2": 0 }] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": elements }))).0, 200);
    assert_eq!(send(&app, "DELETE", &format!("/api/layouts/{id}"), None).0, 204);
    assert_eq!(send(&app, "GET", "/api/layouts", None).1, json!([]));
}

#[test]
fn tracks_need_numeric_ends() {
    let app = app();
    let id = layout(&app, "Main");
    let bad = json!({ "tracks": [{ "x1": "0", "y1": 0, "x2": 40, "y2": 0 }] });
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": bad })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Each track needs numeric x1, y1, x2, y2");
}

#[test]
fn placed_pieces_must_exist_and_keep_one_track_system() {
    let app = app();
    let id = layout(&app, "Main");
    let unknown = json!({ "track_system": "Code 55", "track_pieces": [{ "id": "a", "piece_id": "nope", "x": 0, "y": 0, "rotation": 0, "flip": false }] });
    let (status, _) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": unknown })));
    assert_eq!(status, 400);

    let piece = |id: &str, piece_id: &str| json!({ "id": id, "piece_id": piece_id, "x": 0, "y": 0, "rotation": 0, "flip": false });
    let straight = json!({ "track_system": "Code 55", "track_pieces": [piece("a", "peco-n-sl-300f")] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": straight }))).0, 200);
    let wrong_system = json!({ "track_system": "Setrack (Code 80)", "track_pieces": [piece("a", "peco-n-sl-300f")] });
    let (status, _) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": wrong_system })));
    assert_eq!(status, 400);
}

#[test]
fn dcc_addresses_only_on_turnouts_and_only_once() {
    let app = app();
    let id = layout(&app, "Main");
    let turnout = |id: &str, address: i64| json!({ "id": id, "piece_id": "peco-n-sl-u395f", "x": 0, "y": 0, "rotation": 0, "flip": false, "dcc_address": address });
    let on_track = json!({ "id": "a", "piece_id": "peco-n-sl-300f", "x": 0, "y": 0, "rotation": 0, "flip": false, "dcc_address": 5 });
    let body = json!({ "track_system": "Code 55", "track_pieces": [on_track] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": body }))).0, 400);

    let twice = json!({ "track_system": "Code 55", "track_pieces": [turnout("a", 5), turnout("b", 5)] });
    let (status, value) = send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": twice })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "A DCC turnout address can only be used once in a layout");

    let once = json!({ "track_system": "Code 55", "track_pieces": [turnout("a", 5)] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": once }))).0, 200);
}

#[test]
fn bend_handles_only_on_flex_track() {
    let app = app();
    let id = layout(&app, "Main");
    let handles = json!([{ "x": 30, "y": 15 }, { "x": 60, "y": -15 }]);
    let on_flex = json!({ "id": "a", "piece_id": "peco-n-sl-300f", "x": 0, "y": 0, "rotation": 0, "flip": false, "length": 100, "handles": handles });
    let elements = json!({ "track_system": "Code 55", "track_pieces": [on_flex] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": elements }))).0, 200);
    let on_turnout = json!({ "id": "b", "piece_id": "peco-n-sl-u395f", "x": 0, "y": 0, "rotation": 0, "flip": false, "handles": handles });
    let elements = json!({ "track_system": "Code 55", "track_pieces": [on_turnout] });
    assert_eq!(send(&app, "PATCH", &format!("/api/layouts/{id}"), Some(json!({ "elements": elements }))).0, 400);
}
