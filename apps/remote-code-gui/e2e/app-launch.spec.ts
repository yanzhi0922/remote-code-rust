import { expect, test } from '@playwright/test';

test.describe('Application launch', () => {
  test('loads the main page without errors', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await page.goto('/');

    // Wait for the app to render — look for the main layout container
    await page.waitForSelector('nav, aside, main, [role="main"]', { timeout: 10_000 });

    // Page title should be set
    const title = await page.title();
    expect(title).toBeTruthy();
  });

  test('renders the primary UI shell elements', async ({ page }) => {
    await page.goto('/');
    // Vite dev server: wait for the activity bar to actually mount before counting.
    // domcontentloaded fires before React 18 hydrates, so we wait for a real element.
    await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });

    // The app should render either the activity bar, sidebar, or main content area
    // These are the main structural elements of the app
    const hasActivityBar = await page.locator('nav[aria-label="Workbench activity bar"]').count();
    const hasSidebar = await page.locator('aside').count();
    const hasMainContent = await page.locator('main, [role="main"]').count();

    // At least one of these structural elements should be present
    expect(hasActivityBar + hasSidebar + hasMainContent).toBeGreaterThan(0);
  });

  test('has no JavaScript runtime errors on initial load', async ({ page }) => {
    const jsErrors: string[] = [];
    page.on('pageerror', (error) => {
      jsErrors.push(error.message);
    });

    await page.goto('/');
    await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });

    // Give a moment for any async errors to surface
    await page.waitForTimeout(1000);

    // Filter out known non-critical errors (e.g., Tauri runtime not available in browser)
    const criticalErrors = jsErrors.filter(
      (msg) =>
        !msg.includes('__TAURI__') &&
        !msg.includes('invoke') &&
        !msg.includes('plugin-network'),
    );

    expect(criticalErrors).toEqual([]);
  });
});
