import { request } from './api'

// Project-wide overviews: every turnout on every layout, and what is missing before automatic driving can work
export const overviewApi = {
  turnouts: () => request('GET', '/api/overview/turnouts'),
  health: () => request('GET', '/api/overview/health'),
}
