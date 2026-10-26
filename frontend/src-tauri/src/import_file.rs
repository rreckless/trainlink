//! Choosing a file in the desktop app: an AnyRail export, or a locomotive picture. The native file dialog picks the file, and the core reads it. The page
//! never reads the file itself, because the desktop webview cannot read every file the dialog can pick.

use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// The largest picture the app keeps, the same limit the core applies to uploads
const MAX_PICTURE_BYTES: u64 = 16 * 1024 * 1024;

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

/// A picture the user chose: its name and type, and its bytes encoded as base64
#[derive(Serialize)]
pub struct ChosenPicture {
    pub name: String,
    pub mime: String,
    pub base64: String,
}

/// The picture type for a file extension, if the app accepts it
pub fn picture_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Shows the picture dialog, then reads the chosen picture. None if the user cancels. The size limit is checked
/// before the file is read, so a large file is refused without being loaded
#[tauri::command]
pub async fn pick_picture_file(app: AppHandle) -> Result<Option<ChosenPicture>, String> {
    let Some(picked) = app.dialog().file().add_filter("Picture", &["jpg", "jpeg", "png", "webp"]).blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|error| format!("Could not use the chosen file: {error}"))?;
    let Some(mime) = picture_type(&path) else {
        return Err("Picture must be a JPEG, PNG, or WebP image".into());
    };
    let size = std::fs::metadata(&path).map_err(|error| format!("Could not read the file: {error}"))?.len();
    if size > MAX_PICTURE_BYTES {
        return Err("Picture must be 16 MB or smaller".into());
    }
    let bytes = std::fs::read(&path).map_err(|error| format!("Could not read the file: {error}"))?;
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Some(ChosenPicture { name, mime: mime.into(), base64: STANDARD.encode(bytes) }))
}
