import { apiFetch, BASE_URL, IS_DESKTOP, readReply, request } from './api'

// Pictures are sent as form data in the browser. The desktop core takes the picture's bytes as the body
async function sendForm(method, path, file) {
  if (IS_DESKTOP) {
    // Read the bytes first. A File sent as the body goes as a stream, which the app's own protocol does not receive
    const bytes = file ? await file.arrayBuffer() : undefined
    const response = await apiFetch(`${BASE_URL}${path}`, {
      method,
      headers: file ? { 'Content-Type': file.type || 'application/octet-stream' } : undefined,
      body: bytes,
    })
    return readReply(response)
  }
  const form = new FormData()
  if (file) form.append('file', file)
  const response = await apiFetch(`${BASE_URL}${path}`, { method, body: form })
  return readReply(response)
}

// Locomotive roster, setup, and Z21 locomotive commands
export const locomotiveApi = {
  list: () => request('GET', '/api/locomotives'),
  // The lowest DCC address no locomotive uses
  nextAddress: () => request('GET', '/api/locomotives/next-address'),
  create: (body) => request('POST', '/api/locomotives', body),
  update: (id, body) => request('PATCH', `/api/locomotives/${id}`, body),
  remove: (id) => request('DELETE', `/api/locomotives/${id}`),
  uploadImage: (id, file) => sendForm('POST', `/api/locomotives/${id}/image`, file),
  removeImage: (id) => request('DELETE', `/api/locomotives/${id}/image`),
  // Speed 0-126 and direction, as the Z21 takes them
  setSpeed: (address, speed, forward) =>
    request('POST', '/api/z21/loco/speed', { address, speed, direction: forward }),
  emergencyStop: () => request('POST', '/api/z21/emergency-stop', { confirm: true }),
  resumeTrackPower: () => request('POST', '/api/z21/track-power/on'),
  // Speed, direction, and functions the Z21 last reported, by address
  getStates: (addresses) => request('GET', `/api/z21/locos?addresses=${addresses.join(',')}`),
  setFunction: (address, index, on) => request('POST', '/api/z21/loco/function', { address, function: index, on }),
}

// Full address of a locomotive's picture, or null when it has none
export const pictureUrl = (locomotive) => (locomotive?.image_url ? `${BASE_URL}${locomotive.image_url}` : null)
