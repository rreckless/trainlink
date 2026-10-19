import { useStore } from '../store'

export default function LocomotivePanel() {
  const locomotives = useStore((state) => state.locomotives)
  const selectedLocomotive = useStore((state) => state.selectedLocomotive)
  const updateLocomotiveSpeed = useStore((state) => state.updateLocomotiveSpeed)
  const updateLocomotiveDirection = useStore((state) => state.updateLocomotiveDirection)

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Locomotive Control</h3>

        {locomotives.length === 0 ? (
          <p className="text-slate-400 text-center py-8">No locomotives configured</p>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {locomotives.map((loc) => (
              <div
                key={loc.id}
                className={`bg-slate-700 rounded-lg p-4 cursor-pointer transition-colors ${
                  selectedLocomotive?.id === loc.id
                    ? 'border-2 border-blue-500'
                    : 'border border-slate-600 hover:border-slate-500'
                }`}
              >
                <h4 className="font-semibold mb-3">{loc.name}</h4>

                <div className="space-y-3">
                  {/* Speed control */}
                  <div>
                    <label className="text-sm text-slate-300">Speed: {loc.speed}%</label>
                    <input
                      type="range"
                      min="0"
                      max="100"
                      value={loc.speed}
                      onChange={(e) => updateLocomotiveSpeed(loc.id, parseInt(e.target.value))}
                      className="w-full mt-1"
                    />
                  </div>

                  {/* Direction control */}
                  <div>
                    <label className="text-sm text-slate-300 block mb-1">Direction</label>
                    <div className="flex gap-2">
                      {['reverse', 'stop', 'forward'].map((dir) => (
                        <button
                          key={dir}
                          onClick={() => updateLocomotiveDirection(loc.id, dir)}
                          className={`flex-1 py-1 rounded text-sm transition-colors ${
                            loc.direction === dir
                              ? 'bg-blue-600'
                              : 'bg-slate-600 hover:bg-slate-500'
                          }`}
                        >
                          {dir.toUpperCase()}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Stats */}
                  <div className="text-xs text-slate-400 space-y-1 pt-2 border-t border-slate-600">
                    <p>Address: {loc.dcc_address}</p>
                    <p>Time: {loc.travelled_time.toFixed(1)}s</p>
                    <p>Distance: {loc.travelled_distance.toFixed(2)}</p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Global Controls</h3>
        <div className="flex gap-2">
          <button className="px-4 py-2 bg-green-600 rounded hover:bg-green-700 transition-colors">
            Emergency Stop
          </button>
          <button className="px-4 py-2 bg-slate-700 rounded hover:bg-slate-600 transition-colors">
            Stop All Trains
          </button>
        </div>
      </div>
    </div>
  )
}
