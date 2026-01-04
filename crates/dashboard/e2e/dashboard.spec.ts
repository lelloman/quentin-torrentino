import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('displays stats cards', async ({ page }) => {
    // Wait for page to load
    await expect(page.getByRole('heading', { name: 'Dashboard', exact: true })).toBeVisible();
    await expect(page.getByText('System Status')).toBeVisible();
    await expect(page.getByText('Total Tickets')).toBeVisible();
  });

  test('displays orchestrator status section', async ({ page }) => {
    // OrchestratorStatus component should be visible
    await expect(page.getByText('Orchestrator').first()).toBeVisible();
  });

  test('displays recent tickets section', async ({ page }) => {
    await expect(page.getByText('Recent Tickets')).toBeVisible();
  });

  test('shows tickets section content', async ({ page }) => {
    // Wait for tickets to load - either shows "No tickets yet" or ticket cards
    await page.waitForTimeout(500);
    const recentSection = page.getByText('Recent Tickets');
    await expect(recentSection).toBeVisible();
  });

  test('health status loads', async ({ page }) => {
    // Wait for health check to complete
    await page.waitForTimeout(1000);

    // System Status card should be visible
    await expect(page.getByText('System Status')).toBeVisible();
  });
});
