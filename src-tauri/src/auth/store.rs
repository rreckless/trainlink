//! Where the session is kept: the operating system's keychain in the app, and memory in tests

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::Session;

const SERVICE: &str = "com.trainlink.app";
const ACCOUNT: &str = "supabase-session";

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Option<Session>;
    fn save(&self, session: &Session) -> Result<(), String>;
    fn clear(&self);
}

/// The session in a file in the app's data folder, readable only by the user account. Used while builds are
/// not signed with a developer identity: the Keychain asks again after every build in that case
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
        use std::os::unix::fs::PermissionsExt;
        let text = serde_json::to_string(session).map_err(|error| error.to_string())?;
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        // Written to a new file with owner-only permissions, then moved into place, so a crash never leaves half a file
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, &self.path).map_err(|error| error.to_string())
    }

    fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The macOS Keychain. Kept for when the app is signed with a developer identity, which keeps the Keychain from
/// asking again after each build
#[allow(dead_code)]
pub struct KeychainStore;

#[allow(dead_code)]
impl KeychainStore {
    pub fn new() -> KeychainStore {
        KeychainStore
    }

    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|error| error.to_string())
    }
}

#[allow(dead_code)]
impl SessionStore for KeychainStore {
    fn load(&self) -> Option<Session> {
        let text = Self::entry().ok()?.get_password().ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        let text = serde_json::to_string(session).map_err(|error| error.to_string())?;
        Self::entry()?.set_password(&text).map_err(|error| error.to_string())
    }

    fn clear(&self) {
        if let Ok(entry) = Self::entry() {
            let _ = entry.delete_credential();
        }
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
