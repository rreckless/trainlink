import { useCallback, useEffect, useState } from 'react'
import { CheckCircle2, Circle } from 'lucide-react'
import { useStore } from '../../store'
import { overviewApi } from '../../services/overviewApi'

// The steps to a working setup, in order. Each step shows whether its data exists, and opens the page where it is done
export default function SetupView() {
  const setActiveTab = useStore((state) => state.setActiveTab)
  const [setup, setSetup] = useState(null)
  const [error, setError] = useState('')

  const check = useCallback(
    () =>
      overviewApi
        .setup()
        .then((result) => {
          setSetup(result)
          setError('')
        })
        .catch((err) => setError(`Could not load the checklist: ${err.message}`)),
    []
  )

  useEffect(() => {
    check()
  }, [check])

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Setup</h3>
          {setup && (
            <span className="text-sm text-slate-300">
              {setup.done} of {setup.total} done
            </span>
          )}
        </div>
        {error && <p className="text-sm text-red-300">{error}</p>}
        {!setup && !error && <p className="text-sm text-slate-400">Checking…</p>}
        {setup && (
          <ol className="space-y-2">
            {setup.steps.map((step, index) => (
              <li key={step.id} className="flex items-center gap-3 bg-slate-700 rounded px-3 py-2">
                {step.done ? (
                  <CheckCircle2 className="w-5 h-5 text-green-400 shrink-0" aria-label="Done" />
                ) : (
                  <Circle className="w-5 h-5 text-slate-500 shrink-0" aria-label="To do" />
                )}
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold">
                    {index + 1}. {step.label}
                  </div>
                  <div className="text-xs text-slate-400">{step.detail}</div>
                </div>
                <button onClick={() => setActiveTab(step.page)} className="px-3 py-1 text-sm bg-slate-600 rounded hover:bg-slate-500">
                  Open
                </button>
              </li>
            ))}
          </ol>
        )}
        {setup && (
          <button onClick={check} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
            Check again
          </button>
        )}
      </div>
    </div>
  )
}
