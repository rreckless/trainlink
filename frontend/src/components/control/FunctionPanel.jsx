import { useRef } from 'react'
import { FUNCTION_COUNT, functionLabel } from '../../hooks/useLocomotiveRoster'
import { pictureUrl } from '../../services/locomotiveApi'

// Function buttons for the selected locomotives. Each button shows what that locomotive's function does
export default function FunctionPanel({ roster, controlsEnabled }) {
  const { selectedLocomotives, reported, busy, sendFunction, holdFunction, labelFor, isMomentary } = roster
  // Momentary buttons that are held down, so a release is only sent after a press
  const held = useRef(new Set())
  const hasOn = (index) =>
    selectedLocomotives.length > 0 &&
    selectedLocomotives.every((loc) => reported[loc.dcc_address]?.functions?.includes(index))
  const disabled = busy || !controlsEnabled || selectedLocomotives.length === 0
  const single = selectedLocomotives.length === 1 ? selectedLocomotives[0] : null

  const press = (index) => {
    if (!isMomentary(index)) return sendFunction(index)
    held.current.add(index)
    holdFunction(index, true)
  }
  const release = (index) => {
    if (!held.current.has(index)) return
    held.current.delete(index)
    holdFunction(index, false)
  }

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-4">
      <div className="flex items-center gap-3">
        {single && pictureUrl(single) && (
          <img src={pictureUrl(single)} alt={single.name} className="w-16 h-12 object-cover rounded bg-slate-700" />
        )}
        <div className="flex-1">
          <h3 className="text-lg font-semibold">{single ? single.name : 'Functions'}</h3>
          <span className="text-sm text-slate-400">
            {selectedLocomotives.length === 0
              ? 'Select locomotives in the roster'
              : single
              ? `Address ${single.dcc_address}`
              : `For ${selectedLocomotives.length} selected`}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-2">
        {Array.from({ length: FUNCTION_COUNT }, (_, index) => (
          <button
            key={index}
            onPointerDown={isMomentary(index) ? () => press(index) : undefined}
            onPointerUp={isMomentary(index) ? () => release(index) : undefined}
            onPointerLeave={isMomentary(index) ? () => release(index) : undefined}
            onClick={isMomentary(index) ? undefined : () => press(index)}
            disabled={disabled}
            title={isMomentary(index) ? 'Hold to use' : 'Press to switch on or off'}
            className={`py-3 rounded text-sm font-semibold transition-colors ${
              hasOn(index) ? 'bg-amber-500 text-slate-900' : 'bg-slate-600 hover:bg-slate-500'
            } disabled:opacity-50`}
          >
            {selectedLocomotives.length === 0 ? functionLabel(index) : labelFor(index)}
          </button>
        ))}
      </div>

      <p className="text-xs text-slate-500">
        Toggle buttons switch on or off. Momentary buttons (for example a horn) are on while held.
      </p>
    </div>
  )
}
