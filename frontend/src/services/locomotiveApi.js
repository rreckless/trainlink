import { BASE_URL, request } from './api'

// Pictures are sent as form data, not JSON
async function sendForm(method, path, file) {
  const form = new FormData()
  if (file) form.append('file', file)
  const response = await fetch(`${BASE_URL}${path}`, { method, body: form })
  if (!response.ok) {
    let message = `Request failed (${response.status})`
    try {
      const data = await response.json()
      if (typeof data.detail === 'string') message = data.detail
    } catch (error) {
      // No JSON body. Keep the status message
    }
    throw new Error(message)
  }
  return response.json()
}

// Locomotive roster, setup, and Z21 locomotive commands
export const locomotiveApi = {
  list: () => request('GET', '/api/locomotives'),
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
