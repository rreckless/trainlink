import { useStore } from '../store'
import { useZ21Connection } from '../hooks/useZ21Connection'
import { settingsApi } from '../services/settingsApi'

export default function SystemSettings() {
  const {
    host: z21Host,
    port: z21Port,
    setZ21Address,
    loading: z21Loading,
    error: z21Error,
    setError: setZ21Error,
    connect: handleConnect,
    disconnect: handleDisconnect,
  } = useZ21Connection()

  // Z21 state is polled once in App and shared through the store
  const z21 = useStore((state) => state.z21)
  const z21Connected = z21.connected
  const z21State = z21.state
  const z21LastHeartbeat = z21.lastHeartbeat

  // The address is saved to the account when its field is left, so every device uses it
  const saveAddress = async () => {
    try {
      await settingsApi.set('z21.host', z21Host)
      await settingsApi.set('z21.port', z21Port)
    } catch (err) {
      setZ21Error(err.message)
    }
  }

  return (
    <div className="space-y-4">
      {/* Z21 Connection Section */}
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Roco Z21 Connection</h3>

        {/* Connection Status */}
        <div className="mb-4 p-3 bg-slate-700 rounded">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium">Status</span>
            <div className="flex items-center gap-2">
              <div
                className={`w-3 h-3 rounded-full ${
                  z21Connected && z21State === 'ready' ? 'bg-green-500' : 'bg-red-500'
                }`}
              ></div>
              <span
                className={
                  z21Connected && z21State === 'ready'
                    ? 'text-green-400'
                    : z21Connected && z21State !== 'short_circuit'
                    ? 'text-amber-400'
                    : 'text-red-400'
                }
              >
                {!z21Connected && 'Disconnected'}
                {z21Connected && z21State === 'ready' && 'Connected'}
                {z21Connected && z21State === 'unknown' && 'Connected (checking track)'}
                {z21Connected && z21State === 'track_power_off' && 'Connected (track power off)'}
                {z21Connected && z21State === 'short_circuit' && 'Connected: SHORT CIRCUIT'}
              </span>
            </div>
          </div>

          {z21Connected && z21State === 'short_circuit' && (
            <p className="text-xs text-red-400 mb-2">
              The Z21 reports a short circuit and has cut track power. Fix the short, then
              reconnect or resume track power from the Locomotives screen.
            </p>
          )}

          {z21LastHeartbeat && (
            <p className="text-xs text-slate-400">
              Last heartbeat: {new Date(z21LastHeartbeat).toLocaleTimeString()}
            </p>
          )}

          {z21Error && (
            <p className="text-xs text-red-400 mt-2">Error: {z21Error}</p>
          )}
        </div>

        {/* Connection Settings */}
        <div className="space-y-3 mb-4">
          <div>
            <label className="block text-sm text-slate-300 mb-1">
              Z21 Host Address
            </label>
            <input
              type="text"
              value={z21Host}
              onChange={(e) => setZ21Address(e.target.value, z21Port)}
              onBlur={saveAddress}
              disabled={z21Connected}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white disabled:opacity-50"
              placeholder="10.0.0.111"
            />
          </div>

          <div>
            <label className="block text-sm text-slate-300 mb-1">
              Z21 Port
            </label>
            <input
              type="number"
              value={z21Port}
              onChange={(e) => setZ21Address(z21Host, parseInt(e.target.value))}
              onBlur={saveAddress}
              disabled={z21Connected}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white disabled:opacity-50"
              placeholder="21105"
            />
          </div>
        </div>

        {/* Connection Buttons */}
        <div className="flex gap-2">
          {!z21Connected ? (
            <button
              onClick={handleConnect}
              disabled={z21Loading}
              className="px-4 py-2 bg-green-600 rounded hover:bg-green-700 disabled:opacity-50 transition-colors"
            >
              {z21Loading ? 'Connecting...' : 'Connect to Z21'}
            </button>
          ) : (
            <>
              <button
                onClick={handleDisconnect}
                disabled={z21Loading}
                className="px-4 py-2 bg-red-600 rounded hover:bg-red-700 disabled:opacity-50 transition-colors"
              >
                {z21Loading ? 'Disconnecting...' : 'Disconnect from Z21'}
              </button>
              {z21State !== 'ready' && (
                <button
                  onClick={handleConnect}
                  disabled={z21Loading}
                  className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {z21Loading ? 'Reconnecting...' : 'Reconnect to Z21'}
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {/* Network Settings */}
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Network Settings</h3>

        <div className="space-y-3">
          <div>
            <label className="block text-sm text-slate-300 mb-1">
              Server Address
            </label>
            <input
              type="text"
              defaultValue="localhost:8000"
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </div>

          <div>
            <label className="block text-sm text-slate-300 mb-1">
              WebSocket Port
            </label>
            <input
              type="text"
              defaultValue="8000"
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </div>

          <button className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 transition-colors">
            Save Settings
          </button>
        </div>
      </div>

      {/* General Settings */}
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">General Settings</h3>

        <div className="space-y-3">
          <label className="flex items-center gap-2">
            <input type="checkbox" defaultChecked className="rounded" />
            <span>Auto-connect on startup</span>
          </label>

          <label className="flex items-center gap-2">
            <input type="checkbox" defaultChecked className="rounded" />
            <span>Enable notifications</span>
          </label>

          <label className="flex items-center gap-2">
            <input type="checkbox" className="rounded" />
            <span>Debug mode</span>
          </label>
        </div>
      </div>

      {/* Application Info */}
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Application Info</h3>

        <div className="space-y-2 text-sm text-slate-400">
          <p>Version: 0.1.0</p>
          <p>Build: development</p>
          <p>Status: Phase 1 - Foundation</p>
        </div>
      </div>
    </div>
  )
}