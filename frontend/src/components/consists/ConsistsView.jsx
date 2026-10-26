import { useCallback, useEffect, useState } from 'react'
import { useStore } from '../../store'
import { confirmAction } from '../../services/dialogs'
import { consistApi } from '../../services/consistApi'
import { locomotiveApi } from '../../services/locomotiveApi'
import { refreshZ21Status } from '../../hooks/useZ21Status'

const FUNCTION_MAX = 31
const MEMBERS_MIN = 2

// Consists: make a consist from locomotives, give it a consist address, and drive it as one group
export default function ConsistsView() {
  const z21 = useStore((state) => state.z21)
  const controlsEnabled = z21.state === 'ready'
  const [locomotives, setLocomotives] = useState([])
  const [consists, setConsists] = useState([])
  // The locomotives ticked for the consist being made, in the order they were ticked. The first leads
  const [picked, setPicked] = useState([])
  // Members of the consist being made that run reversed, by locomotive id
  const [reversed, setReversed] = useState(new Set())
  const [name, setName] = useState('')
  // The DCC consist address the new consist is given. The server refuses one another consist already uses
  const [address, setAddress] = useState(1)
  const [functionIndex, setFunctionIndex] = useState(0)
  // What was last sent to each consist this session: { id: { speed, direction } }
  const [commanded, setCommanded] = useState({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  const refresh = useCallback(async () => {
    try {
      const [locos, groups] = await Promise.all([locomotiveApi.list(), consistApi.list()])
      setLocomotives(locos)
      setConsists(groups)
    } catch (err) {
      setError(`Could not load consists: ${err.message}`)
    }
  }, [])

  useEffect(() => {
    refresh()
  }, [refresh])

  const isPicked = (id) => picked.includes(id)
  const togglePicked = (id) =>
    setPicked((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]))
  const toggleReversed = (id) =>
    setReversed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  // Run one request, showing its error and refreshing the list afterwards
  const run = async (action, success) => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const result = await action()
      if (success) setMessage(success(result))
      await refresh()
      return result
    } catch (err) {
      setError(err.message)
      return null
    } finally {
      setBusy(false)
    }
  }

  const createConsist = async () => {
    const members = picked.map((id) => ({ locomotive_id: id, reversed: reversed.has(id) }))
    const created = await run(() => consistApi.create({ name, address, members }), (result) => `Created ${result.name}`)
    if (created) {
      setName('')
      setAddress(1)
      setPicked([])
      setReversed(new Set())
    }
  }

  const changeAddress = (consist, value) =>
    run(() => consistApi.update(consist.id, { address: value }), () => `${consist.name} is now address ${value}`)

  const toggleMemberReversed = (consist, memberId) => {
    const members = consist.members.map((member) => ({
      locomotive_id: member.locomotive_id,
      reversed: member.locomotive_id === memberId ? !member.reversed : member.reversed,
    }))
    return run(() => consistApi.update(consist.id, { members }))
  }

  const removeConsist = async (consist) => {
    if (!(await confirmAction(`Delete the consist ${consist.name}? Its locomotives stay in the roster.`, { okLabel: 'Delete', danger: true }))) return
    run(() => consistApi.remove(consist.id), () => `Deleted ${consist.name}`)
  }

  // Send a speed and direction to every member of one consist
  const sendSpeed = async (consist, speed, direction) => {
    const forward = direction !== 'reverse'
    const sent = await run(() => consistApi.setSpeed(consist.id, speed, forward))
    if (sent) setCommanded((prev) => ({ ...prev, [consist.id]: { speed, direction } }))
  }

  const sendFunction = (consist, on) =>
    run(
      () => consistApi.setFunction(consist.id, functionIndex, on),
      () => `F${functionIndex} ${on ? 'on' : 'off'} for ${consist.name}`
    )

  const pickedLocomotives = picked.map((id) => locomotives.find((loc) => loc.id === id)).filter(Boolean)
  const disabled = busy || !controlsEnabled

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">New consist</h3>
          {!controlsEnabled && <span className="text-xs text-amber-300">Connect the Z21 to drive a consist</span>}
        </div>
        <p className="text-xs text-slate-400">
          A consist needs at least two locomotives. TrainLink drives each one in turn, so the decoders do not follow the
          consist address yet. Their CV19 (multiple-unit address) is not set by TrainLink.
        </p>

        {locomotives.length === 0 ? (
          <p className="text-sm text-slate-400">No locomotives yet. Add them in the Locomotives section.</p>
        ) : (
          <ul className="space-y-1">
            {locomotives.map((loc) => {
              const index = picked.indexOf(loc.id)
              return (
                <li key={loc.id} className="flex items-center gap-2 text-sm">
                  <label className="flex flex-1 items-center gap-2">
                    <input type="checkbox" checked={isPicked(loc.id)} onChange={() => togglePicked(loc.id)} disabled={busy} />
                    <span>{loc.name}</span>
                    <span className="text-xs text-slate-400">address {loc.dcc_address}</span>
                    {index === 0 && <span className="text-xs text-amber-300">lead</span>}
                  </label>
                  {isPicked(loc.id) && (
                    <label className="flex items-center gap-1 text-xs text-slate-300">
                      <input type="checkbox" checked={reversed.has(loc.id)} onChange={() => toggleReversed(loc.id)} disabled={busy} />
                      Reversed
                    </label>
                  )}
                </li>
              )
            })}
          </ul>
        )}

        <div className="flex flex-wrap gap-2">
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Consist name"
            maxLength={40}
            disabled={busy}
            className="flex-1 min-w-40 px-3 py-1 bg-slate-700 rounded border border-slate-600 text-white"
          />
          <label className="flex items-center gap-1 text-sm text-slate-300">
            Address
            <input
              type="number"
              min="1"
              max="127"
              value={address}
              onChange={(event) => setAddress(Number(event.target.value))}
              disabled={busy}
              aria-label="Consist address"
              className="w-20 px-2 py-1 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <button
            onClick={createConsist}
            disabled={busy || name.trim() === '' || pickedLocomotives.length < MEMBERS_MIN}
            className="px-4 py-1 bg-green-600 rounded font-semibold hover:bg-green-700 disabled:opacity-50"
          >
            Create consist
          </button>
        </div>
        {pickedLocomotives.length > 0 && pickedLocomotives.length < MEMBERS_MIN && (
          <p className="text-xs text-slate-400">Pick one more locomotive</p>
        )}
      </div>

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}
      {message && <p className="text-sm text-green-300">{message}</p>}

      <div className="bg-slate-800 rounded-lg p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Consists</h3>
          <label className="flex items-center gap-2 text-sm text-slate-300">
            Function
            <input
              type="number"
              min="0"
              max={FUNCTION_MAX}
              value={functionIndex}
              onChange={(event) => setFunctionIndex(Math.min(FUNCTION_MAX, Math.max(0, Number(event.target.value) || 0)))}
              disabled={disabled}
              className="w-20 px-2 py-1 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
        </div>

        {consists.length === 0 ? (
          <p className="text-sm text-slate-400">No consists yet.</p>
        ) : (
          consists.map((consist) => {
            const state = commanded[consist.id] ?? { speed: 0, direction: 'stop' }
            return (
              <div key={consist.id} className="bg-slate-700 rounded p-3 space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-semibold flex-1">{consist.name}</p>
                  <label className="flex items-center gap-1 text-xs text-slate-300">
                    Address
                    <input
                      key={consist.address}
                      type="number"
                      min="1"
                      max="127"
                      defaultValue={consist.address}
                      onBlur={(event) => {
                        const value = Number(event.target.value)
                        if (value !== consist.address) changeAddress(consist, value)
                      }}
                      disabled={busy}
                      aria-label={`Consist address for ${consist.name}`}
                      className="w-16 px-2 py-1 bg-slate-800 rounded border border-slate-600 text-white"
                    />
                  </label>
                  <button
                    onClick={() => removeConsist(consist)}
                    disabled={busy}
                    className="px-2 py-1 text-xs bg-slate-800 rounded hover:bg-red-800 disabled:opacity-50"
                  >
                    Delete
                  </button>
                </div>

                <ul className="space-y-1">
                  {consist.members.map((member, index) => (
                    <li key={member.locomotive_id} className="flex items-center gap-2 text-sm">
                      <span className="flex-1">
                        {member.name}
                        <span className="text-xs text-slate-400"> · address {member.dcc_address}</span>
                        {index === 0 && <span className="ml-2 text-xs text-amber-300">lead</span>}
                      </span>
                      <label className="flex items-center gap-1 text-xs text-slate-300">
                        <input
                          type="checkbox"
                          checked={member.reversed}
                          onChange={() => toggleMemberReversed(consist, member.locomotive_id)}
                          disabled={busy}
                        />
                        Reversed
                      </label>
                    </li>
                  ))}
                </ul>

                <div className="space-y-1">
                  <label className="block text-sm text-slate-300">
                    Speed: {state.speed} {state.direction !== 'stop' && `(${state.direction})`}
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="126"
                    value={state.speed}
                    onChange={(event) =>
                      sendSpeed(consist, Number(event.target.value), state.direction === 'stop' ? 'forward' : state.direction)
                    }
                    disabled={disabled || state.direction === 'stop'}
                    className="w-full"
                  />
                </div>
                <div className="grid grid-cols-3 gap-2">
                  {['reverse', 'stop', 'forward'].map((direction) => (
                    <button
                      key={direction}
                      onClick={() => sendSpeed(consist, direction === 'stop' ? 0 : state.speed, direction)}
                      disabled={disabled}
                      className={`py-2 rounded font-semibold ${
                        state.direction === direction ? 'bg-blue-600 text-white' : 'bg-slate-600 hover:bg-slate-500'
                      } disabled:opacity-50`}
                    >
                      {direction.toUpperCase()}
                    </button>
                  ))}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  {[true, false].map((on) => (
                    <button
                      key={String(on)}
                      onClick={() => sendFunction(consist, on)}
                      disabled={disabled}
                      className="py-2 rounded bg-slate-600 hover:bg-slate-500 font-semibold disabled:opacity-50"
                    >
                      F{functionIndex} {on ? 'ON' : 'OFF'}
                    </button>
                  ))}
                </div>
              </div>
            )
          })
        )}
      </div>

      {!controlsEnabled && consists.length > 0 && (
        <button onClick={refreshZ21Status} className="text-xs text-slate-400 underline">
          Check the Z21 connection
        </button>
      )}
    </div>
  )
}
