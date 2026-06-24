import { expect, test } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Desktop preview screenshots for the macOS / Vision Pro floating-shell layout.
 *
 * These tests boot a real Chromium at 1440x900, navigate to the Vite dev
 * server, and capture the same renderings the Tauri webview would show.
 * They're separate from the unit/integration suites because they assume
 * the dev server is running and they are intentionally non-asserting on
 * the dynamic session/permission state.
 */

const __dirname = dirname(fileURLToPath(import.meta.url));
const VIEWPORT = { width: 1440, height: 900 };
const OUT_DIR = resolve(__dirname, '../screenshots');

test.beforeAll(async () => {
  await mkdir(OUT_DIR, { recursive: true });
});

async function captureShell(
  page: import('@playwright/test').Page,
  name: string,
  inject?: (window: Window) => void,
) {
  await page.goto('/');
  // Wait for the floating activity bar to mount so the screenshot shows
  // the real post-hydration layout.
  await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });
  // Give the activity bar's open-popover animations a moment to settle.
  await page.waitForTimeout(300);
  if (inject) await page.evaluate(inject);
  await page.waitForTimeout(200);
  const path = `${OUT_DIR}/${name}.png`;
  await page.screenshot({ path, fullPage: false });
  return path;
}

test.describe('Desktop shell screenshots', () => {
  test('main workbench (collapsed activity bar)', async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    const path = await captureShell(page, 'desktop-main');
    // Visual regression baseline — 2% tolerance to absorb font/anti-alias variance.
    await expect(page).toHaveScreenshot('desktop-main.png', { maxDiffPixelRatio: 0.02 });
    expect(path).toContain('.png');
  });

  test('composer close-up (Codex-style chip strip + 4 icon buttons)', async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    await page.goto('/');
    await page.waitForSelector('div[role="toolbar"][data-testid]', { timeout: 15_000 }).catch(() => {});
    await page.waitForSelector('[role="form"][aria-label="Prompt composer"]', { timeout: 15_000 });
    // Crop to the bottom area where the composer lives.
    const composer = page.locator('[role="form"][aria-label="Prompt composer"]');
    await composer.scrollIntoViewIfNeeded();
    await page.waitForTimeout(400);
    const path = `${OUT_DIR}/desktop-composer.png`;
    await composer.screenshot({ path });
    // Visual regression for the composer chip strip (5-agent differentiator).
    await expect(composer).toHaveScreenshot('desktop-composer.png', { maxDiffPixelRatio: 0.02 });
    expect(path).toContain('.png');
  });

  test('expanded activity bar (chat tab)', async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    await page.goto('/');
    await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });
    // Click the brand mark to expand the chip strip.
    await page.locator('nav[aria-label="Workbench activity bar"] button[aria-label="Remote Code"]').click();
    await page.waitForTimeout(200);
    await page.screenshot({ path: `${OUT_DIR}/desktop-activitybar-expanded.png` });
    await expect(page).toHaveScreenshot('desktop-activitybar-expanded.png', { maxDiffPixelRatio: 0.02 });
  });

  test('status bar visible (hover state, expanded popovers)', async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    await page.goto('/');
    await page.waitForSelector('div[role="toolbar"][data-tauri-drag-region]', { timeout: 15_000 });
    await page.locator('div[role="toolbar"][data-tauri-drag-region]').first().hover();
    await page.waitForTimeout(300);
    // Click the project segment to expand the popover so the screenshot
    // demonstrates the codex-popover surface.
    const projectChip = page.locator('button[aria-label="项目"]');
    if (await projectChip.count() > 0) {
      await projectChip.first().click({ force: true });
      await page.waitForTimeout(200);
    }
    // Close the project popover first (its fixed inset-0 backdrop blocks
    // subsequent chip clicks), then open the permission popover.
    const projectClose = page.locator('button[aria-label="Close 项目"]');
    if (await projectClose.count() > 0) {
      await projectClose.first().click({ force: true });
      await page.waitForTimeout(200);
    }
    const permissionChip = page.locator('button[aria-label="权限"]');
    if (await permissionChip.count() > 0) {
      await permissionChip.first().click({ force: true });
      await page.waitForTimeout(200);
    }
    await page.screenshot({ path: `${OUT_DIR}/desktop-statusbar-popovers.png` });
    await expect(page).toHaveScreenshot('desktop-statusbar-popovers.png', { maxDiffPixelRatio: 0.02 });
  });

  test('dark theme', async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    // Set the theme before the app boots by injecting into localStorage.
    await page.addInitScript(() => {
      try { window.localStorage.setItem('rc-theme', 'dark'); } catch { /* ignore */ }
    });
    await captureShell(page, 'desktop-dark');
    await expect(page).toHaveScreenshot('desktop-dark.png', { maxDiffPixelRatio: 0.02 });
  });
});
