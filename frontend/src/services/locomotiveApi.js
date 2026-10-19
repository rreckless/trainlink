import { request } from './api'

// Locomotive roster and Z21 locomotive commands
export const locomotiveApi = {
  list: () => request('GET', '/api/locomotives'),
  create: (body) => request('POST', '/api/locomotives', body),
  remove: (id) => request('DELETE', `/api/locomotives/${id}`),
  // Speed 0-126 and direction, as the Z21 takes them
  setSpeed: (address, speed, forward) =>
    request('POST', '/api/z21/loco/speed', { address, speed, direction: forward }),
  emergencyStop: () => request('POST', '/api/z21/emergency-stop', { confirm: true }),
  resumeTrackPower: () => request('POST', '/api/z21/track-power/on'),
}
