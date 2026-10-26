import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { defineConfig } from '@playwright/test'

// The browser tests run against the real core: the same Rust code as the desktop app, served over HTTP with the built
// interface. Each run gets a new data folder, so no test sees a database from an earlier run
const port = 8765
const data = mkdtempSync(join(tmpdir(), 'trainlink-e2e-'))

export default defineConfig({
  testDir: 'e2e',
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    headless: true,
  },
  webServer: {
    command: `node e2e/prepare-session.mjs ${data} && cargo run --manifest-path src-tauri/Cargo.toml --bin server -- --port ${port} --data ${data} --web dist`,
    url: `http://127.0.0.1:${port}/health`,
    reuseExistingServer: false,
    timeout: 600_000,
  },
})
