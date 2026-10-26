import { useEffect, useMemo, useState } from 'react'
import { useStore } from '../../store'
import { overviewApi } from '../../services/overviewApi'

const FILTERS = [
  { id: 'all', label: 'All' },
  { id: 'addressed', label: 'With an address' },
  { id: 'missing', label: 'No address' },
]

// Every turnout on every layout, with search and a filter for the ones that still need an address
export default function TurnoutsView() {
  const setActiveTab = useStore((state) => state.setActiveTab)
  const [turnouts, setTurnouts] = useState([])
  const [filter, setFilter] = useState('all')
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    overviewApi
      .turnouts()
      .then(setTurnouts)
      .catch((err) => setError(`Could not load the turnouts: ${err.message}`))
  }, [])

  const counts = useMemo(
    () => ({
      all: turnouts.length,
      addressed: turnouts.filter((turnout) => turnout.has_address).length,
      missing: turnouts.filter((turnout) => !turnout.has_address).length,
    }),
    [turnouts]
  )

  const shown = turnouts.filter((turnout) => {
    if (filter === 'addressed' && !turnout.has_address) return false
    if (filter === 'missing' && turnout.has_address) return false
    const text = query.trim().toLowerCase()
    if (text === '') return true
    return [turnout.name, turnout.layout_name, String(turnout.dcc_address ?? '')].some((value) =>
      value.toLowerCase().includes(text)
    )
  })

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-lg font-semibold flex-1">Turnouts</h3>
          <button onClick={() => setActiveTab('editor')} className="px-3 py-1 text-sm bg-slate-700 rounded hover:bg-slate-600">
            Open Layout Editor
          </button>
        </div>
        <div className="flex flex-wrap gap-2">
          {FILTERS.map((item) => (
            <button
              key={item.id}
              onClick={() => setFilter(item.id)}
              className={`px-3 py-1 rounded text-sm ${filter === item.id ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'}`}
            >
              {item.label} <span className="text-xs opacity-75">({counts[item.id]})</span>
            </button>
          ))}
        </div>
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search by name, layout or address"
          className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        />
      </div>

      {error && (
        <div className="bg-red-900 border border-red-700 rounded p-3">
          <p className="text-red-200 text-sm">{error}</p>
        </div>
      )}

      <div className="bg-slate-800 rounded-lg p-4">
        {turnouts.length === 0 ? (
          <p className="text-sm text-slate-400">
            No turnouts yet. Place turnouts in the Layout Editor, then give each one a DCC address.
          </p>
        ) : shown.length === 0 ? (
          <p className="text-sm text-slate-400">No turnouts match.</p>
        ) : (
          <ul className="space-y-2">
            {shown.map((turnout, index) => (
              <li key={`${turnout.layout_id}-${index}`} className="flex items-center gap-3 bg-slate-700 rounded px-3 py-2">
                <div className="flex-1">
                  <p className="font-semibold">{turnout.name}</p>
                  <p className="text-xs text-slate-400">{turnout.layout_name}</p>
                </div>
                {turnout.has_address ? (
                  <span className="text-sm text-green-300">Address {turnout.dcc_address}</span>
                ) : (
                  <span className="text-sm text-amber-300">No address</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
