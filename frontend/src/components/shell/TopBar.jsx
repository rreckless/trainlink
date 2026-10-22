import { useEffect, useRef, useState } from 'react'
import { AlertOctagon, Search, Zap } from 'lucide-react'
import { useStore } from '../../store'
import { locomotiveApi } from '../../services/locomotiveApi'
import { PAGES, describeTrack } from './navigation'
import SyncStatus from './SyncStatus'

// Find pages by name. Enter opens the first match
function PageSearch({ onOpen }) {
  const [query, setQuery] = useState('')
  const input = useRef(null)
  const matches = query.trim() ? PAGES.filter((page) => page.label.toLowerCase().includes(query.trim().toLowerCase())) : []

  useEffect(() => {
    const onKey = (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        input.current?.focus()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const open = (id) => {
    onOpen(id)
    setQuery('')
    input.current?.blur()
  }

  return (
    <div className="relative w-full max-w-md">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" aria-hidden="true" />
      <input
        ref={input}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && matches[0]) open(matches[0].id)
          if (event.key === 'Escape') setQuery('')
        }}
        placeholder="Search pages"
        aria-label="Search pages"
        className="w-full pl-9 pr-12 py-1.5 bg-slate-800 border border-slate-700 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:border-slate-500"
      />
      <kbd className="absolute right-3 top-1/2 -translate-y-1/2 text-[11px] text-slate-500 border border-slate-700 rounded px-1">⌘K</kbd>
      {matches.length > 0 && (
        <ul className="absolute left-0 right-0 mt-1 bg-slate-800 border border-slate-700 rounded-lg shadow-lg overflow-hidden z-10">
          {matches.map((page) => (
            <li key={page.id}>
              <button onMouseDown={() => open(page.id)} className="w-full text-left px-3 py-2 text-sm hover:bg-slate-700">
                {page.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

// Track power and the emergency stop, always in reach
function PowerControls() {
  const z21 = useStore((state) => state.z21)
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState('')
  const track = describeTrack(z21.state)
  const connected = z21.connected

  const run = async (action, done) => {
    setBusy(true)
    setNotice('')
    try {
      await action()
      setNotice(done)
    } catch (error) {
      setNotice(error.message)
    } finally {
      setBusy(false)
      setTimeout(() => setNotice(''), 4000)
    }
  }

  return (
    <div className="flex items-center gap-3">
      <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-slate-800 border border-slate-700 text-sm">
        <span className={`w-2 h-2 rounded-full ${track.dot}`} aria-hidden="true" />
        <span className="text-slate-200">{track.text}</span>
        {z21.state === 'track_power_off' && (
          <button
            onClick={() => run(() => locomotiveApi.resumeTrackPower(), 'Track power resumed')}
            disabled={busy}
            className="ml-1 px-2 py-0.5 rounded bg-green-600 hover:bg-green-500 text-white text-xs disabled:opacity-50"
          >
            Resume
          </button>
        )}
      </div>
      <button
        onClick={() => run(() => locomotiveApi.emergencyStop(), 'Emergency stop sent')}
        disabled={busy || !connected}
        title="Stop every locomotive"
        className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-red-600 hover:bg-red-500 text-white text-sm font-semibold disabled:opacity-40"
      >
        <AlertOctagon className="w-4 h-4" aria-hidden="true" />
        Emergency stop
      </button>
      {notice && <span className="text-xs text-slate-300 whitespace-nowrap">{notice}</span>}
    </div>
  )
}

export default function TopBar({ onOpen, email, onSignOut }) {
  return (
    <header className="h-14 flex-shrink-0 flex items-center gap-4 px-4 bg-slate-950 border-b border-slate-800">
      <div className="flex items-center gap-2 min-w-[10rem]">
        <div className="w-8 h-8 rounded-lg bg-green-500 flex items-center justify-center" aria-hidden="true">
          <Zap className="w-5 h-5 text-slate-950" />
        </div>
        <span className="font-semibold tracking-tight">TrainLink</span>
      </div>
      <div className="flex-1 flex justify-center">
        <PageSearch onOpen={onOpen} />
      </div>
      <PowerControls />
      <div className="flex items-center gap-3 pl-3 border-l border-slate-800">
        <SyncStatus />
        <span className="text-xs text-slate-400 max-w-[12rem] truncate" title={email}>{email}</span>
        <button onClick={onSignOut} className="px-2 py-1 rounded-md text-xs text-slate-200 bg-slate-800 border border-slate-700 hover:bg-slate-700">
          Sign out
        </button>
      </div>
    </header>
  )
}
