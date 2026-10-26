import { request } from './api'

// Project-wide overviews: every turnout on every layout, and what is missing before automatic driving can work
export const overviewApi = {
  turnouts: () => request('GET', '/api/overview/turnouts'),
  health: () => request('GET', '/api/overview/health'),
  setup: () => request('GET', '/api/overview/setup'),
  feedbackMonitor: () => request('GET', '/api/z21/feedback/monitor'),
  connectionLog: () => request('GET', '/api/overview/log'),
  supportPackage: () => request('GET', '/api/overview/support'),
  // A turnout's switching time, manual flag, position feedback, group and notes. Only the fields sent are changed
  updateTurnout: (layoutId, placedId, body) => request('PATCH', `/api/layouts/${layoutId}/turnouts/${placedId}`, body),
}
