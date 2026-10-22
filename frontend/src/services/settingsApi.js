import { request } from './api'

// The account's settings, shared by every device signed in to it. The core keeps them and syncs them
export const settingsApi = {
  getAll: () => request('GET', '/api/settings'),
  set: (name, value) => request('PUT', `/api/settings/${name}`, { value }),
}
