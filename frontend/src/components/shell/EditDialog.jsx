import { useEffect } from 'react'
import { X } from 'lucide-react'

// A pop-up for editing one thing, as LocoLink does: a title with its icon, sections of fields, and Cancel and Save at the
// foot. Escape and the close button leave without saving, unless the dialog is busy
export default function EditDialog({ title, subtitle, icon: Icon, accent = 'bg-slate-600', busy = false, error, onClose, footer, children }) {
  useEffect(() => {
    const onKey = (event) => {
      if (event.key === 'Escape' && !busy) onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [busy, onClose])

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
      <div role="dialog" aria-modal="true" aria-labelledby="edit-dialog-title" className="w-full max-w-2xl max-h-[90vh] flex flex-col rounded-xl border border-slate-700 bg-slate-900 text-white shadow-2xl">
        <div className="flex items-center gap-3 px-5 py-4 border-b border-slate-800">
          {Icon && (
            <span className={`w-9 h-9 flex-shrink-0 rounded-lg flex items-center justify-center ${accent}`}>
              <Icon className="w-5 h-5 text-white" aria-hidden="true" />
            </span>
          )}
          <div className="flex-1 min-w-0">
            <h2 id="edit-dialog-title" className="text-lg font-semibold truncate">
              {title}
            </h2>
            {subtitle && <p className="text-xs text-slate-400 truncate">{subtitle}</p>}
          </div>
          <button onClick={onClose} disabled={busy} aria-label="Close" className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-slate-800 disabled:opacity-50">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">{children}</div>

        {error && (
          <div className="mx-5 mb-3 bg-red-900 border border-red-700 rounded p-3">
            <p className="text-red-200 text-sm">{error}</p>
          </div>
        )}

        <div className="flex justify-end gap-2 px-5 py-3 border-t border-slate-800">
          <button onClick={onClose} disabled={busy} className="px-4 py-2 rounded bg-slate-700 text-sm hover:bg-slate-600 disabled:opacity-50">
            Cancel
          </button>
          {footer}
        </div>
      </div>
    </div>
  )
}

// A titled group of fields inside a dialog
export function DialogSection({ title, icon: Icon, children }) {
  return (
    <section className="space-y-3">
      <h3 className="flex items-center gap-2 text-sm font-semibold text-slate-200">
        {Icon && <Icon className="w-4 h-4 text-slate-400" aria-hidden="true" />}
        {title}
      </h3>
      <div className="space-y-3">{children}</div>
    </section>
  )
}
