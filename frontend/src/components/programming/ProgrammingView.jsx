import { useEffect, useState } from 'react'
import { locomotiveApi } from '../../services/locomotiveApi'
import { programmingApi } from '../../services/programmingApi'

// The CVs most decoders have, by their standard numbers and ranges. A decoder profile can add more later
const KNOWN_CVS = [
  { cv: 1, name: 'Short address', range: '1 to 127' },
  { cv: 2, name: 'Start voltage (minimum speed)', range: '0 to 255' },
  { cv: 3, name: 'Acceleration', range: '0 to 255' },
  { cv: 4, name: 'Deceleration', range: '0 to 255' },
  { cv: 5, name: 'Maximum speed', range: '0 to 255' },
  { cv: 6, name: 'Middle speed', range: '0 to 255' },
  { cv: 7, name: 'Version (read only)', range: 'read only' },
  { cv: 8, name: 'Manufacturer (read only)', range: 'read only' },
  { cv: 17, name: 'Long address, high byte', range: '192 to 231' },
  { cv: 18, name: 'Long address, low byte', range: '0 to 255' },
  { cv: 19, name: 'Consist address (multiple unit)', range: '0 to 127' },
  { cv: 29, name: 'Configuration (bits)', range: '0 to 255' },
]

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

export default function ProgrammingView() {
  const [mode, setMode] = useState('track')
  const [locomotives, setLocomotives] = useState([])
  const [locomotiveId, setLocomotiveId] = useState('')
  const [cv, setCv] = useState(1)
  const [value, setValue] = useState('')
  const [confirmed, setConfirmed] = useState(false)
  const [readValue, setReadValue] = useState(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    locomotiveApi
      .list()
      .then((list) => {
        setLocomotives(list)
        if (list.length > 0) setLocomotiveId(String(list[0].id))
      })
      .catch(() => {})
  }, [])

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
      await programmingApi.read(Number(cv))
      const answer = await waitForAnswer()
      if (answer.error) return setError(answer.error)
      setReadValue(answer.value)
      setMessage(`CV ${cv} is ${answer.value}`)
    })

  const write = () =>
    run(async () => {
      const number = Number(value)
      if (mode === 'main') {
        await programmingApi.pomWrite(Number(locomotiveId), Number(cv), number)
        return setMessage(`Sent CV ${cv} = ${number}. Not checked: the Z21 gives no answer on the main track`)
      }
      await programmingApi.write(Number(cv), number)
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

  const valueNumber = Number(value)
  const valueOk = value !== '' && Number.isInteger(valueNumber) && valueNumber >= 0 && valueNumber <= 255
  const cvOk = Number.isInteger(Number(cv)) && Number(cv) >= 1 && Number(cv) <= 1024

  return (
    <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 items-start">
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <h3 className="text-lg font-semibold">Known CVs</h3>
        <ul className="space-y-1">
          {KNOWN_CVS.map((known) => (
            <li key={known.cv}>
              <button
                onClick={() => setCv(known.cv)}
                className={`w-full text-left px-3 py-2 rounded ${Number(cv) === known.cv ? 'bg-blue-600 text-white' : 'bg-slate-700 hover:bg-slate-600'}`}
              >
                <span className="font-semibold">CV {known.cv}</span> <span className="text-sm">{known.name}</span>
                <span className="block text-xs opacity-75">{known.range}</span>
              </button>
            </li>
          ))}
        </ul>
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
              The locomotive stays on the main track. The Z21 gives no answer for these writes, so they cannot be checked. Reading is not
              available on the main track.
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

        <div className="flex flex-wrap items-center gap-3">
          {mode === 'track' && (
            <button onClick={read} disabled={busy || !cvOk} className="px-4 py-2 bg-slate-600 rounded font-semibold hover:bg-slate-500 disabled:opacity-50">
              Read CV
            </button>
          )}
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
            disabled={busy || !cvOk || !valueOk || !confirmed || (mode === 'main' && locomotiveId === '')}
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
