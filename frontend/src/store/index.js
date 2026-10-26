import { create } from 'zustand'

export const useStore = create((set) => ({
  // Layout state
  layout: null,
  layouts: [],

  // Locomotives
  locomotives: [],
  selectedLocomotive: null,

  // Blocks and signals
  blocks: [],
  signals: [],
  feedbacks: [],

  // Routes
  routes: [],
  selectedRoute: null,

  // UI state
  activeTab: 'switchboard',
  editMode: false,
  selectedTool: null,

  // Connection (browser to TrainLink server)
  connected: false,
  clientId: null,

  // Z21 track state, polled from /api/z21/status
  // state: disconnected | lost | unknown | short_circuit | track_power_off | ready
  z21: { connected: false, state: 'disconnected', lastHeartbeat: null, xbusVersion: null },
  // Z21 address used by Settings and the Control page
  z21Host: '10.0.0.111',
  z21Port: 21105,

  // Actions
  setLayout: (layout) => set({ layout }),
  setLocomotives: (locomotives) => set({ locomotives }),
  setBlocks: (blocks) => set({ blocks }),
  setSignals: (signals) => set({ signals }),
  setFeedbacks: (feedbacks) => set({ feedbacks }),
  setRoutes: (routes) => set({ routes }),
  setActiveTab: (tab) => set({ activeTab: tab }),
  setEditMode: (mode) => set({ editMode: mode }),
  setConnected: (connected) => set({ connected }),
  setClientId: (id) => set({ clientId: id }),
  setZ21Status: (z21) => set({ z21 }),
  setZ21Address: (host, port) => set({ z21Host: host, z21Port: port }),
  updateLocomotiveSpeed: (locId, speed) => set((state) => ({
    locomotives: state.locomotives.map(loc =>
      loc.id === locId ? { ...loc, speed } : loc
    )
  })),
  updateLocomotiveDirection: (locId, direction) => set((state) => ({
    locomotives: state.locomotives.map(loc =>
      loc.id === locId ? { ...loc, direction } : loc
    )
  })),
}))
