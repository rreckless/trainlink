// The desktop app answers API calls from its own core at trainlink://localhost. In the browser the calls go to
// the web backend, at VITE_API_BASE when it is set
export const IS_DESKTOP = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
export const BASE_URL = IS_DESKTOP ? 'trainlink://localhost' : (import.meta.env.VITE_API_BASE ?? 'http://localhost:8000')

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

export async function request(method, path, body) {
  const response = await fetch(`${BASE_URL}${path}`, {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  })
  return readReply(response)
}
