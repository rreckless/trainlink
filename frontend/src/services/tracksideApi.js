import { request } from './api'

// Blocks and signals as the core reports them. Each block carries its live occupancy: occupied_now is what the
// Switchboard shows, and detected is what the Z21's feedback reports (null where no feedback has reported yet).
// Each signal shows the aspect its blocks give, when it protects any, and aspect_source says which rule applies
export const tracksideApi = {
  blocks: () => request('GET', '/api/blocks'),
  signals: () => request('GET', '/api/signals'),
}
