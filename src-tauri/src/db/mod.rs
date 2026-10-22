//! SQLite storage. The tables match the SQLAlchemy models the web backend used

use std::path::Path;

use rusqlite::{Connection, Error, ErrorCode};

pub mod layouts;
pub mod locomotives;
pub mod sync;
pub mod trackside;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS layouts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    width REAL NOT NULL,
    height REAL NOT NULL,
    scale TEXT NOT NULL,
    track_gauge TEXT,
    elements TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS command_stations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    station_type TEXT NOT NULL,
    port TEXT NOT NULL,
    connected INTEGER NOT NULL DEFAULT 0,
    last_heartbeat TEXT,
    firmware_version TEXT,
    supported_features TEXT NOT NULL DEFAULT '{}',
    settings TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS locomotives (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    dcc_address INTEGER NOT NULL UNIQUE,
    locomotive_type TEXT NOT NULL,
    length REAL,
    speed INTEGER NOT NULL DEFAULT 0,
    direction TEXT NOT NULL DEFAULT 'stop',
    functions INTEGER NOT NULL DEFAULT 0,
    function_labels TEXT,
    image_file TEXT,
    travelled_time REAL NOT NULL DEFAULT 0,
    travelled_distance REAL NOT NULL DEFAULT 0,
    command_station_id INTEGER REFERENCES command_stations(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS blocks (
    id INTEGER PRIMARY KEY,
    layout_id INTEGER REFERENCES layouts(id),
    name TEXT NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    max_speed INTEGER NOT NULL DEFAULT 100,
    length REAL,
    state TEXT NOT NULL DEFAULT 'free',
    occupied INTEGER NOT NULL DEFAULT 0,
    occupancy_locomotive_id INTEGER REFERENCES locomotives(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS signals (
    id INTEGER PRIMARY KEY,
    layout_id INTEGER REFERENCES layouts(id),
    name TEXT NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    signal_type TEXT NOT NULL,
    aspect TEXT NOT NULL DEFAULT 'stop',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS feedbacks (
    id INTEGER PRIMARY KEY,
    layout_id INTEGER REFERENCES layouts(id),
    name TEXT NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    feedback_number INTEGER NOT NULL,
    state INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS block_signals (
    block_id INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    signal_id INTEGER NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    PRIMARY KEY (block_id, signal_id)
);
CREATE TABLE IF NOT EXISTS block_feedbacks (
    block_id INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    feedback_id INTEGER NOT NULL REFERENCES feedbacks(id) ON DELETE CASCADE,
    PRIMARY KEY (block_id, feedback_id)
);
CREATE TABLE IF NOT EXISTS routes (
    id INTEGER PRIMARY KEY,
    layout_id INTEGER REFERENCES layouts(id),
    name TEXT NOT NULL,
    block_sequence TEXT NOT NULL,
    speed_profile TEXT,
    dwell_times TEXT,
    auto_return INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_running INTEGER NOT NULL DEFAULT 0,
    current_locomotive_id INTEGER REFERENCES locomotives(id),
    current_block_index INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

/// Open (or create) the database file and make sure every table exists
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    setup(&connection)?;
    Ok(connection)
}

/// An in-memory database, for tests
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let connection = Connection::open_in_memory()?;
    setup(&connection)?;
    Ok(connection)
}

fn setup(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    connection.execute_batch(SCHEMA)?;
    sync::migrate(connection)
}

/// The current UTC time as the API reports it, e.g. 2026-10-20T10:21:00.123456
pub fn now() -> String {
    chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}

/// True if the error is a UNIQUE constraint failing
pub fn is_unique_violation(error: &Error) -> bool {
    matches!(error, Error::SqliteFailure(failure, _) if failure.code == ErrorCode::ConstraintViolation)
}
