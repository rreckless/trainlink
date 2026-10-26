//! Live occupancy, from the Z21's feedback. A block is detected occupied when any feedback linked to it is occupied,
//! and free when every linked feedback has reported free. A block with no report falls back to its own state. A signal
//! shows stop when a block it protects is occupied, and go otherwise.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;

/// The Z21 reports eight inputs per module. Feedback numbers count the same way, from 1 in each module
pub const INPUTS_PER_MODULE: i64 = 8;

/// The module and input (both counted from 1) that a feedback number reads from
pub fn module_and_input(feedback_number: i64) -> (i64, i64) {
    ((feedback_number - 1) / INPUTS_PER_MODULE + 1, (feedback_number - 1) % INPUTS_PER_MODULE + 1)
}

/// What the Z21 last reported for a feedback number, or None if it has not reported that input yet
pub fn reported(z21: &BTreeMap<(i64, i64), bool>, feedback_number: i64) -> Option<bool> {
    z21.get(&module_and_input(feedback_number)).copied()
}

/// Detected occupancy for each block that has feedbacks. Some(true) if any linked feedback is occupied, Some(false) if
/// every linked feedback has reported free, and None if none of them has reported yet
pub fn detected(conn: &Connection, z21: &BTreeMap<(i64, i64), bool>) -> rusqlite::Result<HashMap<i64, bool>> {
    let mut statement = conn.prepare(
        "SELECT bf.block_id, f.feedback_number FROM block_feedbacks bf JOIN feedbacks f ON f.id = bf.feedback_id",
    )?;
    let rows: Vec<(i64, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut result: HashMap<i64, bool> = HashMap::new();
    for (block, number) in rows {
        match reported(z21, number) {
            Some(true) => {
                result.insert(block, true);
            }
            Some(false) => {
                result.entry(block).or_insert(false);
            }
            None => {}
        }
    }
    Ok(result)
}

/// The aspect a signal shows from the occupancy of the blocks it protects: stop if any is occupied, go otherwise
pub fn aspect(occupied: impl IntoIterator<Item = bool>) -> &'static str {
    if occupied.into_iter().any(|value| value) {
        "stop"
    } else {
        "go"
    }
}
