import { useState } from 'react'
import { confirmAction } from '../../services/dialogs'
import { locomotiveApi, pictureUrl } from '../../services/locomotiveApi'


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

// Locomotives on the server. Tick several to send one speed or direction to all of them
export default function LocomotiveRoster({ roster, controlsEnabled, onError }) {
  const { locomotives, refresh, selected, setSelected, toggle, toggleAll, allSelected, selectedLocomotives, reported, busy, setBusy, message, setMessage } = roster
  // What was last sent to each locomotive this session: { id: { speed, direction } }
  const [commanded, setCommanded] = useState({})

  // Send a speed and direction to every selected locomotive at once
  const sendToSelected = async (speed, direction) => {
    const targets = selectedLocomotives
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
  const selectedStates = selectedLocomotives.map((loc) => commanded[loc.id])
  const groupSpeed = selectedStates.length > 0 ? selectedStates[0]?.speed ?? 0 : 0
  const groupDirection = selectedStates.length > 0 ? selectedStates[0]?.direction ?? 'stop' : 'stop'

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

  const removeLocomotive = async (loc) => {
    if (!(await confirmAction(`Remove ${loc.name} from the roster?`, { okLabel: 'Remove', danger: true }))) return
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
      </div>

      {locomotives.length === 0 ? (
        <p className="text-sm text-slate-400">No locomotives yet. Add them in the Locomotives section.</p>
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
                  {pictureUrl(loc) && <img src={pictureUrl(loc)} alt="" className="w-12 h-9 object-cover rounded bg-slate-800" />}
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
