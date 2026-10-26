import { useState } from 'react'
import { FUNCTION_COUNT, functionLabel } from '../../hooks/useLocomotiveRoster'
import { NATIVE_FILE_PICKER } from '../../services/api'
import { pickPictureInDesktop } from '../../services/desktopFiles'
import { locomotiveApi, pictureUrl } from '../../services/locomotiveApi'

const LOCOMOTIVE_TYPES = ['diesel', 'steam', 'electric', 'other']
const LABEL_MAX = 24

// Defaults for a new locomotive: F0 is the light, the rest are numbered
const defaultFunctions = () =>
  Array.from({ length: FUNCTION_COUNT }, (_, index) => ({
    label: functionLabel(index),
    momentary: false,
  }))

// Form for one locomotive: its name, address, picture, and what each of the 20 function buttons does
// In a dialog the dialog's own Save and Cancel act on the form, through its id, so the form has no buttons of its own
export default function LocomotiveForm({ locomotive, busy, onSave, onCancel, onRemovePicture, formId, inDialog = false }) {
  const [name, setName] = useState(locomotive?.name ?? '')
  const [address, setAddress] = useState(locomotive?.dcc_address ?? '')
  const [type, setType] = useState(locomotive?.locomotive_type ?? 'diesel')
  const [length, setLength] = useState(locomotive?.length ?? '')
  const [functions, setFunctions] = useState(locomotive?.function_labels ?? defaultFunctions())
  const [picture, setPicture] = useState(null)
  const [error, setError] = useState('')

  // The lowest address no locomotive uses yet
  const useNextAddress = async () => {
    setError('')
    try {
      const { dcc_address: next } = await locomotiveApi.nextAddress()
      setAddress(String(next))
    } catch (err) {
      setError(err.message)
    }
  }

  // The desktop app's own dialog. A cancelled dialog keeps the picture already chosen
  const choosePicture = async () => {
    setError('')
    try {
      const chosen = await pickPictureInDesktop()
      if (chosen) setPicture(chosen)
    } catch (err) {
      setError(String(err))
    }
  }

  const setFunction = (index, change) =>
    setFunctions((prev) => prev.map((item, i) => (i === index ? { ...item, ...change } : item)))

  const submit = (event) => {
    event.preventDefault()
    setError('')
    const number = Number(address)
    if (!Number.isInteger(number) || number < 1 || number > 9999) return setError('DCC address must be a whole number from 1 to 9999')
    if (functions.some((item) => item.label.trim() === '')) return setError('Every function button needs a label')
    const body = {
      name: name.trim(),
      dcc_address: number,
      locomotive_type: type,
      function_labels: functions.map((item) => ({ label: item.label.trim(), momentary: item.momentary })),
    }
    if (length !== '') body.length = Number(length)
    onSave(body, picture)
  }

  return (
    <form id={formId} onSubmit={submit} className="bg-slate-700 rounded-lg p-4 space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <label className="text-sm text-slate-300">
          Name
          <input
            required
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="mt-1 w-full px-3 py-2 bg-slate-800 rounded border border-slate-600 text-white"
          />
        </label>
        <div className="text-sm text-slate-300">
          <label className="block">
            DCC address (1-9999)
            <div className="mt-1 flex gap-2">
              <input
                type="number"
                min="1"
                max="9999"
                required
                value={address}
                onChange={(event) => setAddress(event.target.value)}
                className="w-full px-3 py-2 bg-slate-800 rounded border border-slate-600 text-white"
              />
              <button type="button" onClick={useNextAddress} disabled={busy} className="px-3 py-2 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50 whitespace-nowrap">
                Next free
              </button>
            </div>
          </label>
        </div>
        <label className="text-sm text-slate-300">
          Type
          <select
            value={type}
            onChange={(event) => setType(event.target.value)}
            className="mt-1 w-full px-3 py-2 bg-slate-800 rounded border border-slate-600 text-white"
          >
            {LOCOMOTIVE_TYPES.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <label className="text-sm text-slate-300">
          Length (cm, optional)
          <input
            type="number"
            min="0"
            step="0.1"
            value={length}
            onChange={(event) => setLength(event.target.value)}
            className="mt-1 w-full px-3 py-2 bg-slate-800 rounded border border-slate-600 text-white"
          />
        </label>
      </div>

      <div className="flex items-center gap-3">
        {locomotive && pictureUrl(locomotive) && !picture && (
          <img src={pictureUrl(locomotive)} alt="" className="w-24 h-16 object-cover rounded bg-slate-800" />
        )}
        {picture && <span className="text-sm text-slate-300">New picture: {picture.name}</span>}
        <div className="text-sm text-slate-300">
          <span className="block mb-1">Picture (JPEG, PNG, or WebP, 2 MB max)</span>
          {NATIVE_FILE_PICKER ? (
            <button
              type="button"
              onClick={choosePicture}
              disabled={busy}
              className="px-3 py-2 bg-slate-700 rounded text-white hover:bg-slate-600 disabled:opacity-50"
            >
              Choose picture…
            </button>
          ) : (
            <label className="inline-block px-3 py-2 bg-slate-700 rounded text-white hover:bg-slate-600 cursor-pointer">
              Choose picture…
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp"
                onChange={(event) => setPicture(event.target.files[0] ?? null)}
                className="sr-only"
              />
            </label>
          )}
        </div>
        {locomotive?.image_url && (
          <button type="button" onClick={() => onRemovePicture(locomotive)} disabled={busy} className="px-3 py-1 bg-slate-800 rounded text-sm hover:bg-red-800">
            Remove picture
          </button>
        )}
      </div>

      <div className="space-y-2">
        <h4 className="font-semibold">Function buttons</h4>
        <p className="text-xs text-slate-400">
          Name each function as this locomotive uses it. Tick Momentary for functions that are on only while held, such as a horn.
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {functions.map((item, index) => (
            <div key={index} className="flex items-center gap-2 bg-slate-800 rounded p-2">
              <span className="w-10 text-xs text-slate-400">F{index}</span>
              <input
                value={item.label}
                maxLength={LABEL_MAX}
                onChange={(event) => setFunction(index, { label: event.target.value })}
                className="flex-1 px-2 py-1 bg-slate-700 rounded border border-slate-600 text-white text-sm"
              />
              <label className="flex items-center gap-1 text-xs text-slate-300">
                <input type="checkbox" checked={item.momentary} onChange={(event) => setFunction(index, { momentary: event.target.checked })} />
                Momentary
              </label>
            </div>
          ))}
        </div>
      </div>

      {error && <p className="text-sm text-red-300">{error}</p>}

      {!inDialog && (
        <div className="flex gap-2">
          <button type="submit" disabled={busy} className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 disabled:opacity-50">
            Save locomotive
          </button>
          <button type="button" onClick={onCancel} disabled={busy} className="px-4 py-2 bg-slate-800 rounded hover:bg-slate-600 disabled:opacity-50">
            Cancel
          </button>
        </div>
      )}
    </form>
  )
}
