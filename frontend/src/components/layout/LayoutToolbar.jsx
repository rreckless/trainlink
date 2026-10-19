import { useState } from 'react'

const TOOLS = [
  { id: 'select', label: 'Select' },
  { id: 'track', label: 'Track' },
  { id: 'piece', label: 'Track piece' },
  { id: 'operate', label: 'Operate' },
  { id: 'block', label: 'Block' },
  { id: 'signal', label: 'Signal' },
  { id: 'feedback', label: 'Feedback' },
]

const EMPTY_FORM = { name: '', width: 800, height: 600, scale: 'HO' }

export default function LayoutToolbar({
  layouts,
  layout,
  tool,
  dirty,
  busy,
  importing,
  onChooseTool,
  onSwitchLayout,
  onCreateLayout,
  onToggleImport,
  onSave,
}) {
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState(EMPTY_FORM)

  const submitForm = async (event) => {
    event.preventDefault()
    await onCreateLayout({
      name: form.name.trim(),
      width: Number(form.width),
      height: Number(form.height),
      scale: form.scale.trim() || 'HO',
    })
    setCreating(false)
    setForm(EMPTY_FORM)
  }

  return (
    <div className="bg-slate-800 rounded-lg p-4 space-y-4">
      <div className="flex flex-wrap gap-3 items-center">
        <select
          value={layout?.id ?? ''}
          onChange={(event) => onSwitchLayout(Number(event.target.value))}
          disabled={busy || layouts.length === 0}
          className="px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white disabled:opacity-50"
        >
          {layouts.length === 0 && <option value="">No layouts</option>}
          {layouts.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>

        <button
          onClick={() => setCreating(!creating)}
          disabled={busy}
          className="px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50"
        >
          {creating ? 'Cancel' : 'New Layout'}
        </button>

        <button
          onClick={onToggleImport}
          disabled={!layout || busy}
          className={`px-3 py-2 rounded disabled:opacity-50 ${
            importing ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'
          }`}
        >
          Import AnyRail
        </button>

        <div className="flex flex-wrap gap-2">
          {TOOLS.map((item) => (
            <button
              key={item.id}
              onClick={() => onChooseTool(item.id)}
              disabled={!layout}
              className={`px-3 py-2 rounded transition-colors disabled:opacity-50 ${
                tool === item.id ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="ml-auto flex items-center gap-3">
          {dirty && <span className="text-sm text-amber-400">Unsaved changes</span>}
          <button
            onClick={onSave}
            disabled={!layout || !dirty || busy}
            className="px-4 py-2 bg-green-600 rounded hover:bg-green-700 disabled:opacity-50"
          >
            {busy ? 'Working...' : 'Save'}
          </button>
        </div>
      </div>

      {creating && (
        <form onSubmit={submitForm} className="grid grid-cols-2 md:grid-cols-5 gap-3 items-end">
          <label className="text-sm text-slate-300 col-span-2 md:col-span-1">
            Name
            <input
              required
              value={form.name}
              onChange={(event) => setForm({ ...form, name: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="text-sm text-slate-300">
            Width
            <input
              type="number"
              min="1"
              required
              value={form.width}
              onChange={(event) => setForm({ ...form, width: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="text-sm text-slate-300">
            Height
            <input
              type="number"
              min="1"
              required
              value={form.height}
              onChange={(event) => setForm({ ...form, height: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="text-sm text-slate-300">
            Scale
            <input
              value={form.scale}
              onChange={(event) => setForm({ ...form, scale: event.target.value })}
              className="mt-1 w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <button
            type="submit"
            disabled={busy}
            className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 disabled:opacity-50"
          >
            Create
          </button>
        </form>
      )}
    </div>
  )
}
