// Row-level security through the real Supabase API (run after the migration is applied to the project).
// Needs two test users created in Authentication (Add user, with auto-confirm), and these environment variables:
//   SUPABASE_URL, SUPABASE_ANON_KEY (the public anon key, never the secret key)
//   TEST_A_EMAIL, TEST_A_PASSWORD, TEST_B_EMAIL, TEST_B_PASSWORD
// Run: node --test supabase/tests/rls_http_test.mjs
import { test } from 'node:test'
import assert from 'node:assert/strict'

const REQUIRED = ['SUPABASE_URL', 'SUPABASE_ANON_KEY', 'TEST_A_EMAIL', 'TEST_A_PASSWORD', 'TEST_B_EMAIL', 'TEST_B_PASSWORD']
const missing = REQUIRED.filter((name) => !process.env[name])
if (missing.length > 0) {
  throw new Error(`Set these before running the tests: ${missing.join(', ')}`)
}
const env = (name) => process.env[name]
const URL_BASE = env('SUPABASE_URL').replace(/\/$/, '')
const ANON = env('SUPABASE_ANON_KEY')

async function signIn(email, password) {
  const response = await fetch(`${URL_BASE}/auth/v1/token?grant_type=password`, {
    method: 'POST',
    headers: { apikey: ANON, 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  assert.equal(response.status, 200, `sign-in failed for ${email}`)
  const data = await response.json()
  return { token: data.access_token, userId: data.user.id }
}

const rest = (token, path, init = {}) =>
  fetch(`${URL_BASE}/rest/v1/${path}`, {
    ...init,
    headers: {
      apikey: ANON,
      Authorization: `Bearer ${token ?? ANON}`,
      'Content-Type': 'application/json',
      Prefer: 'return=representation',
      ...init.headers,
    },
  })

const stamp = Date.now()
let a
let b

test.before(async () => {
  a = await signIn(env('TEST_A_EMAIL'), env('TEST_A_PASSWORD'))
  b = await signIn(env('TEST_B_EMAIL'), env('TEST_B_PASSWORD'))
})

test('A creates a layout and sees it', async () => {
  const created = await rest(a.token, 'layouts', {
    method: 'POST',
    body: JSON.stringify({ name: `RLS test ${stamp}`, width: 800, height: 400, scale: 'N' }),
  })
  assert.equal(created.status, 201)
  const [row] = await created.json()
  assert.equal(row.user_id, a.userId, 'user_id is filled in from the signed-in user')
  const listed = await (await rest(a.token, `layouts?name=eq.RLS%20test%20${stamp}`)).json()
  assert.equal(listed.length, 1)
  globalThis.layoutId = row.id
})

test('B cannot see A\'s layout', async () => {
  const listed = await (await rest(b.token, `layouts?id=eq.${globalThis.layoutId}`)).json()
  assert.deepEqual(listed, [])
})

test('B cannot change or delete A\'s layout', async () => {
  const updated = await (await rest(b.token, `layouts?id=eq.${globalThis.layoutId}`, {
    method: 'PATCH',
    body: JSON.stringify({ name: 'Taken' }),
  })).json()
  assert.deepEqual(updated, [])
  const deleted = await (await rest(b.token, `layouts?id=eq.${globalThis.layoutId}`, { method: 'DELETE' })).json()
  assert.deepEqual(deleted, [])
})

test('B cannot create a layout as A', async () => {
  const response = await rest(b.token, 'layouts', {
    method: 'POST',
    body: JSON.stringify({ user_id: a.userId, name: `Stolen ${stamp}`, width: 1, height: 1, scale: 'N' }),
  })
  assert.ok(response.status >= 400, `expected a refusal, got ${response.status}`)
})

test('anonymous visitors see nothing', async () => {
  const response = await rest(null, 'layouts')
  const body = await response.text()
  assert.ok(response.status >= 400 || body === '[]', `anonymous read returned ${response.status} ${body}`)
})

test('B cannot read or write A\'s pictures', async () => {
  const path = `${a.userId}/rls-test-${stamp}.jpg`
  const uploaded = await fetch(`${URL_BASE}/storage/v1/object/locomotive-pictures/${path}`, {
    method: 'POST',
    headers: { apikey: ANON, Authorization: `Bearer ${a.token}`, 'Content-Type': 'image/jpeg' },
    body: new Uint8Array([0xff, 0xd8, 0xff, 0xe0]),
  })
  assert.equal(uploaded.status, 200, 'A can upload to their own folder')
  const read = await fetch(`${URL_BASE}/storage/v1/object/locomotive-pictures/${path}`, {
    headers: { apikey: ANON, Authorization: `Bearer ${b.token}` },
  })
  assert.ok(read.status >= 400, `B read A's picture with ${read.status}`)
  const write = await fetch(`${URL_BASE}/storage/v1/object/locomotive-pictures/${path}`, {
    method: 'PUT',
    headers: { apikey: ANON, Authorization: `Bearer ${b.token}`, 'Content-Type': 'image/jpeg' },
    body: new Uint8Array([0xff, 0xd8, 0xff, 0xe0]),
  })
  assert.ok(write.status >= 400, `B overwrote A's picture with ${write.status}`)
  await fetch(`${URL_BASE}/storage/v1/object/locomotive-pictures/${path}`, {
    method: 'DELETE',
    headers: { apikey: ANON, Authorization: `Bearer ${a.token}` },
  })
})

test('clean up A\'s test layout', async () => {
  await rest(a.token, `layouts?id=eq.${globalThis.layoutId}`, { method: 'DELETE' })
})
