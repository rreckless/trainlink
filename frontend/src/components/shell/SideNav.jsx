import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useStore } from '../../store'
import { useZ21Connection } from '../../hooks/useZ21Connection'
import { NAV_GROUPS, describeTrack } from './navigation'

// One page in the navigation: a coloured icon tile, and a bar on the left when it is open
function NavItem({ page, active, collapsed, onOpen }) {
  const Icon = page.icon
  return (
    <button
      onClick={() => onOpen(page.id)}
      title={collapsed ? page.label : undefined}
      aria-current={active ? 'page' : undefined}
      className={`relative w-full flex items-center gap-3 rounded-lg px-2 py-1.5 text-sm transition-colors ${
        active ? 'bg-slate-800 text-white' : 'text-slate-300 hover:bg-slate-800/60'
      } ${collapsed ? 'justify-center' : ''}`}
    >
      {active && <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 rounded bg-white" aria-hidden="true" />}
      <span className={`w-7 h-7 flex-shrink-0 rounded-md flex items-center justify-center ${page.accent}`}>
        <Icon className="w-4 h-4 text-white" aria-hidden="true" />
      </span>
      {!collapsed && <span className="truncate">{page.label}</span>}
    </button>
  )
}

// The Z21 at the bottom: where it is, whether the app is connected, and connect or disconnect
function ConnectionCard({ collapsed }) {
  const z21 = useStore((state) => state.z21)
  const { host, port, connect, disconnect, loading, error } = useZ21Connection()
  const track = describeTrack(z21.state)

  if (collapsed) {
    return (
      <div className="flex justify-center py-2" title={track.text}>
        <span className={`w-2.5 h-2.5 rounded-full ${track.dot}`} aria-hidden="true" />
      </div>
    )
  }

  return (
    <div className="rounded-lg bg-slate-800 border border-slate-700 p-3 text-sm space-y-2">
      <div className="flex items-center justify-between">
        <span className="font-medium">Z21</span>
        <span className="text-xs text-slate-400">{host}:{port}</span>
      </div>
      <div className="flex items-center gap-2 text-xs text-slate-300">
        <span className={`w-2 h-2 rounded-full ${track.dot}`} aria-hidden="true" />
        {z21.connected ? track.text : 'Not connected'}
      </div>
      {error && <p className="text-xs text-red-300">{error}</p>}
      <button
        onClick={z21.connected ? disconnect : connect}
        disabled={loading}
        className={`w-full py-1.5 rounded-md text-xs font-semibold disabled:opacity-50 ${
          z21.connected ? 'bg-slate-700 hover:bg-slate-600 text-white' : 'bg-green-600 hover:bg-green-500 text-white'
        }`}
      >
        {loading ? 'Working…' : z21.connected ? 'Disconnect' : 'Connect'}
      </button>
    </div>
  )
}

export default function SideNav({ activeTab, onOpen, collapsed, onToggle }) {
  return (
    <nav
      // The whole menu scrolls, including the connection card at its foot, so the card is reachable when the window is short
      className={`${collapsed ? 'w-16' : 'w-60'} flex-shrink-0 self-stretch min-h-0 overflow-y-auto flex flex-col bg-slate-950 border-r border-slate-800 transition-[width] duration-150`}
      aria-label="Main"
    >
      <div className={`flex items-center h-11 px-3 ${collapsed ? 'justify-center' : 'justify-end'}`}>
        <button
          onClick={onToggle}
          title={collapsed ? 'Expand navigation' : 'Collapse navigation'}
          aria-label={collapsed ? 'Expand navigation' : 'Collapse navigation'}
          className="p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800"
        >
          {collapsed ? <ChevronRight className="w-4 h-4" /> : <ChevronLeft className="w-4 h-4" />}
        </button>
      </div>

      <div className="px-2 space-y-4">
        {NAV_GROUPS.map((group) => (
          <div key={group.title}>
            {!collapsed && <p className="px-2 pb-1 text-[11px] uppercase tracking-wider text-slate-500">{group.title}</p>}
            <div className="space-y-0.5">
              {group.items.map((page) => (
                <NavItem key={page.id} page={page} active={activeTab === page.id} collapsed={collapsed} onOpen={onOpen} />
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* Under the menu items, with room below it so it clears the window's bottom edge (where the shelf can cover it) */}
      <div className="p-2 pb-20">
        <ConnectionCard collapsed={collapsed} />
      </div>
    </nav>
  )
}
