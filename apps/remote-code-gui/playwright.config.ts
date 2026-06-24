import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E test configuration for remote-code-gui.
 *
 * These tests require a running dev server (`npm run dev` or `tauri dev`).
 * The dev server should be started before running: `npx playwright test`
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    locale: 'zh-CN',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npx vite --port 1420 --strictPort',
    url: 'http://localhost:1420',
    reuseExistingServer: true,
    timeout: 30_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
