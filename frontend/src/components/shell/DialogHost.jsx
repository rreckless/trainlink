import { useEffect, useRef, useState } from 'react'
import { closeDialog, subscribeDialog } from '../../services/dialogs'

// Shows the question that services/dialogs.js has open: a confirmation, or a question with a text answer
export default function DialogHost() {
  const [dialog, setDialog] = useState(null)
  const [value, setValue] = useState('')
  const inputRef = useRef(null)
  const isText = dialog?.kind === 'text'

  useEffect(() => subscribeDialog((next) => {
    setDialog(next)
    setValue(next?.value ?? '')
  }), [])

  useEffect(() => {
    if (!dialog) return undefined
    if (isText) inputRef.current?.focus()
    const onKey = (event) => {
      if (event.key === 'Escape') closeDialog(isText ? null : false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [dialog, isText])

  if (!dialog) return null

  const submit = (event) => {
    event.preventDefault()
    closeDialog(isText ? value : true)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <form
        role="dialog"
        aria-modal="true"
        aria-label={dialog.message}
        onSubmit={submit}
        className="w-full max-w-md space-y-4 rounded-xl border border-slate-700 bg-slate-800 p-6 text-white"
      >
        <p>{dialog.message}</p>
        {isText && (
          <input
            ref={inputRef}
            value={value}
            onChange={(event) => setValue(event.target.value)}
            className="w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-white focus:outline-none focus:border-slate-500"
          />
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={() => closeDialog(isText ? null : false)}
            className="rounded bg-slate-700 px-4 py-2 hover:bg-slate-600"
          >
            Cancel
          </button>
          <button
            type="submit"
            className={`rounded px-4 py-2 font-semibold ${dialog.danger ? 'bg-red-700 hover:bg-red-600' : 'bg-green-600 hover:bg-green-500'}`}
          >
            {dialog.okLabel}
          </button>
        </div>
      </form>
    </div>
  )
}
