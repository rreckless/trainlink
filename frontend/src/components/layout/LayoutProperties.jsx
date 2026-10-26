import { useState } from 'react'
import { ENTITY_KINDS } from '../../layout/model'
import { MAX_CURVE_DEGREES, arcCurve, arcLength } from '../../layout/curves'

const DCC_MAX = 2048

// Length of a flex track. Type freely: the value is applied once it is between 10 mm and the full length
function LengthField({ placed, maxLength, busy, onChange }) {
  const current = String(placed.length ?? maxLength)
  const [draft, setDraft] = useState(current)
  const value = Number(draft)
  const valid = draft.trim() !== '' && Number.isFinite(value) && value >= 10 && value <= maxLength

  const change = (text) => {
    setDraft(text)
    const number = Number(text)
    if (text.trim() !== '' && Number.isFinite(number) && number >= 10 && number <= maxLength) onChange(number)
  }

  return (
    <label className="block">
      <span className="text-slate-300">Length (mm, 10 to {maxLength})</span>
      <input
        type="number"
        step="0.1"
        value={draft}
        onChange={(event) => change(event.target.value)}
        onBlur={() => !valid && setDraft(current)}
        disabled={busy}
        className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
      />
      <span className="text-xs text-slate-500">Drag an open end to stretch the track and join it to another piece</span>
    </label>
  )
}

// A curve from a radius and an angle. The panel shows the length the ends will be apart before anything is changed
function CurveFields({ maxLength, busy, onApply }) {
  const [radius, setRadius] = useState('300')
  const [angle, setAngle] = useState('90')
  const [side, setSide] = useState('left')
  const radiusValue = Number(radius)
  const angleValue = Number(angle)
  const valid = Number.isFinite(radiusValue) && radiusValue > 0 && Number.isFinite(angleValue) && angleValue > 0 && angleValue <= MAX_CURVE_DEGREES
  let preview = null
  if (valid) preview = arcCurve(radiusValue, angleValue, side)
  const fits = preview !== null && preview.length >= 10 && preview.length <= maxLength
  return (
    <div className="space-y-2">
      <p className="text-slate-300">Curve from radius and angle</p>
      <div className="grid grid-cols-2 gap-2">
        <label className="block text-xs text-slate-400">
          Radius (mm)
          <input
            type="number"
            min="1"
            step="1"
            value={radius}
            onChange={(event) => setRadius(event.target.value)}
            disabled={busy}
            className="mt-1 w-full px-2 py-1.5 bg-slate-700 rounded border border-slate-600 text-white"
          />
        </label>
        <label className="block text-xs text-slate-400">
          Angle (1 to {MAX_CURVE_DEGREES}°)
          <input
            type="number"
            min="1"
            max={MAX_CURVE_DEGREES}
            step="1"
            value={angle}
            onChange={(event) => setAngle(event.target.value)}
            disabled={busy}
            className="mt-1 w-full px-2 py-1.5 bg-slate-700 rounded border border-slate-600 text-white"
          />
        </label>
      </div>
      <select
        value={side}
        onChange={(event) => setSide(event.target.value)}
        disabled={busy}
        className="w-full px-2 py-1.5 bg-slate-700 rounded border border-slate-600 text-white"
      >
        <option value="left">Bends left</option>
        <option value="right">Bends right</option>
      </select>
      {!valid && <p className="text-xs text-red-300">The radius must be above 0, and the angle from 1 to {MAX_CURVE_DEGREES}°</p>}
      {valid && (
        <p className={`text-xs ${fits ? 'text-slate-400' : 'text-amber-300'}`}>
          {fits
            ? `Ends ${preview.length} mm apart, along an arc of ${Math.round(arcLength(radiusValue, angleValue))} mm`
            : `The ends would be ${preview.length} mm apart. They must be 10 to ${maxLength} mm`}
        </p>
      )}
      <p className="text-xs text-slate-500">A bend over {MAX_CURVE_DEGREES}° is two curves, one after the other.</p>
      <button
        onClick={() => onApply({ radius: radiusValue, angle: angleValue, side })}
        disabled={busy || !valid || !fits}
        className="w-full px-3 py-2 bg-blue-600 rounded hover:bg-blue-500 disabled:opacity-50"
      >
        Apply curve
      </button>
    </div>
  )
}

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

export default function LayoutProperties({ item, track, placed, piece, busy, onRename, onSetDcc, onMakeCurved, onStraighten, onSetLength, onSmooth, onArc, onRotate, onDelete }) {
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
        {onRotate && (
          <div className="space-y-2">
            <p className="text-slate-300">Turn</p>
            <div className="grid grid-cols-4 gap-2">
              {[-90, -15, 15, 90].map((degrees) => (
                <button
                  key={degrees}
                  onClick={() => onRotate(degrees)}
                  disabled={busy}
                  className="py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50"
                >
                  {degrees > 0 ? '+' : ''}{degrees}°
                </button>
              ))}
            </div>
            <p className="text-xs text-slate-500">Turns about its centre. Press R or Shift+R to turn 15°. A piece turned onto an open end joins it.</p>
          </div>
        )}
        {piece.type === 'flex' && onSetLength && (
          <LengthField key={placed.id} placed={placed} maxLength={piece.length} busy={busy} onChange={onSetLength} />
        )}
        {piece.type === 'flex' && onArc && (
          <CurveFields key={`arc-${placed.id}`} maxLength={piece.length} busy={busy} onApply={onArc} />
        )}
        {piece.type === 'flex' && onMakeCurved && (
          <div className="space-y-2">
            <p className="text-xs text-slate-400">
              Drag the two blue handles to shape the curve. The track passes through its two ends.
            </p>
            {Array.isArray(placed.handles) ? (
              <div className="space-y-2">
                <button onClick={onSmooth} disabled={busy} className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50">
                  Smooth curve
                </button>
                <p className="text-xs text-slate-500">Finds the fairest curve. The angles where it joins other track stay the same.</p>
                <button onClick={onStraighten} disabled={busy} className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50">
                  Straighten track
                </button>
              </div>
            ) : (
              <button onClick={onMakeCurved} disabled={busy} className="w-full px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50">
                Make curved
              </button>
            )}
          </div>
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
