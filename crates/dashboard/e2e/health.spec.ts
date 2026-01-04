import { test, expect } from '@playwright/test';

test.describe('Health Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/health');
  });

  test('displays health page', async ({ page }) => {
    // Wait for the API call to complete
    await page.waitForTimeout(1000);

    // Page should have loaded
    const bodyContent = await page.locator('body').textContent();
    expect(bodyContent).toBeTruthy();
  });
});

test.describe('Config Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/config');
  });

  test('displays config page', async ({ page }) => {
    await page.waitForTimeout(1000);

    // Page should have loaded
    const bodyContent = await page.locator('body').textContent();
    expect(bodyContent).toBeTruthy();
  });
});
