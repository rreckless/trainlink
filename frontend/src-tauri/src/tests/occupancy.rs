use serde_json::json;

use super::{app, detail, layout, send};
use crate::occupancy::{aspect, module_and_input};

#[test]
fn feedback_numbers_map_to_module_and_input_as_the_z21_counts_them() {
    assert_eq!(module_and_input(1), (1, 1));
    assert_eq!(module_and_input(8), (1, 8));
    assert_eq!(module_and_input(9), (2, 1));
    assert_eq!(module_and_input(17), (3, 1));
}

#[test]
fn a_signal_shows_stop_when_any_block_it_protects_is_occupied() {
    assert_eq!(aspect([false, false]), "go");
    assert_eq!(aspect([false, true]), "stop");
    assert_eq!(aspect(std::iter::empty()), "go");
}

/// A layout with one block read from feedback 1, and feedback 2 also linked to it
fn block_on_feedbacks(app: &std::sync::Arc<crate::state::AppState>) -> (i64, i64) {
    let layout_id = layout(app, "Main");
    let (_, one) = send(app, "POST", "/api/feedbacks", Some(json!({ "layout_id": layout_id, "name": "F1", "x": 1, "y": 1, "feedback_number": 1 })));
    let (_, two) = send(app, "POST", "/api/feedbacks", Some(json!({ "layout_id": layout_id, "name": "F2", "x": 2, "y": 2, "feedback_number": 2 })));
    let (_, block) = send(app, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 0, "y": 0 })));
    let block_id = block["id"].as_i64().unwrap();
    let feedbacks = json!([one["id"].as_i64().unwrap(), two["id"].as_i64().unwrap()]);
    let (status, linked) = send(app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "feedback_ids": feedbacks })));
    assert_eq!(status, 200, "{linked}");
    assert_eq!(linked["feedback_ids"].as_array().unwrap().len(), 2);
    (layout_id, block_id)
}

fn report(app: &std::sync::Arc<crate::state::AppState>, module: i64, input: i64, occupied: bool) {
    app.inner.lock().unwrap().z21.feedback.insert((module, input), occupied);
}

#[test]
fn a_block_is_occupied_when_its_feedback_reports_it() {
    let app = app();
    let (_, block_id) = block_on_feedbacks(&app);

    let (_, block) = send(&app, "GET", &format!("/api/blocks/{block_id}"), None);
    assert!(block["detected"].is_null(), "nothing reported yet");
    assert_eq!(block["occupied_now"], false, "falls back to the block's own state");

    report(&app, 1, 2, true);
    let (_, block) = send(&app, "GET", &format!("/api/blocks/{block_id}"), None);
    assert_eq!(block["detected"], true);
    assert_eq!(block["occupied_now"], true);

    report(&app, 1, 2, false);
    report(&app, 1, 1, false);
    let (_, block) = send(&app, "GET", &format!("/api/blocks/{block_id}"), None);
    assert_eq!(block["detected"], false);
    assert_eq!(block["occupied_now"], false);
}

#[test]
fn a_signal_follows_the_occupancy_of_the_blocks_it_protects() {
    let app = app();
    let (layout_id, block_id) = block_on_feedbacks(&app);
    let (status, signal) = send(&app, "POST", "/api/signals", Some(json!({ "layout_id": layout_id, "name": "Home", "x": 5, "y": 5, "signal_type": "main" })));
    assert_eq!(status, 201, "{signal}");
    let signal_id = signal["id"].as_i64().unwrap();

    let (status, linked) = send(&app, "PATCH", &format!("/api/signals/{signal_id}"), Some(json!({ "block_ids": [block_id] })));
    assert_eq!(status, 200, "{linked}");
    assert_eq!(linked["aspect_source"], "blocks");
    assert_eq!(linked["aspect"], "go");

    report(&app, 1, 1, true);
    let (_, shown) = send(&app, "GET", &format!("/api/signals/{signal_id}"), None);
    assert_eq!(shown["aspect"], "stop");

    let (status, value) = send(&app, "PATCH", &format!("/api/signals/{signal_id}"), Some(json!({ "aspect": "go" })));
    assert_eq!(status, 409, "{value}");
    assert_eq!(detail(&value), "This signal's aspect follows the blocks it protects. Unlink the blocks to set it by hand");

    let (status, unlinked) = send(&app, "PATCH", &format!("/api/signals/{signal_id}"), Some(json!({ "block_ids": [], "aspect": "caution" })));
    assert_eq!(status, 200, "{unlinked}");
    assert_eq!(unlinked["aspect_source"], "manual");
    assert_eq!(unlinked["aspect"], "caution");
}

#[test]
fn links_must_stay_on_one_layout() {
    let app = app();
    let (_, block_id) = block_on_feedbacks(&app);
    let other = layout(&app, "Other");
    let (_, feedback) = send(&app, "POST", "/api/feedbacks", Some(json!({ "layout_id": other, "name": "Elsewhere", "x": 0, "y": 0, "feedback_number": 9 })));
    let feedback_id = feedback["id"].as_i64().unwrap();
    let (status, value) = send(&app, "PATCH", &format!("/api/blocks/{block_id}"), Some(json!({ "feedback_ids": [feedback_id] })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "A feedback must be on the same layout as its block");
}
