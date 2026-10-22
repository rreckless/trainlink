//! Choosing an AnyRail file in the desktop app. The native file dialog picks the file, and the core reads it. The page
//! never reads the file itself, because the desktop webview cannot read every file the dialog can pick.

use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
pub struct ChosenFile {
    pub name: String,
    pub xml: String,
}

/// The text of a file, or the reason it could not be read
pub fn read_xml(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("Could not read the file: {error}"))
}

/// Shows the file dialog, then reads the chosen XML file. None if the user cancels
#[tauri::command]
pub async fn pick_anyrail_file(app: AppHandle) -> Result<Option<ChosenFile>, String> {
    let Some(picked) = app.dialog().file().add_filter("AnyRail layout", &["xml"]).blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|error| format!("Could not use the chosen file: {error}"))?;
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let xml = read_xml(&path)?;
    Ok(Some(ChosenFile { name, xml }))
}
