use std::time::{Duration, Instant};

use serde_json::json;

use super::{app, send};
use crate::moving_time::MovingTime;

fn travelled(app: &std::sync::Arc<crate::state::AppState>, id: i64) -> f64 {
    send(app, "GET", &format!("/api/locomotives/{id}"), None).1["travelled_time"].as_f64().unwrap()
}

#[test]
fn time_moving_is_saved_when_the_locomotive_stops() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let id = created["id"].as_i64().unwrap();
    let mut moving = MovingTime::new();
    let start = Instant::now();
    {
        let inner = app.inner.lock().unwrap();
        moving.observe(&inner.db, 3, Some(10), start);
        moving.observe(&inner.db, 3, Some(20), start + Duration::from_secs(20));
        moving.observe(&inner.db, 3, Some(0), start + Duration::from_secs(40));
    }
    assert!((travelled(&app, id) - 40.0).abs() < 1e-6);
}

#[test]
fn time_is_saved_every_thirty_seconds_while_moving() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let id = created["id"].as_i64().unwrap();
    let mut moving = MovingTime::new();
    let start = Instant::now();
    let inner = app.inner.lock().unwrap();
    moving.observe(&inner.db, 3, Some(10), start);
    moving.flush_if_due(&inner.db, start + Duration::from_secs(10));
    drop(inner);
    assert_eq!(travelled(&app, id), 0.0, "too soon to save");
    let inner = app.inner.lock().unwrap();
    moving.flush_if_due(&inner.db, start + Duration::from_secs(31));
    drop(inner);
    assert!((travelled(&app, id) - 31.0).abs() < 1e-6);
}

#[test]
fn an_unknown_speed_counts_as_not_moving() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let id = created["id"].as_i64().unwrap();
    let mut moving = MovingTime::new();
    let start = Instant::now();
    let inner = app.inner.lock().unwrap();
    moving.observe(&inner.db, 3, Some(10), start);
    moving.observe(&inner.db, 3, None, start + Duration::from_secs(5));
    drop(inner);
    assert!((travelled(&app, id) - 5.0).abs() < 1e-6);
}
