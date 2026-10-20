import { FUNCTION_COUNT, functionLabel } from '../../hooks/useLocomotiveRoster'

// Functions of the selected locomotives. A button is lit when every selected locomotive reports it on
export default function FunctionPanel({ roster, controlsEnabled }) {
  const { selectedLocomotives, reported, busy, sendFunction } = roster
  const hasOn = (index) =>
    selectedLocomotives.length > 0 &&
    selectedLocomotives.every((loc) => reported[loc.dcc_address]?.functions?.includes(index))
  const disabled = busy || !controlsEnabled || selectedLocomotives.length === 0

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">Functions</h3>
        <span className="text-sm text-slate-400">
          {selectedLocomotives.length === 0
            ? 'Select locomotives in the roster'
            : `For ${selectedLocomotives.length} selected`}
        </span>
      </div>

      <div className="grid grid-cols-4 gap-2">
        {Array.from({ length: FUNCTION_COUNT }, (_, index) => (
          <button
            key={index}
            onClick={() => sendFunction(index)}
            disabled={disabled}
            className={`py-3 rounded text-sm font-semibold transition-colors ${
              hasOn(index) ? 'bg-amber-500 text-slate-900' : 'bg-slate-600 hover:bg-slate-500'
            } disabled:opacity-50`}
          >
            {functionLabel(index)}
          </button>
        ))}
      </div>

      <p className="text-xs text-slate-500">
        Press to switch a function on for every selected locomotive. Press again to switch it off.
      </p>
    </div>
  )
}
