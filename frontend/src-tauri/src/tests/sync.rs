//! Sync between devices, through an in-memory cloud that follows the project's rules. Each test uses two devices
//! and one account

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::send;
use crate::auth::store::MemoryStore;
use crate::auth::Auth;
use crate::db;
use crate::http::ApiError;
use crate::state::AppState;
use crate::sync::cloud::{Cloud, MemoryCloud};
use crate::sync::{self, Report};

fn device(name: &str) -> Arc<AppState> {
    let images = std::env::temp_dir().join(format!("trainlink-sync-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&images);
    std::fs::create_dir_all(&images).unwrap();
    let auth = Auth::new("http://127.0.0.1:1", "anon", Box::new(MemoryStore::new()));
    Arc::new(AppState::with_auth(db::open_in_memory().unwrap(), images, auth))
}

fn sync(app: &Arc<AppState>, cloud: &dyn Cloud) -> Report {
    let images: PathBuf = app.images.clone();
    sync::run(app, cloud, &images).expect("sync")
}

/// Edits made in quick succession must not share a timestamp
fn tick() {
    std::thread::sleep(Duration::from_millis(3));
}

fn layouts(app: &Arc<AppState>) -> Vec<serde_json::Value> {
    send(app, "GET", "/api/layouts", None).1.as_array().unwrap().clone()
}

#[test]
fn a_layout_made_on_one_device_appears_on_the_other() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a1"), device("b1"));
    send(&a, "POST", "/api/layouts", Some(json!({ "name": "Erinburgh", "width": 800, "height": 400, "scale": "N" })));
    sync(&a, &cloud);
    let report = sync(&b, &cloud);
    assert_eq!(report.pulled, 1);
    let found = layouts(&b);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["name"], "Erinburgh");
}

#[test]
fn references_are_translated_between_devices() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a2"), device("b2"));
    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": "Main", "width": 800, "height": 400, "scale": "N" })));
    let layout_id = layout["id"].as_i64().unwrap();
    let (_, first) = send(&a, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 1, "y": 2 })));
    let block_id = first["id"].as_i64().unwrap();
    send(&a, "POST", "/api/routes", Some(json!({ "layout_id": layout_id, "name": "Up", "block_sequence": [block_id] })));
    sync(&a, &cloud);
    sync(&b, &cloud);

    let b_layout = layouts(&b)[0]["id"].as_i64().unwrap();
    let (_, b_blocks) = send(&b, "GET", "/api/blocks", None);
    assert_eq!(b_blocks[0]["layout_id"].as_i64().unwrap(), b_layout, "the block points at the layout on B");
    let (_, b_routes) = send(&b, "GET", "/api/routes", None);
    assert_eq!(b_routes[0]["block_sequence"], json!([b_blocks[0]["id"]]), "the route points at the block on B");
}

#[test]
fn an_edit_and_a_deletion_reach_the_other_device() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a3"), device("b3"));
    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": "Old", "width": 800, "height": 400, "scale": "N" })));
    let layout_id = layout["id"].as_i64().unwrap();
    let (_, block) = send(&a, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Gone", "x": 1, "y": 2 })));
    sync(&a, &cloud);
    sync(&b, &cloud);

    tick();
    send(&a, "PATCH", &format!("/api/layouts/{layout_id}"), Some(json!({ "name": "New" })));
    send(&a, "DELETE", &format!("/api/blocks/{}", block["id"]), None);
    sync(&a, &cloud);
    sync(&b, &cloud);

    assert_eq!(layouts(&b)[0]["name"], "New");
    assert_eq!(send(&b, "GET", "/api/blocks", None).1, json!([]));
}

#[test]
fn the_newer_change_wins_when_two_devices_edit_the_same_layout() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a4"), device("b4"));
    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": "Shared", "width": 800, "height": 400, "scale": "N" })));
    let layout_id = layout["id"].as_i64().unwrap();
    sync(&a, &cloud);
    sync(&b, &cloud);

    tick();
    send(&a, "PATCH", &format!("/api/layouts/{layout_id}"), Some(json!({ "name": "Edited on A" })));
    sync(&a, &cloud);
    tick();
    let b_layout = layouts(&b)[0]["id"].as_i64().unwrap();
    send(&b, "PATCH", &format!("/api/layouts/{b_layout}"), Some(json!({ "name": "Edited later on B" })));
    sync(&b, &cloud);
    sync(&a, &cloud);
    assert_eq!(layouts(&a)[0]["name"], "Edited later on B");
    assert_eq!(layouts(&b)[0]["name"], "Edited later on B");
}

#[test]
fn a_change_made_before_the_other_device_synced_is_not_overwritten_by_older_data() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a5"), device("b5"));
    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": "Start", "width": 800, "height": 400, "scale": "N" })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    tick();
    // B edits while offline, then A edits and syncs first. B's older edit must not replace A's newer one
    let b_layout = layouts(&b)[0]["id"].as_i64().unwrap();
    send(&b, "PATCH", &format!("/api/layouts/{b_layout}"), Some(json!({ "name": "Older edit on B" })));
    tick();
    send(&a, "PATCH", &format!("/api/layouts/{}", layout["id"]), Some(json!({ "name": "Newer edit on A" })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    sync(&a, &cloud);
    // B's edit was made first, so A's later change is the one that stands
    assert_eq!(layouts(&a)[0]["name"], "Newer edit on A");
    assert_eq!(layouts(&b)[0]["name"], "Newer edit on A");
}

#[test]
fn moving_time_adds_up_across_devices_once_each() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a6"), device("b6"));
    send(&a, "POST", "/api/locomotives", Some(json!({ "name": "Class 08", "dcc_address": 8, "locomotive_type": "diesel" })));
    sync(&a, &cloud);
    sync(&b, &cloud);

    {
        let inner = a.inner.lock().unwrap();
        db::locomotives::add_travelled_time(&inner.db, 8, 40.0).unwrap();
    }
    sync(&a, &cloud);
    sync(&b, &cloud);
    assert_eq!(send(&b, "GET", "/api/locomotives", None).1[0]["travelled_time"], 40.0);

    {
        let inner = b.inner.lock().unwrap();
        db::locomotives::add_travelled_time(&inner.db, 8, 10.0).unwrap();
    }
    sync(&b, &cloud);
    sync(&a, &cloud);
    sync(&a, &cloud);
    sync(&b, &cloud);
    assert_eq!(send(&a, "GET", "/api/locomotives", None).1[0]["travelled_time"], 50.0);
    assert_eq!(send(&b, "GET", "/api/locomotives", None).1[0]["travelled_time"], 50.0);
}

#[test]
fn a_picture_follows_the_locomotive() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a7"), device("b7"));
    let (_, loco) = send(&a, "POST", "/api/locomotives", Some(json!({ "name": "Pictured", "dcc_address": 3, "locomotive_type": "diesel" })));
    let id = loco["id"].as_i64().unwrap();
    let bytes = [0xffu8, 0xd8, 0xff, 0xe0, 1, 2, 3];
    let reply = crate::api::handle(&a, &crate::api::Call {
        method: "POST",
        path: &format!("/api/locomotives/{id}/image"),
        query: "",
        body: &bytes,
        content_type: "image/jpeg",
    });
    assert_eq!(reply.status, 200);
    sync(&a, &cloud);
    sync(&b, &cloud);
    let b_loco = send(&b, "GET", "/api/locomotives", None).1[0]["id"].as_i64().unwrap();
    let picture = std::fs::read(b.images.join(format!("{b_loco}.jpg"))).expect("picture on B");
    assert_eq!(picture, bytes);
}

#[test]
fn syncing_twice_changes_nothing() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a8"), device("b8"));
    send(&a, "POST", "/api/layouts", Some(json!({ "name": "Once", "width": 800, "height": 400, "scale": "N" })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    sync(&b, &cloud);
    let report = sync(&a, &cloud);
    assert_eq!(report.pushed, 0, "nothing is left to send after the first sync");
    assert_eq!(layouts(&b).len(), 1);
}

/// A cloud that is down
struct Offline;

impl Cloud for Offline {
    fn user_id(&self) -> String {
        "user-1".into()
    }
    fn fetch(&self, _: &str) -> Result<Vec<serde_json::Value>, ApiError> {
        Err(ApiError::new(503, "Could not reach the cloud to sync"))
    }
    fn upsert(&self, _: &str, _: &[serde_json::Value]) -> Result<(), ApiError> {
        Err(ApiError::new(503, "Could not reach the cloud to sync"))
    }
    fn delete(&self, _: &str, _: &[String], _: &str) -> Result<(), ApiError> {
        Err(ApiError::new(503, "Could not reach the cloud to sync"))
    }
    fn upload_picture(&self, _: &str, _: &[u8]) -> Result<(), ApiError> {
        Err(ApiError::new(503, "Could not reach the cloud to sync"))
    }
    fn download_picture(&self, _: &str) -> Result<Vec<u8>, ApiError> {
        Err(ApiError::new(503, "Could not reach the cloud to sync"))
    }
}

#[test]
fn work_done_offline_waits_and_then_syncs() {
    let cloud = MemoryCloud::new("user-1");
    let a = device("a9");
    send(&a, "POST", "/api/layouts", Some(json!({ "name": "Offline", "width": 800, "height": 400, "scale": "N" })));
    let images = a.images.clone();
    assert!(sync::run(&a, &Offline, &images).is_err(), "the sync reports that it could not reach the cloud");
    let report = sync(&a, &cloud);
    assert_eq!(report.pushed, 1, "the change was kept and sent once the cloud was back");
}

#[test]
fn a_device_belongs_to_one_account() {
    let a = device("a10");
    sync::claim(&a, "user-1").expect("the first account claims the device");
    sync::claim(&a, "user-1").expect("the same account may sign in again");
    let error = sync::claim(&a, "user-2").expect_err("another account is refused");
    assert_eq!(error.status, 409);
}

/// Two devices, one real account in the Supabase project. Needs the test settings (see supabase/README.md) and
/// is not run by default:
///   set -a; source ~/trainlink-test.env; set +a; cargo test real_sync -- --ignored
#[test]
#[ignore]
fn real_sync() {
    use crate::auth::{SUPABASE_ANON_KEY, SUPABASE_URL};
    use crate::sync::cloud::SupabaseCloud;

    let email = std::env::var("TEST_A_EMAIL").expect("TEST_A_EMAIL");
    let password = std::env::var("TEST_A_PASSWORD").expect("TEST_A_PASSWORD");
    let signer = Auth::new(SUPABASE_URL, SUPABASE_ANON_KEY, Box::new(MemoryStore::new()));
    let session = signer.sign_in(&email, &password).expect("sign in to the real project");
    let cloud = SupabaseCloud::new(SUPABASE_URL, SUPABASE_ANON_KEY, &session.access_token, &session.user_id);

    let a = device("real-a");
    let b = device("real-b");
    let stamp = crate::db::now().replace([':', '.', '-', 'T'], "");
    let address = 1000 + (stamp.bytes().fold(0u32, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte as u32)) % 8000) as i64;
    let name = format!("Sync test {stamp}");

    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": name, "width": 800, "height": 400, "scale": "N" })));
    let (_, loco) = send(&a, "POST", "/api/locomotives", Some(json!({ "name": format!("Loco {stamp}"), "dcc_address": address, "locomotive_type": "diesel" })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    assert!(layouts(&b).iter().any(|item| item["name"] == name), "the layout reached the other device");
    let b_loco = send(&b, "GET", "/api/locomotives", None).1.as_array().unwrap().iter().find(|item| item["dcc_address"] == address).cloned();
    assert!(b_loco.is_some(), "the locomotive reached the other device");

    {
        let inner = a.inner.lock().unwrap();
        db::locomotives::add_travelled_time(&inner.db, address, 25.0).unwrap();
    }
    sync(&a, &cloud);
    sync(&b, &cloud);
    let b_total = send(&b, "GET", "/api/locomotives", None).1.as_array().unwrap().iter().find(|item| item["dcc_address"] == address).unwrap()["travelled_time"].as_f64();
    assert_eq!(b_total, Some(25.0), "moving time reached the other device");

    // Clean up: delete the test rows through sync, so the deletion reaches the cloud too
    send(&a, "DELETE", &format!("/api/locomotives/{}", loco["id"]), None);
    send(&a, "DELETE", &format!("/api/layouts/{}", layout["id"]), None);
    sync(&a, &cloud);
    sync(&b, &cloud);
    assert!(!layouts(&b).iter().any(|item| item["name"] == name), "the deletion reached the other device");
}

#[test]
fn writes_to_synced_data_ask_for_a_sync_and_nothing_else_does() {
    use crate::api::changes_synced_data;
    assert!(changes_synced_data("POST", "/api/layouts"));
    assert!(changes_synced_data("PATCH", "/api/layouts/3"));
    assert!(changes_synced_data("DELETE", "/api/layouts/3"));
    assert!(changes_synced_data("POST", "/api/locomotives/7/image"));
    assert!(changes_synced_data("POST", "/api/blocks"));
    assert!(changes_synced_data("PATCH", "/api/routes/2"));
    assert!(!changes_synced_data("GET", "/api/layouts"));
    assert!(!changes_synced_data("POST", "/api/layoutsx"));
    assert!(!changes_synced_data("POST", "/api/z21/connect"));
    assert!(!changes_synced_data("POST", "/api/auth/sign-in"));
    assert!(!changes_synced_data("POST", "/api/sync/now"));
}

#[test]
fn a_request_wakes_the_sync_loop_straight_away() {
    use std::time::{Duration, Instant};
    crate::sync::request_sync();
    let started = Instant::now();
    crate::sync::wait_for_request(Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(5), "waited {:?}", started.elapsed());
}

#[test]
fn settings_set_on_one_device_reach_the_other() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a-settings"), device("b-settings"));
    send(&a, "PUT", "/api/settings/z21.host", Some(json!({ "value": "10.0.0.50" })));
    send(&a, "PUT", "/api/settings/z21.port", Some(json!({ "value": 21106 })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    let (status, body) = send(&b, "GET", "/api/settings", None);
    assert_eq!(status, 200);
    assert_eq!(body["z21.host"], "10.0.0.50");
    assert_eq!(body["z21.port"], 21106);
}

#[test]
fn the_newer_setting_wins_between_devices() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a-newer"), device("b-newer"));
    send(&a, "PUT", "/api/settings/z21.host", Some(json!({ "value": "10.0.0.50" })));
    sync(&a, &cloud);
    sync(&b, &cloud);
    tick();
    send(&b, "PUT", "/api/settings/z21.host", Some(json!({ "value": "192.168.1.9" })));
    sync(&b, &cloud);
    sync(&a, &cloud);
    assert_eq!(send(&a, "GET", "/api/settings", None).1["z21.host"], "192.168.1.9");
}

#[test]
fn settings_are_checked_before_they_are_kept() {
    let app = device("checked-settings");
    assert_eq!(send(&app, "PUT", "/api/settings/z21.host", Some(json!({ "value": "8.8.8.8" }))).0, 422);
    assert_eq!(send(&app, "PUT", "/api/settings/z21.host", Some(json!({ "value": 42 }))).0, 422);
    assert_eq!(send(&app, "PUT", "/api/settings/z21.port", Some(json!({ "value": 0 }))).0, 422);
    assert_eq!(send(&app, "PUT", "/api/settings/volume", Some(json!({ "value": 3 }))).0, 404);
    assert_eq!(send(&app, "GET", "/api/settings", None).1, json!({}));
}

#[test]
fn block_links_reach_the_other_device_and_removals_too() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b) = (device("a-links"), device("b-links"));
    let (_, layout) = send(&a, "POST", "/api/layouts", Some(json!({ "name": "Links", "width": 800, "height": 400, "scale": "N" })));
    let layout_id = layout["id"].as_i64().unwrap();
    let (_, platform) = send(&a, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Platform", "x": 1, "y": 2 })));
    let (_, exit) = send(&a, "POST", "/api/blocks", Some(json!({ "layout_id": layout_id, "name": "Exit", "x": 3, "y": 2 })));
    let (_, feedback) = send(&a, "POST", "/api/feedbacks", Some(json!({ "layout_id": layout_id, "name": "F", "x": 1, "y": 1, "feedback_number": 4 })));
    let (_, signal) = send(&a, "POST", "/api/signals", Some(json!({ "layout_id": layout_id, "name": "Home", "x": 1, "y": 3, "signal_type": "main" })));
    let platform_id = platform["id"].as_i64().unwrap();
    let links = json!({
        "next_block_id": exit["id"],
        "feedback_ids": [feedback["id"]],
        "signal_ids": [signal["id"]],
    });
    let (status, _) = send(&a, "PATCH", &format!("/api/blocks/{platform_id}"), Some(links));
    assert_eq!(status, 200);
    sync(&a, &cloud);
    sync(&b, &cloud);

    let (_, b_blocks) = send(&b, "GET", "/api/blocks", None);
    let (_, b_feedbacks) = send(&b, "GET", "/api/feedbacks", None);
    let (_, b_signals) = send(&b, "GET", "/api/signals", None);
    let b_platform = b_blocks.as_array().unwrap().iter().find(|block| block["name"] == "Platform").unwrap().clone();
    let b_exit = b_blocks.as_array().unwrap().iter().find(|block| block["name"] == "Exit").unwrap();
    assert_eq!(b_platform["next_block_id"], b_exit["id"], "the next block points at the block on B");
    assert_eq!(b_platform["feedback_ids"], json!([b_feedbacks[0]["id"]]));
    assert_eq!(b_platform["signal_ids"], json!([b_signals[0]["id"]]));

    tick();
    let (status, _) = send(&a, "PATCH", &format!("/api/blocks/{platform_id}"), Some(json!({ "next_block_id": null, "feedback_ids": [], "signal_ids": [] })));
    assert_eq!(status, 200);
    sync(&a, &cloud);
    sync(&b, &cloud);
    let (_, b_blocks) = send(&b, "GET", "/api/blocks", None);
    let b_platform = b_blocks.as_array().unwrap().iter().find(|block| block["name"] == "Platform").unwrap();
    assert!(b_platform["next_block_id"].is_null(), "a cleared next block reaches B");
    assert_eq!(b_platform["feedback_ids"], json!([]), "removed links reach B");
    assert_eq!(b_platform["signal_ids"], json!([]));
}

#[test]
fn a_locomotive_keeps_its_command_station_by_name_on_each_device() {
    let cloud = MemoryCloud::new("user-1");
    let (a, b, c) = (device("a-station"), device("b-station"), device("c-station"));
    let (_, shed) = send(&a, "POST", "/api/command-stations", Some(json!({ "name": "Shed", "port": "10.0.0.111:21105" })));
    send(&a, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel", "command_station_id": shed["id"].clone() })));
    sync(&a, &cloud);

    // Device b has its own record of a station called Shed, on its own address
    let (_, shed_b) = send(&b, "POST", "/api/command-stations", Some(json!({ "name": "Shed", "port": "192.168.1.20:21105" })));
    sync(&b, &cloud);
    let on_b = send(&b, "GET", "/api/locomotives", None).1[0].clone();
    assert_eq!(on_b["command_station_id"], shed_b["id"], "the station it is on, on this device");

    // Device c has no station of that name, so the locomotive has none there
    sync(&c, &cloud);
    let on_c = send(&c, "GET", "/api/locomotives", None).1[0].clone();
    assert!(on_c["command_station_id"].is_null());
}

#[test]
fn renaming_a_station_sends_its_locomotives_with_the_new_name() {
    let cloud = MemoryCloud::new("user-1");
    let a = device("a-rename");
    let (_, shed) = send(&a, "POST", "/api/command-stations", Some(json!({ "name": "Shed", "port": "10.0.0.111:21105" })));
    let station_id = shed["id"].as_i64().unwrap();
    send(&a, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel", "command_station_id": station_id })));
    sync(&a, &cloud);
    assert_eq!(cloud.fetch("locomotives").unwrap()[0]["command_station"], "Shed");

    tick();
    send(&a, "PATCH", &format!("/api/command-stations/{station_id}"), Some(json!({ "name": "Shed East" })));
    sync(&a, &cloud);
    assert_eq!(cloud.fetch("locomotives").unwrap()[0]["command_station"], "Shed East");
}
