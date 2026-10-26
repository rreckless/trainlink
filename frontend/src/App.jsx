import { useCallback, useEffect, useState } from 'react'
import { useStore } from './store'
import { useZ21StatusPolling } from './hooks/useZ21Status'
import { useSyncedSettings } from './hooks/useSyncedSettings'
import { useGoogleReturn } from './hooks/useGoogleReturn'
import TopBar from './components/shell/TopBar'
import SideNav from './components/shell/SideNav'
import { PAGES } from './components/shell/navigation'
import Switchboard from './components/Switchboard'
import ControlView from './components/control/ControlView'
import LocomotiveSetup from './components/locomotives/LocomotiveSetup'
import ConsistsView from './components/consists/ConsistsView'
import TurnoutsView from './components/turnouts/TurnoutsView'
import BlocksView from './components/blocks/BlocksView'
import HealthView from './components/health/HealthView'
import SetupView from './components/setup/SetupView'
import ProgrammingView from './components/programming/ProgrammingView'
import LayoutEditor from './components/LayoutEditor'
import RouteManager from './components/RouteManager'
import SystemSettings from './components/SystemSettings'
import SignIn from './components/auth/SignIn'
import { authApi } from './services/authApi'

const PAGE_VIEWS = {
  switchboard: Switchboard,
  control: ControlView,
  locomotives: LocomotiveSetup,
  consists: ConsistsView,
  blocks: BlocksView,
  turnouts: TurnoutsView,
  health: HealthView,
  setup: SetupView,
  decoder: ProgrammingView,
  routes: RouteManager,
  editor: LayoutEditor,
  settings: SystemSettings,
}

function App() {
  const activeTab = useStore((state) => state.activeTab)
  const setActiveTab = useStore((state) => state.setActiveTab)
  const [collapsed, setCollapsed] = useState(false)
  const [account, setAccount] = useState({ loading: true, signedIn: false, email: '' })
  const [returnError, setReturnError] = useState('')
  useZ21StatusPolling()
  useSyncedSettings(account.signedIn)

  const loadAccount = useCallback(async () => {
    try {
      const data = await authApi.getSession()
      setAccount({ loading: false, signedIn: data.signed_in, email: data.email ?? '' })
    } catch {
      setAccount({ loading: false, signedIn: false, email: '' })
    }
  }, [])

  useEffect(() => {
    loadAccount()
  }, [loadAccount])

  // Google returns to the Android app through its link, which finishes the sign-in
  useGoogleReturn(loadAccount, setReturnError)

  const signOut = async () => {
    await authApi.signOut()
    setAccount({ loading: false, signedIn: false, email: '' })
  }

  if (account.loading) {
    return <div className="h-screen bg-slate-900" />
  }
  if (!account.signedIn) {
    return <SignIn onSignedIn={loadAccount} initialError={returnError} />
  }

  const page = PAGES.find((item) => item.id === activeTab) ?? PAGES[0]
  const View = PAGE_VIEWS[page.id] ?? Switchboard

  return (
    <div className="h-screen flex flex-col bg-slate-900 text-white">
      <TopBar onOpen={setActiveTab} email={account.email} onSignOut={signOut} />
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
