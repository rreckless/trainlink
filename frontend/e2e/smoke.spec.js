import { test, expect } from '@playwright/test'

test('the side menu groups its pages as LocoLink does, with the less common pages folded away', async ({ page }) => {
  await page.goto('/')
  for (const section of ['Basics', 'Advanced', 'More advanced features']) {
    await expect(page.getByRole('button', { name: section, exact: false }).first()).toBeVisible()
  }
  for (const name of ['Control', 'Turnouts', 'Blocks', 'Health', 'Settings']) {
    await expect(page.getByRole('button', { name, exact: true }).first()).toBeVisible()
  }
  // The app opens on Switchboard, which sits under More basic features, so that group opens by itself. Toggling it closes it
  const menu = page.getByRole('navigation', { name: 'Main' })
  await expect(menu.getByRole('button', { name: 'Consists', exact: true })).toBeVisible()
  await menu.getByRole('button', { name: /More basic features/ }).click()
  await expect(menu.getByRole('button', { name: 'Consists', exact: true })).toHaveCount(0)
})

test('the health page opens and reports on the project', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Health', exact: true }).first().click()
  await expect(page.getByText('Project health')).toBeVisible()
  await expect(page.getByText('Nothing is missing')).toBeVisible()
})

test('a consist made from two locomotives appears in the consist list', async ({ page }) => {
  for (const [name, address] of [['Class 37', 3], ['Class 08', 4]]) {
    const response = await page.request.post('/api/locomotives', {
      data: { name, dcc_address: address, locomotive_type: 'diesel' },
    })
    expect(response.status()).toBe(201)
  }

  await page.goto('/')
  await page.getByRole('button', { name: 'Consists', exact: true }).first().click()
  await page.getByRole('button', { name: 'New consist' }).click()
  await page.getByLabel('Class 37').check()
  await page.getByLabel('Class 08').check()
  await page.getByPlaceholder('Consist name').fill('Pair')
  await page.getByRole('button', { name: 'Create consist' }).click()

  await expect(page.getByText('Created Pair')).toBeVisible()
  await expect(page.getByLabel('Consist address for Pair')).toHaveValue('1')
})

test('the switchboard lists a block with its live state and a signal with its aspect', async ({ page }) => {
  const layouts = await page.request.post('/api/layouts', {
    data: { name: 'Switchboard test', width: 800, height: 600, scale: 'N' },
  })
  expect(layouts.status()).toBe(201)
  const layoutId = (await layouts.json()).id
  const block = await (await page.request.post('/api/blocks', {
    data: { layout_id: layoutId, name: 'Platform 9', x: 0, y: 0 },
  })).json()
  await page.request.post('/api/signals', {
    data: { layout_id: layoutId, name: 'Home 9', x: 5, y: 5, signal_type: 'main' },
  })
  const signals = await (await page.request.get('/api/signals')).json()
  const signal = signals.find((item) => item.name === 'Home 9')
  await page.request.patch(`/api/signals/${signal.id}`, { data: { block_ids: [block.id] } })

  await page.goto('/')
  await page.getByRole('button', { name: 'Switchboard', exact: true }).first().click()
  await expect(page.getByText('Platform 9')).toBeVisible()
  await expect(page.getByText('Home 9')).toBeVisible()
  await expect(page.getByText('go', { exact: true })).toBeVisible()
})

test('a block can be linked to its next block and a signal on the Blocks page', async ({ page }) => {
  const layout = await (await page.request.post('/api/layouts', {
    data: { name: 'Blocks test', width: 800, height: 600, scale: 'N' },
  })).json()
  const first = await (await page.request.post('/api/blocks', {
    data: { layout_id: layout.id, name: 'Entry block', x: 0, y: 0 },
  })).json()
  const second = await (await page.request.post('/api/blocks', {
    data: { layout_id: layout.id, name: 'Exit block', x: 10, y: 0 },
  })).json()
  await page.request.post('/api/signals', {
    data: { layout_id: layout.id, name: 'Entry signal', x: 1, y: 1, signal_type: 'main' },
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Blocks', exact: true }).first().click()
  await page.getByRole('button', { name: /Entry block/ }).click()
  await page.getByLabel('Next block').selectOption({ label: 'Exit block' })
  await page.getByLabel('Entry signal').check()
  await page.getByRole('button', { name: 'Save block' }).click()

  await expect(page.getByText('Saved Entry block')).toBeVisible()
  const saved = await (await page.request.get(`/api/blocks/${first.id}`)).json()
  expect(saved.next_block_id).toBe(second.id)
  expect(saved.signal_ids.length).toBe(1)
})

test('a route is built from blocks in order on the Routes page', async ({ page }) => {
  const layout = await (await page.request.post('/api/layouts', {
    data: { name: 'Routes test', width: 800, height: 600, scale: 'N' },
  })).json()
  const first = await (await page.request.post('/api/blocks', {
    data: { layout_id: layout.id, name: 'Route A', x: 0, y: 0 },
  })).json()
  const second = await (await page.request.post('/api/blocks', {
    data: { layout_id: layout.id, name: 'Route B', x: 5, y: 0 },
  })).json()
  await page.request.patch(`/api/blocks/${first.id}`, { data: { next_block_id: second.id } })

  await page.goto('/')
  await page.getByRole('button', { name: 'Routes', exact: true }).first().click()
  await page.getByRole('combobox').nth(0).selectOption({ label: 'Routes test' })
  await page.getByRole('button', { name: 'New route' }).click()
  await page.getByLabel('Name').fill('Up')
  await page.getByRole('combobox').nth(1).selectOption({ label: 'Route A' })
  await page.getByRole('button', { name: 'Add block' }).click()
  await page.getByRole('combobox').nth(1).selectOption({ label: 'Route B' })
  await page.getByRole('button', { name: 'Add block' }).click()
  await page.getByRole('button', { name: 'Next' }).click()
  await page.getByRole('button', { name: 'Next' }).click()
  await page.getByRole('button', { name: 'Save route' }).click()

  await expect(page.getByText('Created Up')).toBeVisible()
  await expect(page.getByRole('button', { name: /Up\s+2 blocks/ })).toBeVisible()
  await expect(page.getByText('not linked')).toHaveCount(0)
})

test('the decoder CVs page offers the CVs of the NMRA profile and keeps Write off until the write is confirmed', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Decoder CVs', exact: true }).first().click()
  await expect(page.getByRole('heading', { name: 'Decoder profile' })).toBeVisible()
  await expect(page.getByRole('combobox').first()).toHaveValue(/.+/)
  await expect(page.getByRole('button', { name: /Short address/ })).toBeVisible()
  const write = page.getByRole('button', { name: 'Write CV' })
  await expect(write).toBeDisabled()
  await page.getByLabel('New value (0 to 255)').fill('6')
  await expect(write).toBeDisabled()
  await page.getByLabel('I have checked the CV number and the value').check()
  await expect(write).toBeEnabled()
})
