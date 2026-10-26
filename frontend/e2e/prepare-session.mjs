// Writes a signed-in session into a test data folder, so the browser tests open the app without the real sign-in
// service. The token is not valid anywhere else, and its expiry is far enough away that the core never refreshes it.
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const folder = process.argv[2]
if (!folder) {
  console.error('usage: node e2e/prepare-session.mjs <data folder>')
  process.exit(1)
}

const tenYearsFromNow = Math.floor(Date.now() / 1000) + 10 * 365 * 24 * 60 * 60
mkdirSync(folder, { recursive: true })
writeFileSync(
  join(folder, 'session.json'),
  JSON.stringify({
    access_token: 'e2e-access-token',
    refresh_token: 'e2e-refresh-token',
    expires_at: tenYearsFromNow,
    user_id: 'e2e-user',
    email: 'e2e@example.com',
  })
)
