import { useStore } from '../store'

export default function Switchboard() {
  const layout = useStore((state) => state.layout)
  const blocks = useStore((state) => state.blocks)
  const locomotives = useStore((state) => state.locomotives)

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4">
        <h3 className="text-lg font-semibold mb-4">Layout Overview</h3>

        {layout ? (
          <div className="bg-slate-900 rounded-lg p-8 text-center">
            <div className="w-full h-96 bg-slate-700 rounded border-2 border-slate-600 flex items-center justify-center">
              <p className="text-slate-400">Canvas for {layout.name}</p>
            </div>
            <p className="text-sm text-slate-400 mt-4">
              Dimensions: {layout.width}x{layout.height} - Scale: {layout.scale}
            </p>
          </div>
        ) : (
          <div className="bg-slate-900 rounded-lg p-8 text-center">
            <p className="text-slate-400">No layout loaded</p>
          </div>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* Blocks info */}
        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Blocks ({blocks.length})</h4>
          <div className="space-y-2">
            {blocks.length === 0 ? (
              <p className="text-slate-400 text-sm">No blocks configured</p>
            ) : (
              blocks.slice(0, 5).map((block) => (
                <div key={block.id} className="flex justify-between text-sm">
                  <span>{block.name}</span>
                  <span className={block.occupied ? 'text-red-400' : 'text-green-400'}>
                    {block.occupied ? 'Occupied' : 'Free'}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Locomotives info */}
        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Locomotives ({locomotives.length})</h4>
          <div className="space-y-2">
            {locomotives.length === 0 ? (
              <p className="text-slate-400 text-sm">No locomotives configured</p>
            ) : (
              locomotives.slice(0, 5).map((loc) => (
                <div key={loc.id} className="flex justify-between text-sm">
                  <span>{loc.name}</span>
                  <span className="text-slate-400">{loc.speed}%</span>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
