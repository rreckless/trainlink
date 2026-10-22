import { useEffect, useState } from 'react'
import { syncApi } from '../../services/syncApi'

// When this device last synced, with a button to sync now. Polls the core's status every 20 seconds
export default function SyncStatus() {
  const [status, setStatus] = useState(null)
  const [busy, setBusy] = useState(false)

  const refresh = async () => {
    try {
      setStatus(await syncApi.status())
    } catch {
      setStatus(null)
    }
  }

  useEffect(() => {
    refresh()
    const timer = setInterval(refresh, 20000)
    return () => clearInterval(timer)
  }, [])

  const syncNow = async () => {
    setBusy(true)
    try {
      await syncApi.syncNow()
    } finally {
      setBusy(false)
      refresh()
    }
  }

  if (!status) return null
  const label = status.last_error
    ? 'Not synced'
    : status.last_synced
      ? `Synced ${new Date(`${status.last_synced}Z`).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
      : 'Not synced yet'

  return (
    <div className="flex items-center gap-2 text-xs" title={status.last_error ?? undefined}>
      <span className={`w-2 h-2 rounded-full ${status.last_error ? 'bg-amber-500' : 'bg-green-500'}`} aria-hidden="true" />
      <span className="text-slate-400 whitespace-nowrap">{label}</span>
      <button onClick={syncNow} disabled={busy} className="px-2 py-0.5 rounded bg-slate-800 border border-slate-700 text-slate-200 hover:bg-slate-700 disabled:opacity-50">
        {busy ? 'Syncing…' : 'Sync now'}
      </button>
    </div>
  )
}
