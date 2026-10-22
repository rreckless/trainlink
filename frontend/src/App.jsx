import { useState } from 'react'
import { useStore } from './store'
import { useZ21StatusPolling } from './hooks/useZ21Status'
import TopBar from './components/shell/TopBar'
import SideNav from './components/shell/SideNav'
import { PAGES } from './components/shell/navigation'
import Switchboard from './components/Switchboard'
import ControlView from './components/control/ControlView'
import LocomotiveSetup from './components/locomotives/LocomotiveSetup'
import LayoutEditor from './components/LayoutEditor'
import RouteManager from './components/RouteManager'
import SystemSettings from './components/SystemSettings'

const PAGE_VIEWS = {
  switchboard: Switchboard,
  control: ControlView,
  locomotives: LocomotiveSetup,
  routes: RouteManager,
  editor: LayoutEditor,
  settings: SystemSettings,
}

function App() {
  const activeTab = useStore((state) => state.activeTab)
  const setActiveTab = useStore((state) => state.setActiveTab)
  const [collapsed, setCollapsed] = useState(false)
  useZ21StatusPolling()

  const page = PAGES.find((item) => item.id === activeTab) ?? PAGES[0]
  const View = PAGE_VIEWS[page.id] ?? Switchboard

  return (
    <div className="h-screen flex flex-col bg-slate-900 text-white">
      <TopBar onOpen={setActiveTab} />
      <div className="flex flex-1 min-h-0">
        <SideNav activeTab={page.id} onOpen={setActiveTab} collapsed={collapsed} onToggle={() => setCollapsed(!collapsed)} />
        <main className="flex-1 min-w-0 flex flex-col">
          <div className="px-6 py-4 border-b border-slate-800">
            <h2 className="text-xl font-semibold">{page.label}</h2>
            <p className="text-sm text-slate-400">{page.subtitle}</p>
          </div>
          <div className="flex-1 overflow-auto p-4">
            <View />
          </div>
        </main>
      </div>
    </div>
  )
}

export default App
