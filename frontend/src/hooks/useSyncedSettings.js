import { useEffect } from 'react'
import { useStore } from '../store'
import { settingsApi } from '../services/settingsApi'

const REFRESH_MS = 30_000

// Applies the account's settings to this device, and picks up changes made on other devices. A setting the account
// does not have yet takes this device's current value, so the account starts with the settings from this device
export function useSyncedSettings(signedIn) {
  useEffect(() => {
    if (!signedIn) return undefined
    let applied = null
    let cancelled = false

    const load = async () => {
      try {
        const saved = await settingsApi.getAll()
        if (cancelled) return
        const { z21Host, z21Port, setZ21Address } = useStore.getState()
        if (saved['z21.host'] === undefined) await settingsApi.set('z21.host', z21Host)
        if (saved['z21.port'] === undefined) await settingsApi.set('z21.port', z21Port)
        const remote = { host: saved['z21.host'] ?? z21Host, port: saved['z21.port'] ?? z21Port }
        // Apply only a change from the account, so an address being typed on this device is not overwritten
        if (!applied || applied.host !== remote.host || applied.port !== remote.port) {
          setZ21Address(remote.host, remote.port)
        }
        applied = remote
      } catch {
        // Offline or refused: this device keeps its own values
      }
    }

    load()
    const timer = setInterval(load, REFRESH_MS)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [signedIn])
}
