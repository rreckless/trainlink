import { ENTITY_KINDS } from '../layout/model'

const BASE_URL = 'http://localhost:8000'

async function request(method, path, body) {
  const response = await fetch(`${BASE_URL}${path}`, {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  })

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

export const layoutApi = {
  listLayouts: () => request('GET', '/api/layouts'),
  getTrackLibrary: () => request('GET', '/api/track-library'),
  getLayout: (id) => request('GET', `/api/layouts/${id}`),
  createLayout: (body) => request('POST', '/api/layouts', body),
  updateLayout: (id, body) => request('PATCH', `/api/layouts/${id}`, body),
  importAnyRail: (body) => request('POST', '/api/layouts/anyrail-import', body),
  throwTurnout: (turnout, output) => request('POST', `/api/z21/turnouts/${turnout}/throw`, { output }),
  getTurnoutStates: (turnouts) => request('GET', `/api/z21/turnouts?addresses=${turnouts.join(',')}`),

  listEntities: (kind, layoutId) =>
    request('GET', `${ENTITY_KINDS[kind].path}?layout_id=${layoutId}`),
  createEntity: (kind, body) => request('POST', ENTITY_KINDS[kind].path, body),
  updateEntity: (kind, id, body) => request('PATCH', `${ENTITY_KINDS[kind].path}/${id}`, body),
  deleteEntity: (kind, id) => request('DELETE', `${ENTITY_KINDS[kind].path}/${id}`),
}
