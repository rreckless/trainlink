import { request } from './api'

// Decoder profiles: the CVs and function names of a decoder type. The built-in NMRA standard profile cannot be deleted
export const decoderProfilesApi = {
  list: () => request('GET', '/api/decoder-profiles'),
  get: (id) => request('GET', `/api/decoder-profiles/${id}`),
  create: (body) => request('POST', '/api/decoder-profiles', body),
  remove: (id) => request('DELETE', `/api/decoder-profiles/${id}`),
}
