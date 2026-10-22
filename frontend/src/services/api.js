import { invoke } from '@tauri-apps/api/core'
import { askText } from './dialogs'

// The desktop and mobile apps answer API calls from their own core. Windows and Android webviews reach a custom scheme as
// an http address
export const IS_DESKTOP = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
export const IS_ANDROID = typeof navigator !== 'undefined' && navigator.userAgent.includes('Android')
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

// The bytes of a request body, for the command channel
async function bodyBytes(body) {
  if (body === undefined || body === null) return new Uint8Array()
  if (typeof body === 'string') return new TextEncoder().encode(body)
  if (body instanceof ArrayBuffer) return new Uint8Array(body)
  if (body instanceof Blob) return new Uint8Array(await body.arrayBuffer())
  throw new Error('This upload cannot be sent from the Android app')
}

function toBase64(bytes) {
  let binary = ''
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(binary)
}

function fromBase64(text) {
  return Uint8Array.from(atob(text), (char) => char.charCodeAt(0))
}

// On Android a write goes to the core as a command, because the webview does not pass the body to the address. The
// reply is rebuilt as a Response, so the callers read it the same way
async function sendThroughCore(url, init) {
  const headers = new Headers(init.headers ?? {})
  const bytes = await bodyBytes(init.body)
  const reply = await invoke('api_call', {
    request: {
      method: init.method ?? 'POST',
      path: url.slice(BASE_URL.length),
      contentType: headers.get('Content-Type') ?? 'application/json',
      bodyBase64: toBase64(bytes),
    },
  })
  const payload = reply.status === 204 ? null : fromBase64(reply.bodyBase64)
  return new Response(payload, { status: reply.status, headers: { 'Content-Type': reply.contentType } })
}

// Every request to the server goes through here, so the sign-in cookie is sent and a refusal can ask for the token
export async function apiFetch(url, init = {}) {
  const options = { ...init, credentials: IS_DESKTOP ? 'omit' : 'include' }
  const isWrite = !['GET', 'HEAD'].includes((init.method ?? 'GET').toUpperCase())
  const send = (target, sendOptions) => (IS_ANDROID && isWrite ? sendThroughCore(target, sendOptions) : fetch(target, sendOptions))
  const response = await send(url, options)
  if (response.status !== 401 || IS_DESKTOP || Date.now() - declinedAt < ASK_AGAIN_AFTER_MS) return response
  const pending = signingIn ?? (signingIn = signIn().finally(() => { signingIn = null }))
  if (!(await pending)) {
    declinedAt = Date.now()
    return response
  }
  return send(url, options)
}

export async function request(method, path, body) {
  const response = await apiFetch(`${BASE_URL}${path}`, {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  })
  return readReply(response)
}
