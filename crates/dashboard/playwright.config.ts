import { defineConfig, devices } from '@playwright/test';

// Set to true to also start the Rust backend during tests
const startBackend = process.env.E2E_START_BACKEND === 'true' || process.env.CI;

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? 'github' : 'html',
  timeout: 30000,

  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: [
    // Always start Vite dev server
    {
      command: 'npm run dev',
      url: 'http://localhost:5173',
      reuseExistingServer: !process.env.CI,
      timeout: 30000,
      env: {
        ...process.env,
        // Point Vite proxy to test backend when starting backend
        ...(startBackend ? { API_URL: 'http://localhost:18080' } : {}),
      },
    },
    // Optionally start Rust backend (CI or E2E_START_BACKEND=true)
    ...(startBackend ? [{
      command: 'cargo run --manifest-path ../../Cargo.toml -p torrentino-server',
      url: 'http://localhost:18080/api/v1/health',
      reuseExistingServer: !process.env.CI,
      timeout: 120000,
      env: {
        ...process.env,
        RUST_LOG: 'info',
        QUENTIN_CONFIG: '../../config.test.toml',
      },
    }] : []),
  ],
});
