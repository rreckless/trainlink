import { useCallback, useEffect, useMemo, useState } from 'react'
import { layoutApi } from '../services/layoutApi'
import { turnoutTarget } from '../layout/track'

// Outputs of the DCC turnouts placed in a layout, and switching them. While `active`, the Z21 is read
// every two seconds. onError receives a message, or '' to clear one
export function useTurnoutControl(pieces, active, onError) {
  // Last reported output of each turnout, keyed by address. null is unknown
  const [turnoutStates, setTurnoutStates] = useState({})
  const addressKey = useMemo(
    () => pieces.filter((piece) => piece.dcc_address !== undefined).map((piece) => piece.dcc_address).join(','),
    [pieces]
  )

  useEffect(() => {
    if (!active || addressKey === '') return undefined
    const turnouts = addressKey.split(',').map(Number)
    let cancelled = false
    const read = () =>
      layoutApi
        .getTurnoutStates(turnouts)
        .then((result) => {
          if (!cancelled) setTurnoutStates(result.states)
        })
        .catch((err) => {
          if (!cancelled) onError(`Could not read the turnouts: ${err.message}`)
        })
    read()
    const timer = setInterval(read, 2000)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [active, addressKey, onError])

  const throwPlaced = useCallback(
    async (placed) => {
      const address = placed.dcc_address
      const target = turnoutTarget(placed, turnoutStates[String(address)] ?? null)
      onError('')
      try {
        await layoutApi.throwTurnout(address, target)
        setTurnoutStates((prev) => ({ ...prev, [String(address)]: target }))
      } catch (err) {
        onError(`Could not switch turnout ${address}: ${err.message}`)
      }
    },
    [turnoutStates, onError]
  )

  return { turnoutStates, throwPlaced }
}
