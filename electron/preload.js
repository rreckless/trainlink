const { contextBridge, ipcMain } = require('electron')

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld(
  'api',
  {
    send: (channel, data) => {
      // allowlist channels
      let validChannels = ['toMain']
      if (validChannels.includes(channel)) {
        ipcMain.send(channel, data)
      }
    },
    receive: (channel, func) => {
      let validChannels = ['fromMain']
      if (validChannels.includes(channel)) {
        // Deliberately strip event as it includes `sender`
        ipcMain.on(channel, (event, ...args) => func(...args))
      }
    },
  }
)
