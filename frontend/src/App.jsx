import { useEffect, useState } from 'react'
import { useStore } from './store'
import { useWebSocket } from './hooks/useWebSocket'
import Switchboard from './components/Switchboard'
import LocomotivePanel from './components/LocomotivePanel'
import LayoutEditor from './components/LayoutEditor'
import RouteManager from './components/RouteManager'
import SystemSettings from './components/SystemSettings'

function App() {
  const activeTab = useStore((state) => state.activeTab)
  const setActiveTab = useStore((state) => state.setActiveTab)
  const connected = useStore((state) => state.connected)
  const { send } = useWebSocket()

  const tabs = [
    { id: 'switchboard', label: 'Switchboard', icon: '🚂' },
    { id: 'locomotive', label: 'Locomotives', icon: '🚃' },
    { id: 'routes', label: 'Routes', icon: '🛤️' },
    { id: 'editor', label: 'Layout Editor', icon: '✏️' },
    { id: 'settings', label: 'Settings', icon: '⚙️' },
  ]

  const renderContent = () => {
    switch (activeTab) {
      case 'switchboard':
        return <Switchboard />
      case 'locomotive':
        return <LocomotivePanel />
      case 'routes':
        return <RouteManager />
      case 'editor':
        return <LayoutEditor />
      case 'settings':
        return <SystemSettings />
      default:
        return <Switchboard />
    }
  }

  return (
    <div className="flex h-screen bg-slate-900 text-white">
      {/* Sidebar */}
      <div className="w-64 bg-slate-800 border-r border-slate-700 flex flex-col">
        {/* Header */}
        <div className="p-4 border-b border-slate-700">
          <h1 className="text-2xl font-bold">TrainLink</h1>
          <p className="text-sm text-slate-400">Model Railroad Control</p>
        </div>

        {/* Connection status */}
        <div className="p-4 border-b border-slate-700">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`}></div>
            <span className="text-sm">{connected ? 'Connected' : 'Disconnected'}</span>
          </div>
        </div>

        {/* Navigation */}
        <nav className="flex-1 p-4 space-y-2">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`w-full text-left px-4 py-2 rounded transition-colors ${
                activeTab === tab.id
                  ? 'bg-slate-700 text-white'
                  : 'text-slate-300 hover:bg-slate-700'
              }`}
            >
              <span className="mr-2">{tab.icon}</span>
              {tab.label}
            </button>
          ))}
        </nav>

        {/* Footer */}
        <div className="p-4 border-t border-slate-700">
          <p className="text-xs text-slate-500">Version 0.1.0</p>
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top bar */}
        <div className="bg-slate-800 border-b border-slate-700 p-4">
          <h2 className="text-xl font-semibold">
            {tabs.find(t => t.id === activeTab)?.label}
          </h2>
        </div>

        {/* Content area */}
        <div className="flex-1 overflow-auto p-4">
          {renderContent()}
        </div>
      </div>
    </div>
  )
}

export default App
