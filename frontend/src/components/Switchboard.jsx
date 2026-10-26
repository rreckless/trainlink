import { useEffect, useState } from 'react'
import { useStore } from '../store'
import { tracksideApi } from '../services/tracksideApi'

// The live state of the layout: whether each block is occupied and what each signal shows. Refreshed every two seconds
const REFRESH_MS = 2000

const ASPECT_COLOURS = { stop: 'text-red-400', caution: 'text-amber-400', go: 'text-green-400' }

export default function Switchboard() {
  const layout = useStore((state) => state.layout)
  const locomotives = useStore((state) => state.locomotives)
  const [blocks, setBlocks] = useState([])
  const [signals, setSignals] = useState([])
  const [error, setError] = useState('')

  useEffect(() => {
    let cancelled = false
    const read = () =>
      Promise.all([tracksideApi.blocks(), tracksideApi.signals()])
        .then(([nextBlocks, nextSignals]) => {
          if (cancelled) return
          setBlocks(nextBlocks)
          setSignals(nextSignals)
          setError('')
        })
        .catch((err) => {
          if (!cancelled) setError(`Could not read the layout: ${err.message}`)
        })
    read()
    const timer = setInterval(read, REFRESH_MS)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [])

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

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-4">
        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Blocks ({blocks.length})</h4>
          <div className="space-y-2 max-h-96 overflow-y-auto">
            {blocks.length === 0 ? (
              <p className="text-slate-400 text-sm">No blocks configured</p>
            ) : (
              blocks.map((block) => (
                <div key={block.id} className="flex justify-between text-sm gap-2">
                  <span>
                    {block.name}
                    {block.detected === null && <span className="ml-2 text-xs text-slate-500">no feedback</span>}
                  </span>
                  <span className={block.occupied_now ? 'text-red-400' : 'text-green-400'}>
                    {block.occupied_now ? 'Occupied' : 'Free'}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="bg-slate-800 rounded-lg p-4">
          <h4 className="font-semibold mb-3">Signals ({signals.length})</h4>
          <div className="space-y-2 max-h-96 overflow-y-auto">
            {signals.length === 0 ? (
              <p className="text-slate-400 text-sm">No signals configured</p>
            ) : (
              signals.map((signal) => (
                <div key={signal.id} className="flex justify-between text-sm gap-2">
                  <span>
                    {signal.name}
                    {signal.aspect_source === 'manual' && <span className="ml-2 text-xs text-slate-500">set by hand</span>}
                  </span>
                  <span className={ASPECT_COLOURS[signal.aspect] ?? 'text-slate-400'}>{signal.aspect}</span>
                </div>
              ))
            )}
          </div>
        </div>

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
