import { useEffect, useRef, useCallback } from 'react'
import { useStore } from '../store'

export const useWebSocket = (url = 'ws://localhost:8000/ws') => {
  const wsRef = useRef(null)
  const setConnected = useStore((state) => state.setConnected)
  const setClientId = useStore((state) => state.setClientId)

  const connect = useCallback(() => {
    const clientId = `client_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
    const wsUrl = `${url}/${clientId}`

    wsRef.current = new WebSocket(wsUrl)

    wsRef.current.onopen = () => {
      console.log('WebSocket connected:', clientId)
      setConnected(true)
      setClientId(clientId)
    }

    wsRef.current.onmessage = (event) => {
      const data = JSON.parse(event.data)
      console.log('Message received:', data)
      // Handle messages here
    }

    wsRef.current.onerror = (error) => {
      console.error('WebSocket error:', error)
      setConnected(false)
    }

    wsRef.current.onclose = () => {
      console.log('WebSocket disconnected')
      setConnected(false)
      // Attempt to reconnect after 3 seconds
      setTimeout(connect, 3000)
    }
  }, [url, setConnected, setClientId])

  const send = useCallback((message) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message))
    }
  }, [])

  useEffect(() => {
    connect()

    return () => {
      if (wsRef.current) {
        wsRef.current.close()
      }
    }
  }, [connect])

  return { send, connected: wsRef.current?.readyState === WebSocket.OPEN }
}
