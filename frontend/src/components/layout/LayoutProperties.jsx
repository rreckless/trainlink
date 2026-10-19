import { useState } from 'react'
import { ENTITY_KINDS } from '../../layout/model'

const DCC_MAX = 2048

// DCC address and straight output of a placed turnout. Each change is saved to the layout on Save
function DccFields({ placed, busy, onChange }) {
  const [address, setAddress] = useState(placed.dcc_address ?? '')
  const addressValid = address === '' || (Number.isInteger(Number(address)) && Number(address) >= 1 && Number(address) <= DCC_MAX)

  const changeAddress = (value) => {
    setAddress(value)
    if (value === '') return onChange({ dcc_address: undefined })
    const number = Number(value)
    if (Number.isInteger(number) && number >= 1 && number <= DCC_MAX) onChange({ dcc_address: number })
  }

  return (
    <div className="space-y-2">
      <label className="block">
        <span className="text-slate-300">DCC address (1-{DCC_MAX})</span>
        <input
          type="number"
          min="1"
          max={DCC_MAX}
          value={address}
          placeholder="None"
          onChange={(event) => changeAddress(event.target.value)}
          disabled={busy}
          className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        />
      </label>
      {!addressValid && <p className="text-xs text-red-300">The address must be a whole number from 1 to {DCC_MAX}</p>}
      <label className="block">
        <span className="text-slate-300">Straight route is</span>
        <select
          value={placed.dcc_output_straight ?? 0}
          onChange={(event) => onChange({ dcc_output_straight: Number(event.target.value) })}
          disabled={busy}
          className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        >
          <option value={0}>Output 1 (P=0)</option>
          <option value={1}>Output 2 (P=1)</option>
        </select>
      </label>
    </div>
  )
}

const KIND_NOTES = {
  block: 'Occupancy will come from linked feedbacks. The state can be set by hand for now.',
  signal: 'Aspect changes automatically from block state once signal control is built.',
  feedback: 'Set by the Z21 once the feedback is linked to hardware.',
}

export default function LayoutProperties({ item, track, placed, piece, busy, onRename, onSetDcc, onDelete }) {
  if (placed && piece) {
    return (
      <div className="space-y-3 text-sm">
        <p className="font-semibold">
          {piece.part_number} {piece.name}
        </p>
        <p className="text-slate-400">{piece.series} ({piece.system})</p>
        <p className="text-slate-400">
          Position: ({Math.round(placed.x)}, {Math.round(placed.y)}), rotation {Math.round(placed.rotation)}°
          {placed.flip ? ', mirrored' : ''}
        </p>
        {!piece.verified && (
          <p className="text-xs text-amber-300">Dimensions not yet checked against the catalogue</p>
        )}
        {piece.type === 'turnout' && onSetDcc && (
          <DccFields key={placed.id} placed={placed} busy={busy} onChange={onSetDcc} />
        )}
        <button
          onClick={onDelete}
          disabled={busy}
          className="w-full px-3 py-2 bg-red-700 rounded hover:bg-red-800 disabled:opacity-50"
        >
          Delete track piece
        </button>
      </div>
    )
  }

  if (track) {
    const length = Math.hypot(track.x2 - track.x1, track.y2 - track.y1)
    return (
      <div className="space-y-3 text-sm">
        <p className="font-semibold">Track segment</p>
        <p className="text-slate-400">
          From ({track.x1}, {track.y1}) to ({track.x2}, {track.y2})
        </p>
        <p className="text-slate-400">Length: {Math.round(length)}</p>
        <button
          onClick={onDelete}
          disabled={busy}
          className="w-full px-3 py-2 bg-red-700 rounded hover:bg-red-800 disabled:opacity-50"
        >
          Delete track
        </button>
      </div>
    )
  }

  if (!item) {
    return <p className="text-slate-400 text-sm">Select an element to edit</p>
  }

  return (
    <div className="space-y-3 text-sm">
      <p className="font-semibold">{ENTITY_KINDS[item.kind].label}</p>

      <label className="block">
        <span className="text-slate-300">Name</span>
        <input
          value={item.name}
          onChange={(event) => onRename(event.target.value)}
          disabled={busy}
          className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        />
      </label>

      <p className="text-slate-400">
        Position: ({item.x}, {item.y})
      </p>

      {item.kind === 'block' && <p className="text-slate-400">State: {item.state}</p>}
      {item.kind === 'signal' && (
        <p className="text-slate-400">
          Type: {item.signal_type}, aspect: {item.aspect}
        </p>
      )}
      {item.kind === 'feedback' && (
        <p className="text-slate-400">
          Number: {item.feedback_number}, state: {item.state ? 'occupied' : 'clear'}
        </p>
      )}

      <p className="text-xs text-slate-500">{KIND_NOTES[item.kind]}</p>

      <button
        onClick={onDelete}
        disabled={busy}
        className="w-full px-3 py-2 bg-red-700 rounded hover:bg-red-800 disabled:opacity-50"
      >
        Delete {ENTITY_KINDS[item.kind].label.toLowerCase()}
      </button>
    </div>
  )
}
