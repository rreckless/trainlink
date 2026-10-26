use serde_json::json;

use super::{app, detail, send};

fn station(app: &std::sync::Arc<crate::state::AppState>, name: &str, port: &str) -> i64 {
    let (status, value) = send(app, "POST", "/api/command-stations", Some(json!({ "name": name, "port": port })));
    assert_eq!(status, 201, "{value}");
    value["id"].as_i64().unwrap()
}

fn loco(app: &std::sync::Arc<crate::state::AppState>, name: &str, address: i64, station: Option<i64>) -> (u16, serde_json::Value) {
    send(app, "POST", "/api/locomotives", Some(json!({ "name": name, "dcc_address": address, "locomotive_type": "diesel", "command_station_id": station })))
}

#[test]
fn two_command_stations_can_each_use_the_same_address() {
    let app = app();
    let first = station(&app, "Shed", "10.0.0.111:21105");
    let second = station(&app, "Yard", "10.0.0.112:21105");
    assert_eq!(loco(&app, "Shed Class 37", 3, Some(first)).0, 201);
    let (status, value) = loco(&app, "Yard Class 37", 3, Some(second));
    assert_eq!(status, 201, "{value}");
}

#[test]
fn one_command_station_cannot_use_an_address_twice() {
    let app = app();
    let shed = station(&app, "Shed", "10.0.0.111:21105");
    assert_eq!(loco(&app, "First", 3, Some(shed)).0, 201);
    let (status, value) = loco(&app, "Second", 3, Some(shed));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "DCC address 3 is already used by First on this command station");
    assert_eq!(loco(&app, "Second", 4, Some(shed)).0, 201, "another address is fine");
}

#[test]
fn locomotives_with_no_station_share_one_set_of_addresses() {
    let app = app();
    let shed = station(&app, "Shed", "10.0.0.111:21105");
    assert_eq!(loco(&app, "Loose", 3, None).0, 201);
    assert_eq!(loco(&app, "Loose again", 3, None).0, 409);
    assert_eq!(loco(&app, "On the shed", 3, Some(shed)).0, 201, "a station's address is separate from the loose ones");
}

#[test]
fn an_update_cannot_move_a_locomotive_onto_an_address_its_station_uses() {
    let app = app();
    let shed = station(&app, "Shed", "10.0.0.111:21105");
    loco(&app, "First", 3, Some(shed));
    let (_, second) = loco(&app, "Second", 4, Some(shed));
    let id = second["id"].as_i64().unwrap();
    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "dcc_address": 3 })));
    assert_eq!(status, 409, "{value}");
    assert_eq!(send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "dcc_address": 4 }))).0, 200, "keeping its own address is fine");
}

#[test]
fn the_next_address_is_worked_out_per_command_station() {
    let app = app();
    let shed = station(&app, "Shed", "10.0.0.111:21105");
    let yard = station(&app, "Yard", "10.0.0.112:21105");
    loco(&app, "First", 1, Some(shed));
    loco(&app, "Second", 2, Some(shed));
    let next = |path: &str| send(&app, "GET", path, None).1["dcc_address"].as_i64().unwrap();
    assert_eq!(next(&format!("/api/locomotives/next-address?command_station_id={shed}")), 3);
    assert_eq!(next(&format!("/api/locomotives/next-address?command_station_id={yard}")), 1);
    assert_eq!(next("/api/locomotives/next-address"), 1);
    assert_eq!(send(&app, "GET", "/api/locomotives/next-address?command_station_id=x", None).0, 422);
}

#[test]
fn a_database_from_before_station_addresses_is_rebuilt_and_keeps_its_rows() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE decoder_profiles (id INTEGER PRIMARY KEY);
         CREATE TABLE command_stations (id INTEGER PRIMARY KEY);
         INSERT INTO command_stations (id) VALUES (1), (2);
         CREATE TABLE locomotives (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            dcc_address INTEGER NOT NULL UNIQUE,
            locomotive_type TEXT NOT NULL,
            command_station_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE UNIQUE INDEX locomotives_name_index ON locomotives (name, dcc_address);
         INSERT INTO locomotives (id, name, dcc_address, locomotive_type, command_station_id, created_at, updated_at)
            VALUES (1, 'Old', 3, 'diesel', 1, 'then', 'then');
         CREATE TABLE probe (id INTEGER PRIMARY KEY, locomotive_id INTEGER REFERENCES locomotives(id) ON DELETE CASCADE);
         INSERT INTO probe (locomotive_id) VALUES (1);",
    )
    .expect("old table");

    crate::db::locomotives_migrate(&conn).expect("migrate");
    let kept: i64 = conn.query_row("SELECT COUNT(*) FROM probe WHERE locomotive_id = 1", [], |row| row.get(0)).unwrap();
    assert_eq!(kept, 1, "rows that refer to the locomotive are kept");
    let indexes: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND tbl_name = 'locomotives' AND name = 'locomotives_name_index'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(indexes, 1, "existing indexes are re-created");

    conn.execute("INSERT INTO locomotives (name, dcc_address, locomotive_type, command_station_id, created_at, updated_at) VALUES ('Other', 3, 'diesel', 2, 'now', 'now')", [])
        .expect("the same address on another station");
    assert!(
        conn.execute("INSERT INTO locomotives (name, dcc_address, locomotive_type, command_station_id, created_at, updated_at) VALUES ('Same', 3, 'diesel', 2, 'now', 'now')", []).is_err(),
        "the same station still refuses it"
    );
    crate::db::locomotives_migrate(&conn).expect("running it again is harmless");
}

#[test]
fn a_database_that_already_has_the_sync_columns_is_rebuilt_and_keeps_them() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE decoder_profiles (id INTEGER PRIMARY KEY);
         CREATE TABLE command_stations (id INTEGER PRIMARY KEY);
         INSERT INTO command_stations (id) VALUES (1), (2);
         CREATE TABLE locomotives (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            dcc_address INTEGER NOT NULL UNIQUE,
            locomotive_type TEXT NOT NULL,
            command_station_id INTEGER,
            decoder_profile_id INTEGER REFERENCES decoder_profiles(id),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            sync_id TEXT,
            dirty INTEGER NOT NULL DEFAULT 1
         );
         CREATE UNIQUE INDEX locomotives_sync_id ON locomotives (sync_id);
         INSERT INTO locomotives (id, name, dcc_address, locomotive_type, created_at, updated_at, sync_id, dirty)
            VALUES (1, 'Old', 3, 'diesel', 'then', 'then', 'cloud-1', 0);",
    )
    .expect("old table");

    crate::db::locomotives_migrate(&conn).expect("migrate");
    let (sync_id, dirty): (String, i64) = conn
        .query_row("SELECT sync_id, dirty FROM locomotives WHERE id = 1", [], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("the row keeps its sync columns");
    assert_eq!((sync_id.as_str(), dirty), ("cloud-1", 0));
    let sync_index: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'locomotives_sync_id'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sync_index, 1);
    conn.execute("INSERT INTO locomotives (name, dcc_address, locomotive_type, command_station_id, created_at, updated_at) VALUES ('Other', 3, 'diesel', 2, 'now', 'now')", [])
        .expect("the same address on another station");
}
