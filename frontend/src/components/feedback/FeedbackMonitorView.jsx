import { useCallback, useEffect, useState } from 'react'
import { overviewApi } from '../../services/overviewApi'

// How often the page asks the Z21 for the sensors while it is open
const REFRESH_MS = 2000

const CELL = {
  occupied: 'bg-red-700 text-white',
  free: 'bg-green-800 text-green-100',
  unknown: 'bg-slate-700 text-slate-400',
}

const clock = (at) => (Number.isNaN(Date.parse(at)) ? at : new Date(at).toLocaleTimeString())

// The 128 feedback addresses in modules of 8. Unlinked addresses have a dashed border. The reports are kept only while the app runs
export default function FeedbackMonitorView() {
  const [monitor, setMonitor] = useState(null)
  const [error, setError] = useState('')

  const check = useCallback(
    () =>
      overviewApi
        .feedbackMonitor()
        .then((result) => {
          setMonitor(result)
          setError('')
        })
        .catch((err) => setError(`Could not read the sensors: ${err.message}`)),
    []
  )

  useEffect(() => {
    check()
    const timer = setInterval(check, REFRESH_MS)
    return () => clearInterval(timer)
  }, [check])

  const modules = monitor ? [...new Set(monitor.inputs.map((item) => item.module))] : []

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between flex-wrap gap-2">
          <h3 className="text-lg font-semibold">Feedback monitor</h3>
          <button onClick={check} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
            Read now
          </button>
        </div>
        {error && <p className="text-sm text-red-300">{error}</p>}
        {monitor && !monitor.connected && <p className="text-sm text-amber-300">The Z21 is not connected, so the states below are the last ones it reported.</p>}
        {monitor && (
          <p className="text-sm text-slate-300">
            {monitor.counts.addresses} addresses · {monitor.counts.known} reported · {monitor.counts.occupied} occupied · {monitor.counts.free} free ·{' '}
            {monitor.counts.not_linked} not linked
          </p>
        )}
        <div className="flex flex-wrap gap-3 text-xs text-slate-400">
          <span className="flex items-center gap-1"><span className="inline-block w-3 h-3 rounded bg-red-700" /> Occupied</span>
          <span className="flex items-center gap-1"><span className="inline-block w-3 h-3 rounded bg-green-800" /> Free</span>
          <span className="flex items-center gap-1"><span className="inline-block w-3 h-3 rounded bg-slate-700" /> Not reported yet</span>
          <span className="flex items-center gap-1"><span className="inline-block w-3 h-3 rounded border border-dashed border-slate-400" /> Not linked to a feedback</span>
        </div>
        {monitor && (
          <div className="space-y-1 overflow-x-auto">
            {modules.map((module) => (
              <div key={module} className="flex items-center gap-2">
                <span className="w-16 shrink-0 text-xs text-slate-400">Module {module}</span>
                <div className="grid grid-cols-8 gap-1 flex-1 min-w-[320px]">
                  {monitor.inputs
                    .filter((item) => item.module === module)
                    .map((item) => (
                      <div
                        key={item.number}
                        title={item.feedback ? `${item.feedback.name} (feedback ${item.number})` : `Feedback ${item.number}, not linked`}
                        className={`rounded px-1 py-2 text-center text-xs ${CELL[item.state]} ${
                          item.feedback ? '' : 'border border-dashed border-slate-400 opacity-70'
                        }`}
                      >
                        {item.number}
                      </div>
                    ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="bg-slate-800 rounded-lg p-4 space-y-2">
        <h4 className="font-semibold">Latest reports</h4>
        <p className="text-xs text-slate-400">Kept while the app runs. A report is written when a sensor changes, or when it is first seen occupied.</p>
        {monitor && monitor.reports.length === 0 && <p className="text-sm text-slate-400">No changes reported yet.</p>}
        {monitor && monitor.reports.length > 0 && (
          <ul className="space-y-1">
            {monitor.reports.slice(0, 50).map((report, index) => (
              <li key={`${report.at}-${report.number}-${index}`} className="text-sm bg-slate-700 rounded px-3 py-2 flex justify-between gap-3">
                <span>
                  {clock(report.at)} · {report.number} {report.occupied ? 'occupied' : 'free'}
                </span>
                <span className="text-xs text-slate-400">{report.feedback ? report.feedback.name : 'not linked'}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
