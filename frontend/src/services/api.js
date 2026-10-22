import { askText } from './dialogs'

// The desktop and mobile apps answer API calls from their own core. Windows and Android webviews reach a custom scheme as
// an http address
export const IS_DESKTOP = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
const IS_ANDROID = typeof navigator !== 'undefined' && navigator.userAgent.includes('Android')
const IS_WINDOWS = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows')
const DESKTOP_BASE = IS_WINDOWS || IS_ANDROID ? 'http://trainlink.localhost' : 'trainlink://localhost'
// The native file dialog and its reading in the core: desktop only. Android uses the page's own file picker
export const NATIVE_FILE_PICKER = IS_DESKTOP && !IS_ANDROID
// In the browser the API is the server the page came from. Development sets VITE_API_BASE to reach it
export const BASE_URL = IS_DESKTOP ? DESKTOP_BASE : (import.meta.env.VITE_API_BASE ?? window.location.origin)

// The reply's JSON, or null for no content. An error reply throws with the server's detail message
export async function readReply(response) {
  if (!response.ok) {
    let message = `Request failed (${response.status})`
    try {
      const data = await response.json()
      if (typeof data.detail === 'string') message = data.detail
    } catch (error) {
      // Response had no JSON body. Keep the status message
    }
    throw new Error(message)
  }

  return response.status === 204 ? null : response.json()
}

// A server on another device asks for its token. A 401 asks once, signs in, and sends the request again. After a
// refusal, the page does not ask again for a minute, so polling does not keep asking
const ASK_AGAIN_AFTER_MS = 60_000
let declinedAt = 0
let signingIn = null

async function signIn() {
  const token = await askText('Enter the TrainLink server token. The server prints where it is kept when it starts', '', { okLabel: 'Sign in' })
  if (!token) return false
  try {
    const response = await fetch(`${BASE_URL}/login`, {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: token.trim() }),
    })
    return response.ok
  } catch (error) {
    return false
  }
}

// Every request to the server goes through here, so the sign-in cookie is sent and a refusal can ask for the token
export async function apiFetch(url, init = {}) {
  const options = { ...init, credentials: IS_DESKTOP ? 'omit' : 'include' }
  const response = await fetch(url, options)
  if (response.status !== 401 || IS_DESKTOP || Date.now() - declinedAt < ASK_AGAIN_AFTER_MS) return response
  const pending = signingIn ?? (signingIn = signIn().finally(() => { signingIn = null }))
  if (!(await pending)) {
    declinedAt = Date.now()
    return response
  }
  return fetch(url, options)
}

export async function request(method, path, body) {
  const response = await apiFetch(`${BASE_URL}${path}`, {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  })
  return readReply(response)
}
