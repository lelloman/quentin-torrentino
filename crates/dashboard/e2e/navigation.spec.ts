import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test('loads dashboard page', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
  });

  test('shows system status card', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('text=System Status')).toBeVisible();
  });

  test('navigates to tickets page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Tickets');
    await expect(page).toHaveURL('/tickets');
    await expect(page.getByRole('heading', { name: 'Tickets' })).toBeVisible();
  });

  test('navigates to health page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Health');
    await expect(page).toHaveURL('/health');
  });

  test('navigates to config page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Config');
    await expect(page).toHaveURL('/config');
  });

  test('navigates to audit page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Audit');
    await expect(page).toHaveURL('/audit');
  });

  test('navigates to search page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Search');
    await expect(page).toHaveURL('/search');
  });

  test('navigates to torrents page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Torrents');
    await expect(page).toHaveURL('/torrents');
  });

  test('navigates to pipeline page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Pipeline');
    await expect(page).toHaveURL('/pipeline');
  });

  test('navigates to textbrain page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=TextBrain');
    await expect(page).toHaveURL('/textbrain');
  });

  test('navigates to settings page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Settings');
    await expect(page).toHaveURL('/settings');
  });

  test('view all link navigates to tickets', async ({ page }) => {
    await page.goto('/');
    await page.click('text=View all');
    await expect(page).toHaveURL('/tickets');
  });
});
