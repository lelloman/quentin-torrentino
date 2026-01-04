import { test, expect } from '@playwright/test';

test.describe('Search Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/search');
  });

  test('displays search interface', async ({ page }) => {
    await page.waitForTimeout(500);

    // Should have some search-related UI
    const hasSearchInput = await page.locator('input[type="text"], input[type="search"]').count() > 0;
    const hasSearchButton = await page.locator('button').filter({ hasText: /search/i }).count() > 0;
    const hasSearchHeading = await page.locator('text=Search').count() > 0;

    expect(hasSearchInput || hasSearchButton || hasSearchHeading).toBe(true);
  });
});

test.describe('Torrents Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/torrents');
  });

  test('displays torrents interface', async ({ page }) => {
    await page.waitForTimeout(500);

    // Should show torrents-related content
    const pageContent = await page.content();
    expect(
      pageContent.includes('Torrent') ||
      pageContent.includes('torrent') ||
      pageContent.includes('Download')
    ).toBe(true);
  });
});

test.describe('Pipeline Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/pipeline');
  });

  test('displays pipeline interface', async ({ page }) => {
    await page.waitForTimeout(500);

    // Should show pipeline-related content
    const pageContent = await page.content();
    expect(
      pageContent.includes('Pipeline') ||
      pageContent.includes('pipeline') ||
      pageContent.includes('Processing') ||
      pageContent.includes('Queue')
    ).toBe(true);
  });
});

test.describe('TextBrain Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/textbrain');
  });

  test('displays textbrain interface', async ({ page }) => {
    await page.waitForTimeout(500);

    // Should show textbrain-related content
    const pageContent = await page.content();
    expect(
      pageContent.includes('TextBrain') ||
      pageContent.includes('LLM') ||
      pageContent.includes('Matcher') ||
      pageContent.includes('Query')
    ).toBe(true);
  });
});

test.describe('Settings Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/settings');
  });

  test('displays settings interface', async ({ page }) => {
    await page.waitForTimeout(500);

    // Should show settings content
    const pageContent = await page.content();
    expect(
      pageContent.includes('Settings') ||
      pageContent.includes('settings') ||
      pageContent.includes('Configuration')
    ).toBe(true);
  });
});
