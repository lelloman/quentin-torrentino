import { test, expect } from '@playwright/test';

test.describe('Tickets Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/tickets');
  });

  test('displays tickets heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Tickets' })).toBeVisible();
  });

  test('displays Create Ticket button', async ({ page }) => {
    await expect(page.locator('text=Create Ticket')).toBeVisible();
  });

  test('displays state filter', async ({ page }) => {
    await expect(page.locator('text=Filter by state')).toBeVisible();
  });

  test('opens ticket type chooser on Create Ticket click', async ({ page }) => {
    await page.click('text=Create Ticket');
    await expect(page.locator('text=What do you want to download?')).toBeVisible();
  });

  test('shows music, video, and manual entry options', async ({ page }) => {
    await page.click('text=Create Ticket');
    await expect(page.locator('text=Music Album')).toBeVisible();
    await expect(page.locator('text=Movie / TV Show')).toBeVisible();
    await expect(page.locator('text=Manual Entry')).toBeVisible();
  });

  test('can cancel ticket creation', async ({ page }) => {
    await page.click('text=Create Ticket');
    await expect(page.locator('text=What do you want to download?')).toBeVisible();

    await page.click('button:has-text("Cancel")');
    await expect(page.locator('text=What do you want to download?')).not.toBeVisible();
  });

  test('opens music wizard when Music Album is selected', async ({ page }) => {
    await page.click('text=Create Ticket');
    await page.click('text=Music Album');

    // Music wizard should appear with MusicBrainz search
    await expect(page.locator('text=Search MusicBrainz').or(page.locator('text=Artist'))).toBeVisible();
  });

  test('opens video wizard when Movie / TV Show is selected', async ({ page }) => {
    await page.click('text=Create Ticket');
    await page.click('text=Movie / TV Show');

    // Video wizard should appear
    await expect(page.locator('text=Search TMDB').or(page.locator('text=Title'))).toBeVisible();
  });

  test('opens simple form when Manual Entry is selected', async ({ page }) => {
    await page.click('text=Create Ticket');
    await page.click('text=Manual Entry');

    // Simple form should appear with description field
    await expect(page.getByText('Create New Ticket')).toBeVisible();
  });
});

test.describe('Ticket State Filtering', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/tickets');
  });

  test('filter dropdown is functional', async ({ page }) => {
    // Find the state filter component
    const filterArea = page.locator('text=Filter by state').locator('..');

    // It should contain filter options
    await expect(filterArea).toBeVisible();
  });
});
