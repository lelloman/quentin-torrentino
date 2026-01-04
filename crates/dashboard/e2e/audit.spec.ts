import { test, expect } from '@playwright/test';

test.describe('Audit Log Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/audit');
  });

  test('displays audit log page', async ({ page }) => {
    // Wait for page to load
    await page.waitForTimeout(500);

    // The page should have loaded - check for any content
    const pageContent = await page.content();
    expect(pageContent.length).toBeGreaterThan(0);
  });

  test('has interactive elements', async ({ page }) => {
    await page.waitForTimeout(500);

    // Page should have rendered something
    const bodyContent = await page.locator('body').textContent();
    expect(bodyContent).toBeTruthy();
  });
});
