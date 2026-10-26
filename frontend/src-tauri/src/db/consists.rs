//! Consists: named groups of locomotives that run together. The first member is the lead. A locomotive is in at most
//! one consist, so a drive command never reaches the same locomotive twice

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use super::now;

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub locomotive_id: i64,
    pub name: String,
    pub dcc_address: i64,
    /// True when this locomotive runs the opposite way to the consist
    pub reversed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Consist {
    pub id: i64,
    pub name: String,
    pub members: Vec<Member>,
}

/// A member as given to the API: which locomotive, and whether it runs reversed
pub type MemberInput = (i64, bool);

fn members_of(conn: &Connection, consist_id: i64) -> rusqlite::Result<Vec<Member>> {
    let mut statement = conn.prepare(
        "SELECT l.id, l.name, l.dcc_address, m.reversed FROM consist_members m
         JOIN locomotives l ON l.id = m.locomotive_id
         WHERE m.consist_id = ?1 ORDER BY m.position",
    )?;
    let rows = statement.query_map([consist_id], |row| {
        Ok(Member {
            locomotive_id: row.get(0)?,
            name: row.get(1)?,
            dcc_address: row.get(2)?,
            reversed: row.get::<_, i64>(3)? != 0,
        })
    })?;
    rows.collect()
}

fn load(conn: &Connection, id: i64, name: String) -> rusqlite::Result<Consist> {
    Ok(Consist { id, name, members: members_of(conn, id)? })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Consist>> {
    let mut statement = conn.prepare("SELECT id, name FROM consists ORDER BY name")?;
    let rows: Vec<(i64, String)> = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|(id, name)| load(conn, id, name)).collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Consist>> {
    let name: Option<String> = conn.query_row("SELECT name FROM consists WHERE id = ?1", [id], |row| row.get(0)).optional()?;
    name.map(|name| load(conn, id, name)).transpose()
}

/// Write the members in order, replacing any earlier ones
fn write_members(tx: &Transaction, consist_id: i64, members: &[MemberInput]) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM consist_members WHERE consist_id = ?1", [consist_id])?;
    for (position, (locomotive_id, reversed)) in members.iter().enumerate() {
        tx.execute(
            "INSERT INTO consist_members (consist_id, locomotive_id, position, reversed) VALUES (?1, ?2, ?3, ?4)",
            params![consist_id, locomotive_id, position as i64, *reversed as i64],
        )?;
    }
    Ok(())
}

pub fn insert(conn: &mut Connection, name: &str, members: &[MemberInput]) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;
    let stamp = now();
    tx.execute(
        "INSERT INTO consists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
        params![name, stamp],
    )?;
    let id = tx.last_insert_rowid();
    write_members(&tx, id, members)?;
    tx.commit()?;
    Ok(id)
}

/// Change the name and, when given, the members. Returns false if the consist does not exist
pub fn update(conn: &mut Connection, id: i64, name: Option<&str>, members: Option<&[MemberInput]>) -> rusqlite::Result<bool> {
    let tx = conn.transaction()?;
    let changed = match name {
        Some(name) => tx.execute("UPDATE consists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, name, now()])?,
        None => tx.query_row("SELECT COUNT(*) FROM consists WHERE id = ?1", [id], |row| row.get::<_, i64>(0))? as usize,
    };
    if changed == 0 {
        return Ok(false);
    }
    if let Some(members) = members {
        write_members(&tx, id, members)?;
    }
    tx.commit()?;
    Ok(true)
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM consists WHERE id = ?1", [id])? > 0)
}

/// The consist a locomotive belongs to, if any
pub fn consist_of(conn: &Connection, locomotive_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT consist_id FROM consist_members WHERE locomotive_id = ?1", [locomotive_id], |row| row.get(0))
        .optional()
}
