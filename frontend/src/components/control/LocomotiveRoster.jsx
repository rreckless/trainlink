import { useEffect, useState } from 'react'
import { locomotiveApi } from '../../services/locomotiveApi'

const LOCOMOTIVE_TYPES = ['diesel', 'steam', 'electric', 'other']
// Moving time in hours, minutes, and seconds
function formatDuration(seconds) {
  const total = Math.floor(seconds ?? 0)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, '0')}m`
  if (minutes > 0) return `${minutes}m ${String(secs).padStart(2, '0')}s`
  return `${secs}s`
}

// F0 is the light. The others are the functions the decoder maps
const FUNCTION_BUTTONS = [
  { index: 0, label: 'Light' },
  { index: 1, label: 'F1' },
  { index: 2, label: 'F2' },
  { index: 3, label: 'F3' },
  { index: 4, label: 'F4' },
]
const EMPTY_FORM = { name: '', dcc_address: '', locomotive_type: 'diesel', length: '' }

// Locomotives on the server. Tick several to send one speed or direction to all of them
export default function LocomotiveRoster({ controlsEnabled, onError }) {
  const [locomotives, setLocomotives] = useState([])
  const [selected, setSelected] = useState(new Set())
  // What was last sent to each locomotive this session: { id: { speed, direction } }
  const [commanded, setCommanded] = useState({})
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [form, setForm] = useState(EMPTY_FORM)
  const [adding, setAdding] = useState(false)
  // What the Z21 last reported for each locomotive, by address. Read every two seconds
  const [reported, setReported] = useState({})

  const refresh = () =>
    locomotiveApi
      .list()
      .then(setLocomotives)
      .catch((err) => onError(`Could not load locomotives: ${err.message}`))

  useEffect(() => {
    refresh()
    // refresh only once, on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

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
  }, [addressKey])

  const toggle = (id) =>
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  const allSelected = locomotives.length > 0 && selected.size === locomotives.length
  const toggleAll = () => setSelected(allSelected ? new Set() : new Set(locomotives.map((loc) => loc.id)))

  // Send a speed and direction to every selected locomotive at once
  const sendToSelected = async (speed, direction) => {
    const targets = locomotives.filter((loc) => selected.has(loc.id))
    if (targets.length === 0) return onError('Select one or more locomotives first')
    setBusy(true)
    onError('')
    setMessage('')
    const forward = direction !== 'reverse'
    const results = await Promise.allSettled(
      targets.map((loc) => locomotiveApi.setSpeed(loc.dcc_address, speed, forward))
    )
    const failed = []
    const sent = {}
    results.forEach((result, index) => {
      const loc = targets[index]
      if (result.status === 'fulfilled') sent[loc.id] = { speed, direction }
      else failed.push(`${loc.name}: ${result.reason.message}`)
    })
    setCommanded((prev) => ({ ...prev, ...sent }))
    if (failed.length > 0) onError(failed.join('. '))
    else setMessage(`${targets.length} locomotive${targets.length === 1 ? '' : 's'} ${direction === 'stop' ? 'stopped' : `at speed ${speed}, ${direction}`}`)
    setBusy(false)
  }

  // The speed the selected locomotives were last sent, for the group controls
  const selectedStates = locomotives.filter((loc) => selected.has(loc.id)).map((loc) => commanded[loc.id])
  const groupSpeed = selectedStates.length > 0 ? selectedStates[0]?.speed ?? 0 : 0
  const groupDirection = selectedStates.length > 0 ? selectedStates[0]?.direction ?? 'stop' : 'stop'

  // Switch a function on every selected locomotive. If they all have it on, switch it off
  const sendFunction = async (index) => {
    const targets = locomotives.filter((loc) => selected.has(loc.id))
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
    else setMessage(`${FUNCTION_BUTTONS.find((f) => f.index === index).label} ${on ? 'on' : 'off'} for ${targets.length} locomotive${targets.length === 1 ? '' : 's'}`)
    setBusy(false)
  }

  const emergencyStop = async () => {
    setBusy(true)
    onError('')
    setMessage('')
    try {
      await locomotiveApi.emergencyStop()
      setCommanded((prev) => Object.fromEntries(Object.keys(prev).map((id) => [id, { speed: 0, direction: 'stop' }])))
      setMessage('Emergency stop sent. All locomotives stopped')
    } catch (err) {
      onError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const addLocomotive = async (event) => {
    event.preventDefault()
    onError('')
    const body = {
      name: form.name.trim(),
      dcc_address: Number(form.dcc_address),
      locomotive_type: form.locomotive_type,
    }
    if (form.length !== '') body.length = Number(form.length)
    try {
      await locomotiveApi.create(body)
      setForm(EMPTY_FORM)
      setAdding(false)
      await refresh()
    } catch (err) {
      onError(err.message)
    }
  }

  const removeLocomotive = async (loc) => {
    if (!window.confirm(`Remove ${loc.name} from the roster?`)) return
    onError('')
    try {
      await locomotiveApi.remove(loc.id)
      setSelected((prev) => {
        const next = new Set(prev)
        next.delete(loc.id)
        return next
      })
      await refresh()
    } catch (err) {
      onError(err.message)
    }
  }

  const disabled = busy || !controlsEnabled

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">Locomotives</h3>
        <button
          onClick={() => setAdding(!adding)}
          className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600"
        >
          {adding ? 'Cancel' : 'Add locomotive'}
        </button>
      </div>

      {adding && (
        <form onSubmit={addLocomotive} className="grid grid-cols-2 gap-3 items-end">
          <label className="text-sm text-slate-300">
            Name
            <input
              required
              value={form.name}
              onChange={(event) => setForm({ ...form, name: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="text-sm text-slate-300">
            DCC address (1-9999)
            <input
              type="number"
              min="1"
              max="9999"
              required
              value={form.dcc_address}
              onChange={(event) => setForm({ ...form, dcc_address: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="text-sm text-slate-300">
            Type
            <select
              value={form.locomotive_type}
              onChange={(event) => setForm({ ...form, locomotive_type: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            >
              {LOCOMOTIVE_TYPES.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </label>
          <label className="text-sm text-slate-300">
            Length (cm, optional)
            <input
              type="number"
              min="0"
              step="0.1"
              value={form.length}
              onChange={(event) => setForm({ ...form, length: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <button type="submit" className="col-span-2 px-4 py-2 bg-blue-600 rounded hover:bg-blue-700">
            Add
          </button>
        </form>
      )}

      {locomotives.length === 0 ? (
        <p className="text-sm text-slate-400">No locomotives yet. Add one to start controlling trains.</p>
      ) : (
        <>
          <div className="space-y-2 bg-slate-700 rounded p-3">
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={allSelected} onChange={toggleAll} disabled={busy} />
              Select all ({selected.size} selected)
            </label>
            <div className="space-y-1">
              <label className="block text-sm text-slate-300">
                Speed: {groupSpeed} {groupDirection !== 'stop' && `(${groupDirection})`}
              </label>
              <input
                type="range"
                min="0"
                max="126"
                value={groupSpeed}
                onChange={(event) => sendToSelected(Number(event.target.value), groupDirection === 'stop' ? 'forward' : groupDirection)}
                disabled={disabled || selected.size === 0 || groupDirection === 'stop'}
                className="w-full"
              />
            </div>
            <div className="grid grid-cols-3 gap-2">
              {['reverse', 'stop', 'forward'].map((direction) => (
                <button
                  key={direction}
                  onClick={() => sendToSelected(direction === 'stop' ? 0 : groupSpeed, direction)}
                  disabled={disabled || selected.size === 0}
                  className={`py-2 rounded font-semibold ${
                    groupDirection === direction ? 'bg-blue-600 text-white' : 'bg-slate-600 hover:bg-slate-500'
                  } disabled:opacity-50`}
                >
                  {direction.toUpperCase()}
                </button>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-5 gap-2">
            {FUNCTION_BUTTONS.map((fn) => (
              <button
                key={fn.index}
                onClick={() => sendFunction(fn.index)}
                disabled={disabled || selected.size === 0}
                className="py-2 rounded text-sm font-semibold bg-slate-600 hover:bg-slate-500 disabled:opacity-50"
              >
                {fn.label}
              </button>
            ))}
          </div>

          <div className="space-y-2">
            {locomotives.map((loc) => {
              const state = commanded[loc.id]
              const live = reported[loc.dcc_address]
              return (
                <div
                  key={loc.id}
                  className={`flex items-center gap-3 rounded p-3 ${
                    selected.has(loc.id) ? 'bg-slate-600 border border-blue-500' : 'bg-slate-700 border border-slate-600'
                  }`}
                >
                  <input type="checkbox" checked={selected.has(loc.id)} onChange={() => toggle(loc.id)} disabled={busy} />
                  <div className="flex-1">
                    <p className="font-semibold">{loc.name}</p>
                    <p className="text-xs text-slate-400">
                      Address {loc.dcc_address} · {loc.locomotive_type}
                      {live && live.speed !== null
                        ? ` · Z21: speed ${live.speed} ${live.direction}`
                        : state
                        ? ` · sent: speed ${state.speed} ${state.direction}`
                        : ' · no state yet'}
                      {live?.functions?.length ? ` · functions ${live.functions.map((f) => `F${f}`).join(' ')}` : ''}
                      {` · moved ${formatDuration(loc.travelled_time)}`}
                    </p>
                  </div>
                  <button
                    onClick={() => removeLocomotive(loc)}
                    disabled={busy}
                    className="px-2 py-1 text-xs bg-slate-800 rounded hover:bg-red-800 disabled:opacity-50"
                  >
                    Remove
                  </button>
                </div>
              )
            })}
          </div>
        </>
      )}

      <button
        onClick={emergencyStop}
        disabled={busy || !controlsEnabled}
        className="w-full py-3 bg-red-700 hover:bg-red-800 text-white font-bold rounded-lg disabled:opacity-50 border-2 border-red-600"
      >
        EMERGENCY STOP
      </button>

      {message && <p className="text-sm text-green-300">{message}</p>}
    </div>
  )
}
