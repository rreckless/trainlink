export default function SystemSettings() {
  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Command Stations</h3>

        <div className="space-y-3 mb-4">
          <div className="bg-slate-700 rounded-lg p-3 flex justify-between items-center">
            <div>
              <h4 className="font-semibold">Main Station</h4>
              <p className="text-sm text-slate-400">Märklin P50x</p>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 bg-red-500 rounded-full"></div>
              <span className="text-sm">Disconnected</span>
            </div>
          </div>
        </div>

        <button className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700">
          Add Command Station
        </button>
      </div>

      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Network Settings</h3>

        <div className="space-y-3">
          <div>
            <label className="block text-sm text-slate-300 mb-1">Server Address</label>
            <input
              type="text"
              defaultValue="localhost:8000"
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </div>

          <div>
            <label className="block text-sm text-slate-300 mb-1">WebSocket Port</label>
            <input
              type="text"
              defaultValue="8000"
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </div>

          <button className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700">
            Save Settings
          </button>
        </div>
      </div>

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
