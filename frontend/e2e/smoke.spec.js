import { test, expect } from '@playwright/test'

test('the side menu lists the pages under Drive, Layout and System', async ({ page }) => {
  await page.goto('/')
  for (const name of ['Control', 'Consists', 'Turnouts', 'Health', 'Settings']) {
    await expect(page.getByRole('button', { name, exact: true }).first()).toBeVisible()
  }
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
  await page.getByLabel('Class 37').check()
  await page.getByLabel('Class 08').check()
  await page.getByPlaceholder('Consist name').fill('Pair')
  await page.getByRole('button', { name: 'Create consist' }).click()

  await expect(page.getByText('Created Pair')).toBeVisible()
  await expect(page.getByLabel('Consist address for Pair')).toHaveValue('1')
})
