import { expect, test } from '@playwright/test';

test.describe('Accessibility', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');
  });

  test('ActivityBar exposes a tablist with explicit orientation', async ({ page }) => {
    // Wait for React to mount before counting.
    await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });
    // Click the ActivityBar header to expand the tablist (collapsed by default).
    await page.locator('nav[aria-label="Workbench activity bar"] button[aria-label="Remote Code"]').click();
    await page.waitForTimeout(150);
    const tablist = page.locator('nav[aria-label="Workbench activity bar"] [role="tablist"]');
    await expect(tablist.first()).toBeVisible();
    // ActivityBar uses a horizontal floating chip strip in the macOS shell.
    const orientation = await tablist.first().getAttribute('aria-orientation');
    expect(['horizontal', 'vertical']).toContain(orientation);
  });

  test('ActivityBar tab buttons have correct ARIA roles', async ({ page }) => {
    await page.waitForSelector('nav[aria-label="Workbench activity bar"]', { timeout: 15_000 });
    await page.locator('nav[aria-label="Workbench activity bar"] button[aria-label="Remote Code"]').click();
    await page.waitForTimeout(150);
    const tabs = page.locator('nav[aria-label="Workbench activity bar"] [role="tab"]');
    const tabCount = await tabs.count();

    expect(tabCount).toBeGreaterThanOrEqual(2);

    for (let i = 0; i < tabCount; i++) {
      const ariaSelected = await tabs.nth(i).getAttribute('aria-selected');
      expect(ariaSelected).not.toBeNull();
    }
  });

  test('ActivityBar has navigation landmark with label', async ({ page }) => {
    const nav = page.locator('nav[aria-label="Workbench activity bar"]');
    await expect(nav).toBeVisible();
  });

  test('ChatInput has form with proper aria-label', async ({ page }) => {
    // Wait for the chat input to render
    const form = page.locator('[role="form"][aria-label="Prompt composer"]');
    await expect(form).toBeVisible();
  });

  test('ChatInput textarea has accessible label', async ({ page }) => {
    const textarea = page.locator('textarea[aria-label="Prompt input"]');
    await expect(textarea).toBeVisible();
  });

  test('ChatInput slash command palette uses listbox semantics', async ({ page }) => {
    // Type a slash to trigger the command palette
    const textarea = page.locator('textarea[aria-label="Prompt input"]');
    await textarea.fill('/');
    await page.waitForTimeout(100);

    const listbox = page.locator('[role="listbox"][aria-label="Slash commands"]');
    await expect(listbox).toBeVisible();

    // Options should have role="option" and aria-selected
    const options = listbox.locator('[role="option"]');
    const optionCount = await options.count();
    expect(optionCount).toBeGreaterThan(0);

    // First option should be selected
    const firstSelected = await options.first().getAttribute('aria-selected');
    expect(firstSelected).toBe('true');
  });

  test('sidebar search has accessible label', async ({ page }) => {
    const searchInput = page.locator('input[aria-label="搜索项目和会话"]');
    await expect(searchInput).toBeVisible();
  });

  test('sidebar buttons have descriptive aria-labels', async ({ page }) => {
    const newSessionBtn = page.locator('button[aria-label="新会话"]');
    await expect(newSessionBtn).toBeVisible();

    const addProjectBtn = page.locator('button[aria-label="添加项目"]');
    await expect(addProjectBtn).toBeVisible();
  });
});
