import { useState } from 'react'
import { useStore } from '../../store'
import { refreshZ21Status } from '../../hooks/useZ21Status'
import { locomotiveApi } from '../../services/locomotiveApi'
import { useZ21Connection } from '../../hooks/useZ21Connection'
import { useLocomotiveRoster } from '../../hooks/useLocomotiveRoster'
import LocomotiveRoster from './LocomotiveRoster'
import FunctionPanel from './FunctionPanel'
import OperatePanel from './OperatePanel'

const TRACK_MESSAGES = {
  disconnected: 'Connect the Z21 in Settings to control locomotives and turnouts',
  lost: 'The Z21 has stopped answering. TrainLink is reconnecting, and commands are blocked until it does',
  unknown: 'Waiting for the Z21 to report its track state',
  short_circuit: 'The Z21 reports a short circuit. Fix the short, then resume track power',
  track_power_off: 'Resume track power to run locomotives and turnouts',
  ready: 'Ready',
}

export default function ControlView() {
  // The Z21 is polled once in App and shared through the store
  const z21 = useStore((state) => state.z21)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const controlsEnabled = z21.state === 'ready'
  const connection = useZ21Connection()
  const roster = useLocomotiveRoster(setError)

  const resumeTrackPower = async () => {
    setBusy(true)
    setError('')
    try {
      await locomotiveApi.resumeTrackPower()
      refreshZ21Status()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 flex flex-wrap items-center gap-3">
        <span className="text-sm text-slate-300">
          Z21 at {connection.host}:{connection.port}
        </span>
        <div className="ml-auto flex gap-2">
          {!z21.connected ? (
            <button
              onClick={connection.connect}
              disabled={connection.loading}
              className="px-4 py-2 bg-green-600 rounded hover:bg-green-700 disabled:opacity-50 transition-colors"
            >
              {connection.loading ? 'Connecting...' : 'Connect to Z21'}
            </button>
          ) : (
            <>
              <button
                onClick={connection.disconnect}
                disabled={connection.loading}
                className="px-4 py-2 bg-red-600 rounded hover:bg-red-700 disabled:opacity-50 transition-colors"
              >
                {connection.loading ? 'Disconnecting...' : 'Disconnect from Z21'}
              </button>
              {z21.state !== 'ready' && (
                <button
                  onClick={connection.connect}
                  disabled={connection.loading}
                  className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {connection.loading ? 'Reconnecting...' : 'Reconnect to Z21'}
                </button>
              )}
            </>
          )}
        </div>
        {connection.error && <p className="w-full text-sm text-red-300">Error: {connection.error}</p>}
      </div>

      <div
        className={`rounded-lg p-4 ${
          controlsEnabled ? 'bg-green-900 border border-green-700' : 'bg-amber-900 border border-amber-700'
        }`}
      >
        <p className="font-semibold">{TRACK_MESSAGES[z21.state] ?? TRACK_MESSAGES.disconnected}</p>
        {z21.connected && z21.state !== 'ready' && z21.state !== 'unknown' && (
          <button
            onClick={resumeTrackPower}
            disabled={busy || z21.state === 'short_circuit'}
            className="mt-3 px-4 py-2 bg-slate-700 rounded font-semibold hover:bg-slate-600 disabled:opacity-50"
          >
            Resume Track Power
          </button>
        )}
      </div>

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-4 items-start">
        <LocomotiveRoster roster={roster} controlsEnabled={controlsEnabled} onError={setError} />
        <FunctionPanel roster={roster} controlsEnabled={controlsEnabled} />
      </div>
      <OperatePanel />
    </div>
  )
}
