import { request } from './api'

// Sync status and "sync now", through the core
export const syncApi = {
  status: () => request('GET', '/api/sync/status'),
  syncNow: () => request('POST', '/api/sync/now'),
}
