import { useEffect, useState } from 'react'
import { decoderProfilesApi } from '../../services/decoderProfilesApi'
import { locomotiveApi } from '../../services/locomotiveApi'
import { programmingApi } from '../../services/programmingApi'

const POLL_MS = 500
const POLL_LIMIT_MS = 35000

// Wait until the Z21 has answered the operation in progress, and return what it said
async function waitForAnswer() {
  const started = Date.now()
  while (Date.now() - started < POLL_LIMIT_MS) {
    const { pending, last } = await programmingApi.status()
    if (!pending) return last
    await new Promise((resolve) => setTimeout(resolve, POLL_MS))
  }
  return { error: 'No answer from the Z21 in time' }
}

function rangeText(entry) {
  return entry.read_only ? 'read only' : `${entry.min} to ${entry.max}`
}

// The file a profile is imported from: a profile's own fields, as the API takes them
function readProfileFile(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      try {
        resolve(JSON.parse(reader.result))
      } catch (error) {
        reject(new Error('The file is not valid JSON'))
      }
    }
    reader.onerror = () => reject(new Error('The file could not be read'))
    reader.readAsText(file)
  })
}

export default function ProgrammingView() {
  const [mode, setMode] = useState('track')
  const [profiles, setProfiles] = useState([])
  const [profileId, setProfileId] = useState('')
  const [locomotives, setLocomotives] = useState([])
  const [locomotiveId, setLocomotiveId] = useState('')
  const [cv, setCv] = useState(1)
  const [value, setValue] = useState('')
  const [confirmed, setConfirmed] = useState(false)
  const [readValue, setReadValue] = useState(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')

  const profile = profiles.find((item) => String(item.id) === profileId) ?? null
  const cvs = profile?.cvs ?? []
  const entry = cvs.find((item) => item.cv === Number(cv)) ?? null
  const locomotive = locomotives.find((item) => String(item.id) === locomotiveId) ?? null

  const loadProfiles = async () => {
    const list = await decoderProfilesApi.list()
    setProfiles(list)
    return list
  }

  useEffect(() => {
    loadProfiles()
      .then((list) => {
        if (list.length > 0) setProfileId(String(list[0].id))
      })
      .catch(() => {})
    locomotiveApi
      .list()
      .then((list) => {
        setLocomotives(list)
        if (list.length > 0) setLocomotiveId(String(list[0].id))
      })
      .catch(() => {})
  }, [])

  // A locomotive that has a profile of its own starts with that profile
  useEffect(() => {
    if (locomotive?.decoder_profile_id) setProfileId(String(locomotive.decoder_profile_id))
  }, [locomotive?.id, locomotive?.decoder_profile_id])

  const run = async (action) => {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      await action()
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }

  const read = () =>
    run(async () => {
      if (mode === 'main') {
        await programmingApi.pomRead(Number(locomotiveId), Number(cv))
      } else {
        await programmingApi.read(Number(cv))
      }
      const answer = await waitForAnswer()
      if (answer.error) return setError(answer.error)
      setReadValue(answer.value)
      setMessage(`CV ${cv} is ${answer.value}`)
    })

  const write = () =>
    run(async () => {
      const number = Number(value)
      if (mode === 'main') {
        await programmingApi.pomWrite(Number(locomotiveId), Number(cv), number, profile?.id)
        return setMessage(`Sent CV ${cv} = ${number}. Not checked: the Z21 gives no answer for a write on the main track`)
      }
      await programmingApi.write(Number(cv), number, profile?.id)
      const written = await waitForAnswer()
      if (written.error) return setError(written.error)
      // Read the CV back, so the write is checked
      await programmingApi.read(Number(cv))
      const back = await waitForAnswer()
      if (back.error) return setError(`The write was sent, but the read-back failed: ${back.error}`)
      setReadValue(back.value)
      setMessage(
        back.value === number
          ? `CV ${cv} is now ${number}, checked by reading it back`
          : `Mismatch: CV ${cv} reads back as ${back.value}, not ${number}`
      )
    })

  const useForLocomotive = () =>
    run(async () => {
      await locomotiveApi.update(Number(locomotiveId), { decoder_profile_id: profile.id })
      setLocomotives(await locomotiveApi.list())
      setMessage(`${locomotive.name} now uses the ${profile.name} profile`)
    })

  const importProfile = (event) =>
    run(async () => {
      const file = event.target.files?.[0]
      event.target.value = ''
      if (!file) return
      const body = await readProfileFile(file)
      const created = await decoderProfilesApi.create(body)
      await loadProfiles()
      setProfileId(String(created.id))
      setMessage(`Imported the ${created.name} profile`)
    })

  const deleteProfile = () =>
    run(async () => {
      await decoderProfilesApi.remove(profile.id)
      const list = await loadProfiles()
      setProfileId(list.length > 0 ? String(list[0].id) : '')
      setLocomotives(await locomotiveApi.list())
      setMessage(`Deleted the ${profile.name} profile`)
    })

  const valueNumber = Number(value)
  const cvOk = Number.isInteger(Number(cv)) && Number(cv) >= 1 && Number(cv) <= 1024
  const valueOk = value !== '' && Number.isInteger(valueNumber) && valueNumber >= 0 && valueNumber <= 255
  // With a profile, the CV must be in it, not read-only, and the value inside its range
  const profileOk = !profile || (entry !== null && !entry.read_only && valueNumber >= entry.min && valueNumber <= entry.max)
  const writeOk = cvOk && valueOk && profileOk
  const profileNote = profile && !entry ? `CV ${cv} is not in the ${profile.name} profile` : entry?.read_only ? `CV ${cv} is read only` : null

  return (
    <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 items-start">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <h3 className="text-lg font-semibold">Decoder profile</h3>
        <select
          value={profileId}
          onChange={(event) => setProfileId(event.target.value)}
          disabled={busy || profiles.length === 0}
          className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
        >
          {profiles.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
        {profile && <p className="text-xs text-slate-400">{profile.description || profile.manufacturer}</p>}

        <div className="flex flex-wrap gap-2">
          {locomotive && profile && (
            <button
              onClick={useForLocomotive}
              disabled={busy || String(locomotive.decoder_profile_id ?? '') === profileId}
              className="px-3 py-1 bg-slate-600 rounded text-sm hover:bg-slate-500 disabled:opacity-50"
            >
              Use for {locomotive.name}
            </button>
          )}
          {profile && !profile.builtin && (
            <button onClick={deleteProfile} disabled={busy} className="px-3 py-1 bg-red-800 rounded text-sm hover:bg-red-700 disabled:opacity-50">
              Delete profile
            </button>
          )}
        </div>
        <label className="block text-sm text-slate-300">
          Import a profile (JSON)
          <input type="file" accept="application/json,.json" onChange={importProfile} disabled={busy} className="mt-1 block w-full text-xs" />
        </label>

        <h3 className="text-lg font-semibold pt-2">CVs</h3>
        <ul className="space-y-1">
          {cvs.map((known) => (
            <li key={known.cv}>
              <button
                onClick={() => setCv(known.cv)}
                className={`w-full text-left px-3 py-2 rounded ${Number(cv) === known.cv ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'}`}
              >
                <span className="font-semibold">CV {known.cv}</span> <span className="text-sm">{known.name}</span>
                <span className="block text-xs opacity-75">{rangeText(known)}</span>
              </button>
            </li>
          ))}
        </ul>

        {profile?.functions?.length > 0 && (
          <>
            <h3 className="text-lg font-semibold pt-2">Functions</h3>
            <ul className="text-sm text-slate-300 space-y-1">
              {profile.functions.map((fn) => (
                <li key={fn.index}>
                  F{fn.index}: {fn.label}
                </li>
              ))}
            </ul>
          </>
        )}
      </div>

      <div className="xl:col-span-2 bg-slate-800 rounded-lg p-4 space-y-4">
        <div className="flex gap-2">
          <button onClick={() => setMode('track')} className={`px-3 py-1 rounded ${mode === 'track' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}>
            Programming track
          </button>
          <button onClick={() => setMode('main')} className={`px-3 py-1 rounded ${mode === 'main' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}>
            Main track (while running)
          </button>
        </div>
        {mode === 'track' ? (
          <p className="text-xs text-slate-400">
            Put one locomotive alone on the programming track. Reads and writes are checked by the Z21.
          </p>
        ) : (
          <div className="space-y-2">
            <p className="text-xs text-amber-300">
              The locomotive stays on the main track. A read works if the decoder supports RailCom. A write gets no answer from the Z21, so
              it cannot be checked.
            </p>
            <select
              value={locomotiveId}
              onChange={(event) => setLocomotiveId(event.target.value)}
              disabled={busy}
              className="px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            >
              {locomotives.length === 0 && <option value="">No locomotives</option>}
              {locomotives.map((loco) => (
                <option key={loco.id} value={loco.id}>
                  {loco.name} (address {loco.dcc_address})
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <label className="space-y-1 text-sm text-slate-300">
            CV number (1 to 1024)
            <input
              type="number"
              min="1"
              max="1024"
              value={cv}
              onChange={(event) => setCv(event.target.value)}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
          <label className="space-y-1 text-sm text-slate-300">
            New value (0 to 255)
            <input
              type="number"
              min="0"
              max="255"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              disabled={busy}
              className="w-full px-3 py-2 bg-slate-700 rounded border border-slate-600 text-white"
            />
          </label>
        </div>
        {profileNote && <p className="text-sm text-amber-300">{profileNote}</p>}

        <div className="flex flex-wrap items-center gap-3">
          <button
            onClick={read}
            disabled={busy || !cvOk || (mode === 'main' && locomotiveId === '')}
            className="px-4 py-2 bg-slate-600 rounded font-semibold hover:bg-slate-500 disabled:opacity-50"
          >
            Read CV
          </button>
        </div>

        <div className="bg-amber-900 border border-amber-700 rounded p-3 space-y-2">
          <p className="text-sm text-amber-100">
            A wrong value can stop a decoder working. Check the CV number and the value before you write.
          </p>
          <label className="flex items-center gap-2 text-sm text-amber-100">
            <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} disabled={busy} />
            I have checked the CV number and the value
          </label>
          <button
            onClick={write}
            disabled={busy || !writeOk || !confirmed || (mode === 'main' && locomotiveId === '')}
            className="px-4 py-2 bg-green-600 rounded font-semibold hover:bg-green-700 disabled:opacity-50"
          >
            Write CV
          </button>
        </div>

        {readValue !== null && <p className="text-sm text-slate-300">Last value read: {readValue}</p>}
        {error && (
          <div className="bg-red-900 border border-red-700 rounded p-3">
            <p className="text-red-200 text-sm">{error}</p>
          </div>
        )}
        {message && <p className="text-sm text-green-300">{message}</p>}
      </div>
    </div>
  )
}
