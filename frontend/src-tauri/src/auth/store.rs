//! Where the session is kept: a file in the app's data folder, and memory in tests

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::Session;

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Option<Session>;
    fn save(&self, session: &Session) -> Result<(), String>;
    fn clear(&self);
}

/// The session in a file in the app's data folder. On Unix it is readable only by the user account; on Windows the
/// folder under the user's AppData already is
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: &Path) -> FileStore {
        FileStore { path: path.to_path_buf() }
    }
}

impl SessionStore for FileStore {
    fn load(&self) -> Option<Session> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        let text = serde_json::to_string(session).map_err(|error| error.to_string())?;
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        // Written to a new file with owner-only permissions, then moved into place, so a crash never leaves half a file
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())?;
        }
        std::fs::rename(&temporary, &self.path).map_err(|error| error.to_string())
    }

    fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Keeps the session in memory. For tests
pub struct MemoryStore {
    session: Mutex<Option<Session>>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore { session: Mutex::new(None) }
    }
}

impl SessionStore for MemoryStore {
    fn load(&self) -> Option<Session> {
        self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        *self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(session.clone());
        Ok(())
    }

    fn clear(&self) {
        *self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}
