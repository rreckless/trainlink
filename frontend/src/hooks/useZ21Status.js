import { useEffect } from 'react'
import { useStore } from '../store'
import { apiFetch, BASE_URL } from '../services/api'

const STATUS_URL = `${BASE_URL}/api/z21/status`

// Fetch the Z21 status once and store it. Anything that changes the Z21 calls this afterwards
export const refreshZ21Status = async () => {
  let z21 = { connected: false, state: 'disconnected', lastHeartbeat: null, xbusVersion: null, model: null, firmware: null, features: [] }
  try {
    const response = await apiFetch(STATUS_URL)
    if (response.ok) {
      const data = await response.json()
      z21 = {
        connected: data.connected,
        state: data.connected ? data.state : 'disconnected',
        lastHeartbeat: data.last_heartbeat,
        xbusVersion: data.xbus_version ?? null,
        model: data.model ?? null,
        firmware: data.firmware ?? null,
        features: data.features ?? [],
      }
    }
  } catch (error) {
    // TrainLink server unreachable: treat the Z21 as disconnected
  }
  useStore.getState().setZ21Status(z21)
}

// Poll the Z21 status. Call once from the top-level component
export const useZ21StatusPolling = (intervalMs = 3000) => {
  useEffect(() => {
    refreshZ21Status()
    const interval = setInterval(refreshZ21Status, intervalMs)
    return () => clearInterval(interval)
  }, [intervalMs])
}
