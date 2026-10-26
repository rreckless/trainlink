import { request } from './api'

// Consists: groups of locomotives driven together. Members are { locomotive_id, reversed }, the first one leads
export const consistApi = {
  list: () => request('GET', '/api/consists'),
  create: (body) => request('POST', '/api/consists', body),
  update: (id, body) => request('PATCH', `/api/consists/${id}`, body),
  remove: (id) => request('DELETE', `/api/consists/${id}`),
  // Speed 0-126 and direction, sent to every member. A reversed member runs the other way
  setSpeed: (id, speed, forward) => request('POST', `/api/consists/${id}/speed`, { speed, direction: forward }),
  setFunction: (id, index, on) => request('POST', `/api/consists/${id}/function`, { function: index, on }),
}
