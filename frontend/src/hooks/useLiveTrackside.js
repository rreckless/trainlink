import { useEffect, useState } from 'react'
import { tracksideApi } from '../services/tracksideApi'

const REFRESH_MS = 2000

// The live state of every block and signal, by id: whether each block is occupied now, and the aspect each signal
// shows. Refreshed every two seconds while enabled. A failed read keeps the last values
export function useLiveTrackside(enabled = true) {
  const [live, setLive] = useState({ occupied: {}, aspects: {} })

  useEffect(() => {
    if (!enabled) return undefined
    let cancelled = false
    const read = () =>
      Promise.all([tracksideApi.blocks(), tracksideApi.signals()])
        .then(([blocks, signals]) => {
          if (cancelled) return
          setLive({
            occupied: Object.fromEntries(blocks.map((block) => [block.id, block.occupied_now])),
            aspects: Object.fromEntries(signals.map((signal) => [signal.id, signal.aspect])),
          })
        })
        .catch(() => {})
    read()
    const timer = setInterval(read, REFRESH_MS)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [enabled])

  return live
}
