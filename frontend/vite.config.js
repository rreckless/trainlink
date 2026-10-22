import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// `tauri ios dev --host` sets TAURI_DEV_HOST, so a physical device can reach the dev server. Otherwise only this computer can
const tauriHost = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [react()],
  server: {
    host: tauriHost || false,
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, '')
      },
      '/ws': {
        target: 'ws://localhost:8000',
        ws: true
      }
    }
  },
  build: {
    outDir: 'dist',
    sourcemap: true
  }
})
