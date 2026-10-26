import { request } from './api'

// Routes: an ordered list of blocks in one layout. The core reports each route's unlinked_steps, the pairs of blocks
// that do not follow the first block's next-block link
export const routesApi = {
  list: (layoutId) => request('GET', `/api/routes?layout_id=${layoutId}`),
  create: (body) => request('POST', '/api/routes', body),
  update: (id, body) => request('PATCH', `/api/routes/${id}`, body),
  remove: (id) => request('DELETE', `/api/routes/${id}`),
  // Start a route with a locomotive in its first block, or stop it
  run: (id, locomotiveId) => request('POST', `/api/routes/${id}/run`, { locomotive_id: locomotiveId }),
  stop: (id) => request('POST', `/api/routes/${id}/stop`),
  // What the automation has done that matters: stops made to avoid a collision or a crash, newest last
  notices: () => request('GET', '/api/routes/notices'),
  // Blocks with a train that no run has claimed, until they are free. Dismissing hides the alarm but not the hold
  alarms: () => request('GET', '/api/routes/alarms'),
  dismissAlarm: (blockId) => request('POST', `/api/routes/alarms/${blockId}/dismiss`),
}
