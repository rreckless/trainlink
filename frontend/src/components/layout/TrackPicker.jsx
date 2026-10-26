import { useMemo, useState } from 'react'
import { pieceShape } from '../../layout/track'

function describe(piece) {
  if (piece.type === 'straight') return `${piece.length} mm straight`
  if (piece.type === 'curve') {
    return piece.radius ? `${piece.radius} mm radius, ${piece.angle}°` : 'Radius set per placement'
  }
  if (piece.type === 'turnout') {
    const angle = piece.angle_meaning === 'frog' ? `${piece.angle}° frog` : `${piece.angle}°`
    return `${piece.hand}-hand turnout, ${piece.length} mm, ${piece.radius} mm radius, ${angle}`
  }
  return piece.type.replace('_', ' ')
}

// Why a piece can't be picked, or null if it can
function blockedReason(piece, trackSystem) {
  if (!pieceShape(piece)) return 'Not placeable yet. Geometry not confirmed for this piece'
  if (trackSystem && piece.system !== trackSystem) return `This layout uses ${trackSystem}`
  return null
}

export default function TrackPicker({ library, trackSystem, pieceId, onPick, freePlace, onToggleFree }) {
  const [query, setQuery] = useState('')

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase()
    const matches = library.filter(
      (piece) =>
        !needle ||
        piece.part_number.toLowerCase().includes(needle) ||
        piece.name.toLowerCase().includes(needle)
    )
    const bySystem = new Map()
    for (const piece of matches) {
      if (!bySystem.has(piece.series)) bySystem.set(piece.series, [])
      bySystem.get(piece.series).push(piece)
    }
    return [...bySystem.entries()]
  }, [library, query])

  return (
    <div className="space-y-3 text-sm">
      <p className="font-semibold">Track pieces</p>
      <p className="text-xs text-slate-400">
        {library.length === 0
          ? 'Loading the track library...'
          : `${library[0].manufacturer} ${library[0].gauge} gauge. Dimensions are unverified until you check them against the catalogue.`}
      </p>

      {onToggleFree && (
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={freePlace} onChange={(event) => onToggleFree(event.target.checked)} />
          Place without connecting
        </label>
      )}

      <input
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Search part number or name"
        className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
      />

      <div className="max-h-96 overflow-auto space-y-3">
        {groups.map(([system, pieces]) => (
          <div key={system}>
            <p className="text-xs uppercase tracking-wide text-slate-400 mb-1">{system}</p>
            <div className="space-y-1">
              {pieces.map((piece) => {
                const reason = blockedReason(piece, trackSystem)
                const selected = piece.id === pieceId
                return (
                  <button
                    key={piece.id}
                    onClick={() => onPick(piece.id)}
                    disabled={Boolean(reason)}
                    title={reason || undefined}
                    className={`w-full text-left px-3 py-2 rounded disabled:opacity-40 ${
                      selected ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'
                    }`}
                  >
                    <span className="font-semibold">{piece.part_number}</span>{' '}
                    <span>{piece.name}</span>
                    <span className="block text-xs text-slate-300">{describe(piece)}</span>
                    {reason && <span className="block text-xs text-amber-300">{reason}</span>}
                  </button>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
