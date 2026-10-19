import { useStore } from '../store'

export default function RouteManager() {
  const routes = useStore((state) => state.routes)

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4">
        <div className="flex justify-between items-center mb-4">
          <h3 className="text-lg font-semibold">Route Manager</h3>
          <button className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700">
            New Route
          </button>
        </div>

        {routes.length === 0 ? (
          <p className="text-slate-400 text-center py-8">No routes configured</p>
        ) : (
          <div className="space-y-2">
            {routes.map((route) => (
              <div
                key={route.id}
                className="bg-slate-700 rounded-lg p-4 flex justify-between items-center"
              >
                <div>
                  <h4 className="font-semibold">{route.name}</h4>
                  <p className="text-sm text-slate-400">
                    {route.block_sequence.length} blocks
                  </p>
                </div>
                <div className="flex gap-2">
                  <button
                    className={`px-3 py-1 rounded text-sm ${
                      route.is_running
                        ? 'bg-red-600 hover:bg-red-700'
                        : 'bg-green-600 hover:bg-green-700'
                    }`}
                  >
                    {route.is_running ? 'Stop' : 'Run'}
                  </button>
                  <button className="px-3 py-1 bg-slate-600 rounded text-sm hover:bg-slate-500">
                    Edit
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Route Options</h3>
        <div className="space-y-3">
          <label className="flex items-center gap-2">
            <input type="checkbox" className="rounded" />
            <span>Auto return to start</span>
          </label>
          <label className="flex items-center gap-2">
            <input type="checkbox" className="rounded" />
            <span>Loop continuously</span>
          </label>
        </div>
      </div>
    </div>
  )
}
