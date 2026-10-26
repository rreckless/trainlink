//! SQLite storage. The tables match the SQLAlchemy models the web backend used

use std::path::Path;

use rusqlite::{Connection, Error, ErrorCode};

pub mod command_stations;
pub mod decoder_profiles;
pub mod consists;
pub mod layouts;
pub mod locomotives;
pub mod settings;
pub mod sync;
pub mod trackside;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
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
CREATE TABLE IF NOT EXISTS decoder_profiles (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    manufacturer TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    builtin INTEGER NOT NULL DEFAULT 0,
    cvs TEXT NOT NULL DEFAULT '[]',
    functions TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS locomotives (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    dcc_address INTEGER NOT NULL,
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
    decoder_profile_id INTEGER REFERENCES decoder_profiles(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS consists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    address INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS consist_members (
    consist_id INTEGER NOT NULL REFERENCES consists(id) ON DELETE CASCADE,
    locomotive_id INTEGER NOT NULL UNIQUE REFERENCES locomotives(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    reversed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (consist_id, locomotive_id)
);
CREATE TABLE IF NOT EXISTS blocks (
    id INTEGER PRIMARY KEY,
    layout_id INTEGER REFERENCES layouts(id),
    name TEXT NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    max_speed INTEGER NOT NULL DEFAULT 100,
    next_block_id INTEGER REFERENCES blocks(id),
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
    consists::migrate(connection)?;
    trackside::migrate(connection)?;
    locomotives_migrate(connection)?;
    decoder_profiles::seed(connection)?;
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

/// The locomotives table as it is now, with the sync columns. A database made before some of these columns existed is
/// rebuilt to this shape, which keeps every column it had
const LOCOMOTIVES_TABLE: &str = "(
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    dcc_address INTEGER NOT NULL,
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
    decoder_profile_id INTEGER REFERENCES decoder_profiles(id),
    pending_command_station TEXT,
    sync_id TEXT,
    dirty INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)";

/// Bring an older locomotives table up to date: no single address for all stations, the columns added since, and the
/// address index that makes each command station's addresses unique
pub(crate) fn locomotives_migrate(conn: &Connection) -> rusqlite::Result<()> {
    let table_sql: String = conn.query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'locomotives'", [], |row| row.get(0))?;
    if table_sql.contains("dcc_address INTEGER NOT NULL UNIQUE") {
        rebuild_locomotives_without_global_address(conn)?;
    }
    let columns = table_columns(conn, "locomotives")?;
    let added = [
        ("decoder_profile_id", "INTEGER REFERENCES decoder_profiles(id)"),
        ("pending_command_station", "TEXT"),
        ("group_name", "TEXT"),
        ("max_speed", "INTEGER"),
        ("reverse_direction", "INTEGER NOT NULL DEFAULT 0"),
        ("colour", "TEXT"),
    ];
    for (column, definition) in added {
        if !columns.iter().any(|known| known == column) {
            conn.execute_batch(&format!("ALTER TABLE locomotives ADD COLUMN {column} {definition};"))?;
        }
    }
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS locomotives_station_address ON locomotives (COALESCE(command_station_id, 0), dcc_address);",
    )
}

fn table_columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    conn.prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect()
}

/// SQLite cannot drop a UNIQUE constraint, so the table is copied into one without it, as SQLite's own guide describes.
/// Every column that exists is copied, and every index is made again. Foreign keys are off while the table is replaced,
/// so the rows that refer to locomotives are not deleted with the old table
fn rebuild_locomotives_without_global_address(conn: &Connection) -> rusqlite::Result<()> {
    let columns = table_columns(conn, "locomotives")?.join(", ");
    let indexes: Vec<String> = conn
        .prepare("SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = 'locomotives' AND sql IS NOT NULL")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut copy = format!(
        "BEGIN; CREATE TABLE locomotives_new {LOCOMOTIVES_TABLE}; INSERT INTO locomotives_new ({columns}) SELECT {columns} FROM locomotives; DROP TABLE locomotives; ALTER TABLE locomotives_new RENAME TO locomotives;"
    );
    for index in indexes {
        copy.push_str(&format!("{index};"));
    }
    copy.push_str(" COMMIT;");
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = conn.execute_batch(&copy);
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK;");
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}
