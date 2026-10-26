//! The application state shared by the API, the Z21 link, and the moving time tracker

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::auth::Auth;
use crate::sync::SyncStatus;
use crate::moving_time::MovingTime;
use crate::z21::Z21;

/// Everything that changes at runtime, behind one lock so the Z21 reader and the API never disagree
pub struct Inner {
    pub db: Connection,
    pub z21: Z21,
    pub moving: MovingTime,
    /// Route runs started in this session, by route id. Not stored: a run that is not here is stopped at start-up
    pub runs: std::collections::HashMap<i64, crate::automation::Run>,
    /// Blocks reserved by a running route, by block id: the route that holds the block. A route holds the block it is in
    /// and the block it is about to enter. Not stored, like the runs
    pub claims: std::collections::HashMap<i64, i64>,
    /// What the automation has done that someone should know about, newest last, kept to the last 20
    pub notices: Vec<String>,
    /// Blocks that are occupied with no run holding them, by block id, with the message to show. Cleared when the block is free
    pub alarms: std::collections::HashMap<i64, String>,
    /// The blocks that were occupied at the last check. A block only raises an alarm when it becomes occupied
    pub seen_occupied: std::collections::HashSet<i64>,
}

pub struct AppState {
    pub inner: Mutex<Inner>,
    /// Locomotive pictures, one file per locomotive
    pub images: PathBuf,
    /// Sign-in. Kept outside the lock, since its calls go over the network
    pub auth: Auth,
    /// The last sync's outcome, for the status line
    pub sync: Mutex<SyncStatus>,
}

impl AppState {
    /// The data folder is the parent of the picture folder
    pub fn new(db: Connection, images: PathBuf) -> AppState {
        let data = images.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        AppState::with_auth(db, images, Auth::for_app(&data))
    }

    pub fn with_auth(db: Connection, images: PathBuf, auth: Auth) -> AppState {
        AppState {
            inner: Mutex::new(Inner { db, z21: Z21::new(), moving: MovingTime::new(), runs: Default::default(), claims: Default::default(), notices: Vec::new(), alarms: Default::default(), seen_occupied: Default::default() }),
            images,
            auth,
            sync: Mutex::new(SyncStatus::default()),
        }
    }
}
