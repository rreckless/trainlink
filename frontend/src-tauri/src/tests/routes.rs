use serde_json::json;

use super::{app, detail, layout, send};

fn block(app: &std::sync::Arc<crate::state::AppState>, layout_id: i64, name: &str) -> i64 {
    let (status, value) = send(app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": name, "x": 0, "y": 0 })));
    assert_eq!(status, 201, "{value}");
    value["id"].as_i64().unwrap()
}

#[test]
fn a_route_keeps_its_blocks_in_order_and_cannot_repeat_one() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A");
    let b = block(&app, layout_id, "B");
    let (status, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Up", "block_sequence": [a, b] })));
    assert_eq!(status, 201, "{route}");
    assert_eq!(route["block_sequence"], json!([a, b]));

    let (status, value) = send(&app, "PATCH", &format!("/api/routes/{}", route["id"]), Some(json!({ "block_sequence": [a, b, a] })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "A block can appear only once in a route");
}

#[test]
fn steps_that_do_not_follow_a_block_link_are_reported() {
    let app = app();
    let layout_id = layout(&app, "Main");
    let a = block(&app, layout_id, "A");
    let b = block(&app, layout_id, "B");
    let c = block(&app, layout_id, "C");
    let (_, route) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Run", "block_sequence": [a, b, c] })));
    let route_id = route["id"].as_i64().unwrap();
    assert_eq!(route["unlinked_steps"], json!([[a, b], [b, c]]), "no block is linked yet");

    send(&app, "PATCH", &format!("/api/blocks/{a}"), Some(json!({ "next_block_id": b })));
    let (_, shown) = send(&app, "GET", &format!("/api/routes/{route_id}"), None);
    assert_eq!(shown["unlinked_steps"], json!([[b, c]]), "the linked step is no longer reported");

    send(&app, "PATCH", &format!("/api/blocks/{b}"), Some(json!({ "next_block_id": c })));
    let (_, shown) = send(&app, "GET", "/api/routes", None);
    assert_eq!(shown[0]["unlinked_steps"], json!([]));
}

#[test]
fn a_route_cannot_use_a_block_from_another_layout() {
    let app = app();
    let main = layout(&app, "Main");
    let other = layout(&app, "Other");
    let a = block(&app, main, "A");
    let elsewhere = block(&app, other, "Elsewhere");
    let (status, value) = send(&app, "POST", "/api/routes", Some(json!({ "layout_id": main, "name": "Mixed", "block_sequence": [a, elsewhere] })));
    assert_eq!(status, 400);
    assert!(detail(&value).starts_with("Blocks not in layout"));
}
