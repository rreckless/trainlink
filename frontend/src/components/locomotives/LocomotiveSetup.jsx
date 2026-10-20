import { useEffect, useState } from 'react'
import { locomotiveApi, pictureUrl } from '../../services/locomotiveApi'
import LocomotiveForm from './LocomotiveForm'

// Add and edit locomotives: name, DCC address, picture, and what each function button does
export default function LocomotiveSetup() {
  const [locomotives, setLocomotives] = useState([])
  // null: no form open. 'new': adding. Otherwise the locomotive being edited
  const [editing, setEditing] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  const refresh = () =>
    locomotiveApi
      .list()
      .then(setLocomotives)
      .catch((err) => setError(`Could not load locomotives: ${err.message}`))

  useEffect(() => {
    refresh()
  }, [])

  const save = async (body, picture) => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      let loco
      if (editing === 'new') {
        const created = await locomotiveApi.create({
          name: body.name,
          dcc_address: body.dcc_address,
          locomotive_type: body.locomotive_type,
          length: body.length,
        })
        loco = await locomotiveApi.update(created.id, body)
      } else {
        loco = await locomotiveApi.update(editing.id, body)
      }
      if (picture) loco = await locomotiveApi.uploadImage(loco.id, picture)
      setMessage(`Saved ${loco.name}`)
      setEditing(null)
      await refresh()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const removePicture = async (loco) => {
    setBusy(true)
    setError('')
    try {
      const updated = await locomotiveApi.removeImage(loco.id)
      setEditing(updated)
      await refresh()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const remove = async (loco) => {
    if (!window.confirm(`Delete ${loco.name}? Its picture and settings are removed too.`)) return
    setBusy(true)
    setError('')
    try {
      await locomotiveApi.remove(loco.id)
      if (editing?.id === loco.id) setEditing(null)
      await refresh()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="space-y-4">
      <div className="bg-slate-800 rounded-lg p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Locomotives</h3>
          <button
            onClick={() => setEditing(editing === 'new' ? null : 'new')}
            disabled={busy}
            className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600 disabled:opacity-50"
          >
            {editing === 'new' ? 'Cancel' : 'Add locomotive'}
          </button>
        </div>

        {error && <p className="text-sm text-red-300">{error}</p>}
        {message && <p className="text-sm text-green-300">{message}</p>}

        {editing && (
          <LocomotiveForm
            key={editing === 'new' ? 'new' : editing.id}
            locomotive={editing === 'new' ? null : editing}
            busy={busy}
            onSave={save}
            onCancel={() => setEditing(null)}
            onRemovePicture={removePicture}
          />
        )}

        {locomotives.length === 0 ? (
          <p className="text-sm text-slate-400">No locomotives yet. Add one to start.</p>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {locomotives.map((loco) => (
              <div key={loco.id} className="flex gap-3 bg-slate-700 rounded p-3">
                {pictureUrl(loco) ? (
                  <img src={pictureUrl(loco)} alt="" className="w-20 h-14 object-cover rounded bg-slate-800" />
                ) : (
                  <div className="w-20 h-14 rounded bg-slate-800 flex items-center justify-center text-xs text-slate-500">No picture</div>
                )}
                <div className="flex-1">
                  <p className="font-semibold">{loco.name}</p>
                  <p className="text-xs text-slate-400">
                    Address {loco.dcc_address} · {loco.locomotive_type}
                  </p>
                  <p className="text-xs text-slate-400 truncate">
                    {loco.function_labels.slice(0, 5).map((item) => item.label).join(' · ')}…
                  </p>
                </div>
                <div className="flex flex-col gap-1">
                  <button onClick={() => setEditing(loco)} disabled={busy} className="px-2 py-1 text-xs bg-slate-600 rounded hover:bg-slate-500 disabled:opacity-50">
                    Edit
                  </button>
                  <button onClick={() => remove(loco)} disabled={busy} className="px-2 py-1 text-xs bg-slate-800 rounded hover:bg-red-800 disabled:opacity-50">
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
