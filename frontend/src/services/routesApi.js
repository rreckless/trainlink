import { request } from './api'

// Routes: an ordered list of blocks in one layout. The core reports each route's unlinked_steps, the pairs of blocks
// that do not follow the first block's next-block link
export const routesApi = {
  list: (layoutId) => request('GET', `/api/routes?layout_id=${layoutId}`),
  create: (body) => request('POST', '/api/routes', body),
  update: (id, body) => request('PATCH', `/api/routes/${id}`, body),
  remove: (id) => request('DELETE', `/api/routes/${id}`),
}
