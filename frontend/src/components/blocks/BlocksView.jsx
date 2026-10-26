import { useEffect, useMemo, useState } from 'react'
import { Boxes } from 'lucide-react'
import EditDialog from '../shell/EditDialog'
import { tracksideApi } from '../../services/tracksideApi'

const STATES = [
  { id: 'free', label: 'Free' },
  { id: 'occupied', label: 'Occupied' },
  { id: 'reserved', label: 'Reserved' },
]

// Blocks: their properties, the block after each one, and the feedbacks and signals they are linked to
export default function BlocksView() {
  const [blocks, setBlocks] = useState([])
  const [feedbacks, setFeedbacks] = useState([])
  const [signals, setSignals] = useState([])
  const [selectedId, setSelectedId] = useState(null)
  const [draft, setDraft] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  const refresh = () =>
    Promise.all([tracksideApi.blocks(), tracksideApi.feedbacks(), tracksideApi.signals()])
      .then(([nextBlocks, nextFeedbacks, nextSignals]) => {
        setBlocks(nextBlocks)
        setFeedbacks(nextFeedbacks)
        setSignals(nextSignals)
        setError('')
      })
      .catch((err) => setError(`Could not load the blocks: ${err.message}`))

  useEffect(() => {
    refresh()
  }, [])

  const selected = blocks.find((block) => block.id === selectedId) ?? null

  // Choosing a block loads its values into the form, so edits are kept apart until Save
  useEffect(() => {
    if (!selected) {
      setDraft(null)
      return
    }
    setDraft({
      name: selected.name,
      max_speed: selected.max_speed,
      state: selected.state,
      next_block_id: selected.next_block_id ?? '',
      feedback_ids: selected.feedback_ids,
      signal_ids: selected.signal_ids,
    })
  }, [selectedId, blocks]) // eslint-disable-line react-hooks/exhaustive-deps

  // A saved message stays until another block is chosen
  useEffect(() => {
    setMessage('')
  }, [selectedId])

  // Only the blocks, feedbacks and signals on the same layout as the selected block can be linked to it
  const sameLayout = useMemo(() => {
    if (!selected) return { blocks: [], feedbacks: [], signals: [] }
    const onLayout = (item) => item.layout_id === selected.layout_id
    return {
      blocks: blocks.filter((item) => onLayout(item) && item.id !== selected.id),
      feedbacks: feedbacks.filter(onLayout),
      signals: signals.filter(onLayout),
    }
  }, [selected, blocks, feedbacks, signals])

  const toggle = (key, id) =>
    setDraft((prev) => ({
      ...prev,
      [key]: prev[key].includes(id) ? prev[key].filter((item) => item !== id) : [...prev[key], id],
    }))

  const save = async () => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      await tracksideApi.updateBlock(selected.id, {
        name: draft.name.trim(),
        max_speed: Number(draft.max_speed),
        state: draft.state,
        next_block_id: draft.next_block_id === '' ? null : Number(draft.next_block_id),
        feedback_ids: draft.feedback_ids,
        signal_ids: draft.signal_ids,
      })
      const name = draft.name.trim()
      await refresh()
      setMessage(`Saved ${name}`)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 items-start">
      <div className="bg-slate-800 rounded-lg p-4 space-y-2">
        <h3 className="text-lg font-semibold">Blocks ({blocks.length})</h3>
        {blocks.length === 0 ? (
          <p className="text-sm text-slate-400">No blocks yet. Add them in the Layout Editor.</p>
        ) : (
          <ul className="space-y-1">
            {blocks.map((block) => (
              <li key={block.id}>
                <button
                  onClick={() => setSelectedId(block.id)}
                  className={`w-full text-left px-3 py-2 rounded flex justify-between ${
                    block.id === selectedId ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'
                  }`}
                >
                  <span>{block.name}</span>
                  <span className={block.occupied_now ? 'text-red-300' : 'text-green-300'}>
                    {block.occupied_now ? 'Occupied' : 'Free'}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {selected && draft && (
        <EditDialog
          title={selected.name}
          subtitle="Block"
          icon={Boxes}
          accent="bg-cyan-500"
          busy={busy}
          error={error}
          onClose={() => setSelectedId(null)}
          footer={
            <button
              onClick={save}
              disabled={busy || draft.name.trim() === ''}
              className="px-4 py-2 bg-green-600 rounded text-sm font-semibold hover:bg-green-700 disabled:opacity-50"
            >
              Save block
            </button>
          }
        >
          {message && <p className="text-sm text-green-300">{message}</p>}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <label className="space-y-1 text-sm text-slate-300">
            Name
            <input
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="space-y-1 text-sm text-slate-300">
            Maximum speed
            <input
              type="number"
              min="0"
              max="126"
              value={draft.max_speed}
              onChange={(event) => setDraft({ ...draft, max_speed: event.target.value })}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="space-y-1 text-sm text-slate-300">
            State, set by hand
            <select
              value={draft.state}
              onChange={(event) => setDraft({ ...draft, state: event.target.value })}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            >
              {STATES.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label}
                </option>
              ))}
            </select>
          </label>
          <label className="space-y-1 text-sm text-slate-300">
            Next block
            <select
              value={draft.next_block_id}
              onChange={(event) => setDraft({ ...draft, next_block_id: event.target.value })}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            >
              <option value="">None</option>
              {sameLayout.blocks.map((block) => (
                <option key={block.id} value={block.id}>
                  {block.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        <fieldset className="space-y-2">
          <legend className="text-sm text-slate-300 mb-1">Feedbacks that show this block occupied</legend>
          {sameLayout.feedbacks.length === 0 ? (
            <p className="text-xs text-slate-400">No feedbacks on this layout yet.</p>
          ) : (
            sameLayout.feedbacks.map((feedback) => (
              <label key={feedback.id} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={draft.feedback_ids.includes(feedback.id)}
                  onChange={() => toggle('feedback_ids', feedback.id)}
                  disabled={busy}
                />
                {feedback.name} <span className="text-xs text-slate-400">feedback {feedback.feedback_number}</span>
              </label>
            ))
          )}
        </fieldset>

        <fieldset className="space-y-2">
          <legend className="text-sm text-slate-300 mb-1">Signals that show this block&apos;s aspect</legend>
          {sameLayout.signals.length === 0 ? (
            <p className="text-xs text-slate-400">No signals on this layout yet.</p>
          ) : (
            sameLayout.signals.map((signal) => (
              <label key={signal.id} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={draft.signal_ids.includes(signal.id)}
                  onChange={() => toggle('signal_ids', signal.id)}
                  disabled={busy}
                />
                {signal.name} <span className="text-xs text-slate-400">{signal.signal_type}</span>
              </label>
            ))
          )}
        </fieldset>
        </EditDialog>
      )}
    </div>
  )
}
