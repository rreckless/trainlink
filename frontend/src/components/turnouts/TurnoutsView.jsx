import { useEffect, useMemo, useState } from 'react'
import { useStore } from '../../store'
import { overviewApi } from '../../services/overviewApi'

// The settings a turnout keeps, as the editor shows them. Limits match the server
const THROW_MS_MIN = 50
const THROW_MS_MAX = 5000
const GROUP_MAX = 40
const NOTES_MAX = 500

const draftFrom = (settings) => ({
  throw_ms: settings.throw_ms,
  manual: settings.manual,
  feedback: settings.feedback,
  group: settings.group,
  notes: settings.notes,
})

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
  const [editing, setEditing] = useState(null)
  const [draft, setDraft] = useState(null)
  const [saving, setSaving] = useState(false)

  const load = () =>
    overviewApi
      .turnouts()
      .then(setTurnouts)
      .catch((err) => setError(`Could not load the turnouts: ${err.message}`))

  useEffect(() => {
    load()
  }, [])

  const key = (turnout) => `${turnout.layout_id}/${turnout.placed_id}`

  const openEditor = (turnout) => {
    setError('')
    setEditing(key(turnout))
    setDraft(draftFrom(turnout.settings))
  }

  const closeEditor = () => {
    setEditing(null)
    setDraft(null)
  }

  const save = async (turnout) => {
    setSaving(true)
    setError('')
    try {
      await overviewApi.updateTurnout(turnout.layout_id, turnout.placed_id, {
        ...draft,
        throw_ms: Number(draft.throw_ms),
      })
      await load()
      closeEditor()
    } catch (err) {
      setError(err.message)
    } finally {
      setSaving(false)
    }
  }

  const throwMsValid = Number.isInteger(Number(draft?.throw_ms)) && Number(draft?.throw_ms) >= THROW_MS_MIN && Number(draft?.throw_ms) <= THROW_MS_MAX

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
              <li key={`${turnout.layout_id}-${index}`} className="bg-slate-700 rounded px-3 py-2 space-y-2">
                <div className="flex items-center gap-3">
                  <div className="flex-1">
                    <p className="font-semibold">{turnout.name}</p>
                    <p className="text-xs text-slate-400">
                      {turnout.layout_name}
                      {turnout.settings.group && ` · ${turnout.settings.group}`}
                      {` · ${turnout.settings.throw_ms} ms`}
                      {turnout.settings.manual && ' · manual'}
                      {turnout.settings.feedback && ' · feedback'}
                    </p>
                  </div>
                  {turnout.has_address ? (
                    <span className="text-sm text-green-300">Address {turnout.dcc_address}</span>
                  ) : (
                    <span className="text-sm text-amber-300">No address</span>
                  )}
                  <button
                    onClick={() => (editing === key(turnout) ? closeEditor() : openEditor(turnout))}
                    className="px-2 py-1 text-xs bg-slate-600 rounded hover:bg-slate-500"
                  >
                    {editing === key(turnout) ? 'Close' : 'Settings'}
                  </button>
                </div>

                {editing === key(turnout) && draft && (
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3 bg-slate-800 rounded p-3">
                    <label className="space-y-1 text-sm text-slate-300">
                      Switching time (ms, {THROW_MS_MIN} to {THROW_MS_MAX})
                      <input
                        type="number"
                        min={THROW_MS_MIN}
                        max={THROW_MS_MAX}
                        value={draft.throw_ms}
                        onChange={(event) => setDraft({ ...draft, throw_ms: event.target.value })}
                        disabled={saving}
                        className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                      />
                    </label>
                    <label className="space-y-1 text-sm text-slate-300">
                      Group
                      <input
                        value={draft.group}
                        maxLength={GROUP_MAX}
                        onChange={(event) => setDraft({ ...draft, group: event.target.value })}
                        disabled={saving}
                        className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                      />
                    </label>
                    <label className="space-y-1 text-sm text-slate-300 md:col-span-2">
                      Notes
                      <textarea
                        value={draft.notes}
                        maxLength={NOTES_MAX}
                        onChange={(event) => setDraft({ ...draft, notes: event.target.value })}
                        disabled={saving}
                        rows={2}
                        className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
                      />
                    </label>
                    <label className="flex items-center gap-2 text-sm text-slate-300">
                      <input
                        type="checkbox"
                        checked={draft.manual}
                        onChange={(event) => setDraft({ ...draft, manual: event.target.checked })}
                        disabled={saving}
                      />
                      Manual turnout (no motor)
                    </label>
                    <label className="flex items-center gap-2 text-sm text-slate-300">
                      <input
                        type="checkbox"
                        checked={draft.feedback}
                        onChange={(event) => setDraft({ ...draft, feedback: event.target.checked })}
                        disabled={saving}
                      />
                      Position feedback
                    </label>
                    <div className="md:col-span-2 flex gap-2">
                      <button
                        onClick={() => save(turnout)}
                        disabled={saving || !throwMsValid}
                        className="px-3 py-1 bg-blue-600 rounded text-sm font-semibold hover:bg-blue-500 disabled:opacity-50"
                      >
                        {saving ? 'Saving…' : 'Save settings'}
                      </button>
                      <button onClick={closeEditor} disabled={saving} className="px-3 py-1 bg-slate-700 rounded text-sm hover:bg-slate-600 disabled:opacity-50">
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
