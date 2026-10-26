import { request } from './api'

// The command stations this device has, by name. A turnout or locomotive can be set to one of them
export const commandStationApi = {
  list: () => request('GET', '/api/command-stations'),
}
