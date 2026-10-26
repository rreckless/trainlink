import { useState } from 'react'
import { Activity, Boxes, Gauge, MousePointer2, Pencil, Plus, Radio, Trash2, Upload } from 'lucide-react'
import { askText } from '../../services/dialogs'
import EditDialog, { DialogSection } from '../shell/EditDialog'

// The tools for the track plan, then the tools that mark what the layout is for. Pieces are placed by dragging them
// from the list, so there is no tool for that
const DRAW_TOOLS = [
  { id: 'select', label: 'Select', icon: MousePointer2 },
  { id: 'operate', label: 'Operate', icon: Gauge },
]
const MARK_TOOLS = [
  { id: 'block', label: 'Block', icon: Boxes },
  { id: 'signal', label: 'Signal', icon: Radio },
  { id: 'feedback', label: 'Feedback', icon: Activity },
]

const EMPTY_FORM = { name: '', width: 800, height: 600, scale: 'HO' }

const fieldClass = 'mt-1 w-full px-3 py-2 bg-slate-800 rounded-md border border-slate-600 text-white'

// One button in a group of tools. The active tool is lit
function ToolButton({ item, active, disabled, onChoose }) {
  const Icon = item.icon
  return (
    <button
      onClick={() => onChoose(item.id)}
      disabled={disabled}
      aria-pressed={active}
      title={item.label}
      className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors disabled:opacity-40 ${
        active ? 'bg-blue-600 text-white shadow' : 'text-slate-300 hover:bg-slate-800 hover:text-white'
      }`}
    >
      <Icon className="w-4 h-4" aria-hidden="true" />
      <span>{item.label}</span>
    </button>
  )
}

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
  onRenameLayout,
  onDeleteLayout,
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
    <div className="rounded-xl border border-slate-800 bg-slate-900 p-3 space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        {/* The layouts, as tabs. A layout is switched to by its tab */}
        <div className="flex items-center gap-1 overflow-x-auto max-w-full" role="tablist" aria-label="Layouts">
          {layouts.map((item) => (
            <button
              key={item.id}
              role="tab"
              aria-selected={layout?.id === item.id}
              onClick={() => onSwitchLayout(item.id)}
              disabled={busy}
              className={`whitespace-nowrap px-3 py-1.5 rounded-md text-sm disabled:opacity-50 ${
                layout?.id === item.id ? 'bg-slate-700 text-white' : 'text-slate-400 hover:bg-slate-800 hover:text-white'
              }`}
            >
              {item.name}
            </button>
          ))}
          <button
            onClick={() => setCreating(true)}
            disabled={busy}
            title="New layout"
            aria-label="New layout"
            className="p-1.5 rounded-md text-slate-400 hover:bg-slate-800 hover:text-white disabled:opacity-50"
          >
            <Plus className="w-4 h-4" aria-hidden="true" />
          </button>
        </div>

        <div className="ml-auto flex flex-wrap items-center gap-2">
          <button
            onClick={onToggleImport}
            disabled={!layout || busy}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm disabled:opacity-50 ${
              importing ? 'bg-blue-600 text-white' : 'text-slate-300 hover:bg-slate-800 hover:text-white'
            }`}
          >
            <Upload className="w-4 h-4" aria-hidden="true" />
            Import AnyRail
          </button>
          <button
            onClick={async () => {
              const name = await askText('Rename layout to:', layout?.name ?? '', { okLabel: 'Rename' })
              if (name !== null && name.trim()) onRenameLayout(name.trim())
            }}
            disabled={!layout || busy}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm text-slate-300 hover:bg-slate-800 hover:text-white disabled:opacity-50"
          >
            <Pencil className="w-4 h-4" aria-hidden="true" />
            Rename
          </button>
          <button
            onClick={onDeleteLayout}
            disabled={!layout || busy}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm text-slate-300 hover:bg-red-900 hover:text-white disabled:opacity-50"
          >
            <Trash2 className="w-4 h-4" aria-hidden="true" />
            Delete
          </button>
          <button
            onClick={onSave}
            disabled={!layout || !dirty || busy}
            className="flex items-center gap-2 px-4 py-1.5 rounded-md bg-green-600 text-sm font-semibold text-white hover:bg-green-500 disabled:opacity-50"
          >
            {dirty && <span className="w-1.5 h-1.5 rounded-full bg-amber-200" aria-label="Unsaved changes" />}
            {busy ? 'Working...' : 'Save'}
          </button>
        </div>
      </div>

      {layout && (
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex flex-wrap gap-1 rounded-lg bg-slate-950 p-1" role="group" aria-label="Tools">
            {DRAW_TOOLS.map((item) => (
              <ToolButton key={item.id} item={item} active={tool === item.id} disabled={busy} onChoose={onChooseTool} />
            ))}
          </div>
          <div className="inline-flex flex-wrap gap-1 rounded-lg bg-slate-950 p-1" role="group" aria-label="Marking tools">
            {MARK_TOOLS.map((item) => (
              <ToolButton key={item.id} item={item} active={tool === item.id} disabled={busy} onChoose={onChooseTool} />
            ))}
          </div>
        </div>
      )}

      {creating && (
        <EditDialog
          title="New layout"
          subtitle="The size of the area you will draw on, and its scale"
          icon={Plus}
          accent="bg-sky-500"
          busy={busy}
          onClose={() => setCreating(false)}
          footer={
            <button
              type="submit"
              form="new-layout-form"
              disabled={busy}
              className="px-4 py-2 rounded bg-blue-600 text-sm font-semibold hover:bg-blue-500 disabled:opacity-50"
            >
              Create layout
            </button>
          }
        >
          <form id="new-layout-form" onSubmit={submitForm}>
            <DialogSection title="General">
              <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                <label className="text-sm text-slate-300 md:col-span-3">
                  Name
                  <input required value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} className={fieldClass} />
                </label>
              </div>
            </DialogSection>
            <DialogSection title="Size">
              <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                <label className="text-sm text-slate-300">
                  Width
                  <input type="number" min="1" required value={form.width} onChange={(event) => setForm({ ...form, width: event.target.value })} className={fieldClass} />
                </label>
                <label className="text-sm text-slate-300">
                  Height
                  <input type="number" min="1" required value={form.height} onChange={(event) => setForm({ ...form, height: event.target.value })} className={fieldClass} />
                </label>
                <label className="text-sm text-slate-300">
                  Scale
                  <input value={form.scale} onChange={(event) => setForm({ ...form, scale: event.target.value })} className={fieldClass} />
                </label>
              </div>
            </DialogSection>
          </form>
        </EditDialog>
      )}
    </div>
  )
}
