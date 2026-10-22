import { invoke } from '@tauri-apps/api/core'

// The desktop app's native picture dialog. The core reads the picture, so the webview never reads the file itself.
// Resolves to a File the upload uses as it would a browser file, or null when the user cancels
export async function pickPictureInDesktop() {
  const chosen = await invoke('pick_picture_file')
  if (!chosen) return null
  const bytes = Uint8Array.from(atob(chosen.base64), (char) => char.charCodeAt(0))
  return new File([bytes], chosen.name, { type: chosen.mime })
}
