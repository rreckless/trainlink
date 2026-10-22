import { useState } from 'react'
import { useStore } from '../store'
import { refreshZ21Status } from './useZ21Status'
import { BASE_URL } from '../services/api'

const Z21_API = `${BASE_URL}/api/z21`

// Connect to and disconnect from the Z21. Settings and the Control page share the address and the actions
export function useZ21Connection() {
  const host = useStore((state) => state.z21Host)
  const port = useStore((state) => state.z21Port)
  const setZ21Address = useStore((state) => state.setZ21Address)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(null)

  const connect = async () => {
    setLoading(true)
    setError(null)
    try {
      const response = await fetch(`${Z21_API}/connect`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ host, port }),
      })
      const data = await response.json()
      if (!response.ok) setError(data.detail || 'Failed to connect')
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
      refreshZ21Status()
    }
  }

  const disconnect = async () => {
    setLoading(true)
    try {
      const response = await fetch(`${Z21_API}/disconnect`, { method: 'POST' })
      if (response.ok) setError(null)
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
      refreshZ21Status()
    }
  }

  return { host, port, setZ21Address, loading, error, setError, connect, disconnect }
}
