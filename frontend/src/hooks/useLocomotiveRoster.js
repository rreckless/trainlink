import { useCallback, useEffect, useMemo, useState } from 'react'
import { locomotiveApi } from '../services/locomotiveApi'

// The Control page panels share one roster: the locomotives, the selection, and what the Z21 reports
export const FUNCTION_COUNT = 20

// F0 is the light. The rest are shown by number
export const functionLabel = (index) => (index === 0 ? 'Light' : `F${index}`)

export function useLocomotiveRoster(onError) {
  const [locomotives, setLocomotives] = useState([])
  const [selected, setSelected] = useState(new Set())
  // What the Z21 last reported for each locomotive, by address. Read every two seconds
  const [reported, setReported] = useState({})
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')

  const refresh = useCallback(
    () =>
      locomotiveApi
        .list()
        .then(setLocomotives)
        .catch((err) => onError(`Could not load locomotives: ${err.message}`)),
    [onError]
  )

  useEffect(() => {
    refresh()
  }, [refresh])

  const addressKey = locomotives.map((loc) => loc.dcc_address).join(',')
  useEffect(() => {
    if (addressKey === '') return undefined
    let cancelled = false
    // Errors are not shown here: the Z21 may be offline, and this would repeat every two seconds
    const read = () => {
      // The roster list carries each locomotive's moving time, so it is refreshed on the same tick
      refresh()
      return locomotiveApi
        .getStates(addressKey.split(',').map(Number))
        .then((result) => {
          if (!cancelled) setReported(result.states)
        })
        .catch(() => {})
    }
    read()
    const timer = setInterval(read, 2000)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [addressKey, refresh])

  const toggle = useCallback(
    (id) =>
      setSelected((prev) => {
        const next = new Set(prev)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        return next
      }),
    []
  )

  const allSelected = locomotives.length > 0 && selected.size === locomotives.length
  const toggleAll = () => setSelected(allSelected ? new Set() : new Set(locomotives.map((loc) => loc.id)))
  const selectedLocomotives = useMemo(() => locomotives.filter((loc) => selected.has(loc.id)), [locomotives, selected])

  // Switch a function on every selected locomotive. If they all have it on, switch it off
  const sendFunction = useCallback(
    async (index) => {
      const targets = selectedLocomotives
      if (targets.length === 0) return onError('Select one or more locomotives first')
      const hasIt = (loc) => reported[loc.dcc_address]?.functions?.includes(index) ?? false
      const on = !targets.every(hasIt)
      setBusy(true)
      onError('')
      setMessage('')
      const results = await Promise.allSettled(targets.map((loc) => locomotiveApi.setFunction(loc.dcc_address, index, on)))
      const failed = []
      setReported((prev) => {
        const next = { ...prev }
        targets.forEach((loc, i) => {
          if (results[i].status !== 'fulfilled') return
          const current = next[loc.dcc_address] ?? { speed: null, direction: 'stop', functions: [] }
          const functions = on
            ? [...new Set([...current.functions, index])].sort((a, b) => a - b)
            : current.functions.filter((f) => f !== index)
          next[loc.dcc_address] = { ...current, functions }
        })
        return next
      })
      results.forEach((result, i) => {
        if (result.status === 'rejected') failed.push(`${targets[i].name}: ${result.reason.message}`)
      })
      if (failed.length > 0) onError(failed.join('. '))
      else setMessage(`${functionLabel(index)} ${on ? 'on' : 'off'} for ${targets.length} locomotive${targets.length === 1 ? '' : 's'}`)
      setBusy(false)
    },
    [selectedLocomotives, reported, onError]
  )

  return {
    locomotives,
    refresh,
    selected,
    setSelected,
    toggle,
    toggleAll,
    allSelected,
    selectedLocomotives,
    reported,
    busy,
    setBusy,
    message,
    setMessage,
    sendFunction,
  }
}
