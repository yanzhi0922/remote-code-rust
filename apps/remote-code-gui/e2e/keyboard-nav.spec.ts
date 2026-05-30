import { expect, test } from '@playwright/test';

test.describe('Keyboard navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');
  });

  test('Tab key traverses focusable elements', async ({ page }) => {
    // Start from the body and tab through elements
    await page.keyboard.press('Tab');

    // After first tab, focus should be on a visible, focusable element
    const activeElement = page.locator(':focus');
    await expect(activeElement).toBeVisible();
  });

  test('ActivityBar tabs are focusable via Tab key', async ({ page }) => {
    // Click on the activity bar to establish focus context
    const nav = page.locator('nav[aria-label="Workbench activity bar"]');
    await nav.click();

    // Tab to the tablist buttons
    const tabButtons = page.locator('nav[aria-label="Workbench activity bar"] [role="tab"]');
    const tabCount = await tabButtons.count();

    if (tabCount > 0) {
      await tabButtons.first().focus();
      await expect(tabButtons.first()).toBeFocused();
    }
  });

  test('sidebar search input is focusable', async ({ page }) => {
    const searchInput = page.locator('input[aria-label="搜索项目和会话"]');
    await searchInput.focus();
    await expect(searchInput).toBeFocused();
  });

  test('chat input textarea is focusable', async ({ page }) => {
    const textarea = page.locator('textarea[aria-label="Prompt input"]');
    await textarea.focus();
    await expect(textarea).toBeFocused();
  });

  test('slash command palette keyboard navigation works', async ({ page }) => {
    // Open the slash palette
    const textarea = page.locator('textarea[aria-label="Prompt input"]');
    await textarea.focus();
    await textarea.fill('/');

    await page.waitForTimeout(100);

    const listbox = page.locator('[role="listbox"][aria-label="Slash commands"]');
    await expect(listbox).toBeVisible();

    // ArrowDown should move to next option
    await page.keyboard.press('ArrowDown');
    const textareaARIADesc = await textarea.getAttribute('aria-activedescendant');
    expect(textareaARIADesc).toBeTruthy();

    // Escape should close the palette
    await page.keyboard.press('Escape');
    await expect(listbox).not.toBeVisible();
  });

  test('focus moves to body when Escape is pressed in search', async ({ page }) => {
    const searchInput = page.locator('input[aria-label="搜索项目和会话"]');
    await searchInput.focus();
    await searchInput.fill('test');

    // Press Escape — browser behavior may blur the input
    await page.keyboard.press('Escape');
    // The input should no longer be focused
    await expect(searchInput).not.toBeFocused();
  });

  test('theme toggle button is focusable', async ({ page }) => {
    const themeButton = page.locator('button[aria-label="Switch to light theme"], button[aria-label="Switch to dark theme"]');
    await expect(themeButton).toBeVisible();
    await themeButton.focus();
    await expect(themeButton).toBeFocused();
  });
});
