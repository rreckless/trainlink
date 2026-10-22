//! Moving time per locomotive: the seconds a locomotive is at a speed above zero.
//! Time runs from the speed the Z21 reports and from the speed commands this app sends. It is kept in
//! memory while the locomotive moves, saved when it stops, and saved at most every 30 seconds while it moves.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::db::locomotives;

pub const FLUSH_INTERVAL: Duration = Duration::from_secs(30);

pub struct MovingTime {
    /// Address -> when the locomotive started moving, or was last saved
    since: HashMap<i64, Instant>,
    last_flush: Instant,
}

impl MovingTime {
    pub fn new() -> MovingTime {
        MovingTime { since: HashMap::new(), last_flush: Instant::now() }
    }

    /// Record a locomotive's speed. An unknown speed counts as not moving
    pub fn observe(&mut self, db: &Connection, address: i64, speed: Option<i64>, now: Instant) {
        let moving = speed.is_some_and(|value| value > 0);
        if moving && !self.since.contains_key(&address) {
            self.since.insert(address, now);
        } else if !moving {
            if let Some(started) = self.since.remove(&address) {
                save(db, address, now.duration_since(started));
            }
        }
    }

    /// Save the time of moving locomotives, at most every FLUSH_INTERVAL
    pub fn flush_if_due(&mut self, db: &Connection, now: Instant) {
        if now.duration_since(self.last_flush) < FLUSH_INTERVAL {
            return;
        }
        self.last_flush = now;
        for (address, started) in self.since.iter_mut() {
            save(db, *address, now.duration_since(*started));
            *started = now;
        }
    }
}

fn save(db: &Connection, address: i64, elapsed: Duration) {
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        // A locomotive that was deleted while moving has nothing to save to
        let _ = locomotives::add_travelled_time(db, address, seconds);
    }
}
