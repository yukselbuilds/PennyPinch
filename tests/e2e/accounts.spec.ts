import { test, expect, Page } from '@playwright/test';
import { build } from 'vite';
import react from '@vitejs/plugin-react';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const root = process.cwd(), run = path.join(root, 'work/proofs/accounts/ui', crypto.randomUUID());
fs.mkdirSync(run, { recursive: true });
const inputs = ['src/features/integrations/Accounts.tsx', 'src/features/integrations/accounts.css', 'src/lib/accounts.ts', 'src/styles.css',
  'tests/fixtures/accounts.tsx', 'tests/fixtures/accountsTransport.ts', 'tests/e2e/accounts.spec.ts', 'tests/e2e/playwright.config.ts', 'package.json', 'pnpm-lock.yaml'];
const fingerprint = () => Object.fromEntries(inputs.map(p => [p, crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex')]));
const before = fingerprint(), outcomes: unknown[] = [], audits: unknown[] = [];
test.beforeAll(async () => {
  await build({ configFile: false, logLevel: 'silent', plugins: [react(), { name: 'only-fixture-transport', generateBundle() {
    if ([...this.getModuleIds()].some(p => /[\\/]node_modules[\\/]@tauri-apps[\\/]api[\\/]/.test(p))) throw Error('native_api_in_fixture');
  } }], define: { 'process.env.NODE_ENV': '"production"' },
    resolve: { alias: { '@tauri-apps/api/core': path.join(root, 'tests/fixtures/accountsTransport.ts') } },
    build: { outDir: path.join(run, 'bundle'), emptyOutDir: false, lib: { entry: path.join(root, 'tests/fixtures/accounts.tsx'), name: 'AccountsFixture', formats: ['iife'], fileName: () => 'fixture.js', cssFileName: 'fixture' } } });
});
test.afterEach(async ({}, info) => { outcomes.push({ name: info.title, status: info.status }); });
test.afterAll(() => { const after = fingerprint(); fs.writeFileSync(path.join(run, 'result.json'), JSON.stringify({ before, after, inputs_unchanged: JSON.stringify(before) === JSON.stringify(after), outcomes, audits }, null, 2)); });
async function setup(page: Page, language = 'en', theme = 'dark', scenario = 'data') {
  await page.route('http://127.0.0.1:54329/', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><title>Headroom synthetic accounts fixture</title></head><body></body></html>' }));
  await page.goto('http://127.0.0.1:54329/');
  await page.addStyleTag({ path: path.join(run, 'bundle/fixture.css') });
  await page.addScriptTag({ path: path.join(run, 'bundle/fixture.js') });
  await page.addScriptTag({ path: path.join(root, 'node_modules/axe-core/axe.min.js') });
  await page.evaluate(v => (window as any).AccountsFixture.mount(v.language, v.theme, v.scenario), { language, theme, scenario });
  await expect(page.locator('.accounts')).toBeVisible();
  await expect(page.locator('.accounts__refresh')).toBeEnabled();
}
async function audit(page: Page, label: string) {
  const value = await page.evaluate(async () => (window as any).axe.run(document, { runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'] } }));
  const violations = value.violations.map((v: any) => ({ id: v.id, impact: v.impact, targets: v.nodes.map((n: any) => n.target) }));
  audits.push({ label, violations }); expect(violations.filter((v: any) => ['serious', 'critical'].includes(v.impact))).toEqual([]);
}
test('accounts: local metadata, unsupported Desktop, languages, focus and responsive layout', async ({ page }) => {
  for (const language of ['tr', 'en']) for (const theme of ['dark', 'light']) {
    await setup(page, language, theme);
    const first = page.locator('.accounts__row').first();
    await expect(first).toContainText(language === 'tr' ? 'Yerel giriş kaydı var' : 'Local sign-in record found');
    await expect(page.locator('.accounts__row').nth(1)).toContainText(language === 'tr' ? 'Hesap durumu bilinmiyor' : 'Account status is unknown');
    await first.locator('select').selectOption('7132bced-3ce0-4c1b-8f48-d93d7b96f7c3');
    await expect(first.locator('.accounts__login')).toHaveCount(0);
    await first.locator('select').selectOption('7132bced-3ce0-4c1b-8f48-d93d7b96f7c0');
    await expect(first.locator('.accounts__login')).toBeEnabled();
    await first.locator('.accounts__login').click();
    const dialog = page.locator('.accounts-dialog[open]'); await expect(dialog).toContainText('ChatGPT');
    await expect(dialog.locator('button').first()).toBeFocused();
    for (let i = 0; i < 6; i++) { await page.keyboard.press('Tab'); expect(await dialog.evaluate(d => d.contains(document.activeElement))).toBe(true); }
    await audit(page, `${language}-${theme}-dialog`);
    await page.keyboard.press('Escape'); await expect(dialog).toHaveCount(0); await expect(first.locator('.accounts__login')).toBeFocused();
    expect(await page.evaluate(() => (window as any).AccountsFixture.history().includes('start_login'))).toBe(false);
    for (const width of [1024, 683]) {
      await page.setViewportSize({ width, height: 720 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)).toBe(false);
      await audit(page, `${language}-${theme}-${width}`);
    }
    if (language === 'tr' && theme === 'dark' || language === 'en' && theme === 'light') await page.screenshot({ path: path.join(run, `${language}-${theme}.png`), fullPage: true });
  }
});
test('accounts: pending flow survives remount, cancellation failure retains ownership and can retry', async ({ page }) => {
  await setup(page); await page.locator('.accounts__login').first().click();
  await page.locator('.accounts-dialog[open] .button-primary').click(); await expect(page.locator('.accounts__flow')).toBeVisible();
  await page.evaluate(() => (window as any).AccountsFixture.show(false)); await expect(page.locator('.accounts')).toHaveCount(0);
  await page.evaluate(() => (window as any).AccountsFixture.show(true)); await expect(page.locator('.accounts__flow')).toBeVisible();
  expect(await page.evaluate(() => (window as any).AccountsFixture.history().includes('cancel_login'))).toBe(false);
  await page.evaluate(() => (window as any).AccountsFixture.setFlowState('cleanup_failed'));
  await expect(page.locator('.accounts__flow')).toContainText('could not be confirmed closed');
  await expect(page.locator('.accounts__refresh')).toBeDisabled();
  await page.locator('.accounts__cancel-flow').click(); await expect(page.locator('.accounts__flow')).toHaveCount(0);
  await expect(page.locator('.accounts__refresh')).toBeEnabled();
  expect(await page.evaluate(() => (window as any).AccountsFixture.history().filter((v: string) => v === 'cancel_login').length)).toBe(1);
  await setup(page, 'en', 'dark', 'lost_start_reply');
  await page.locator('.accounts__login').first().click(); await page.locator('.accounts-dialog[open] .button-primary').click();
  await expect(page.locator('[role="alert"]')).toContainText('unavailable');
  await page.locator('.accounts__refresh').click(); await expect(page.locator('.accounts__flow')).toBeVisible();
  await page.locator('.accounts__cancel-flow').click(); await expect(page.locator('.accounts__flow')).toHaveCount(0);
});
test('accounts: absence, transport failure, launch failure and rescan discard stale reads', async ({ page }) => {
  await setup(page, 'en', 'dark', 'absent'); await expect(page.locator('.accounts__row').first()).toContainText('No local sign-in record');
  await setup(page, 'en', 'dark', 'error'); await expect(page.locator('.accounts__row').first()).toContainText('unavailable');
  await expect(page.locator('body')).not.toContainText('sentinel');
  await setup(page, 'en', 'dark', 'launch_failure'); await page.locator('.accounts__login').first().click();
  await page.locator('.accounts-dialog[open] .button-primary').click(); await expect(page.locator('[role="alert"]')).toContainText('could not');
  await expect(page.locator('.accounts__flow')).toHaveCount(0);
  await setup(page); await page.evaluate(() => (window as any).AccountsFixture.blockRead());
  await page.locator('.accounts__refresh').click(); await page.evaluate(() => (window as any).AccountsFixture.rescan());
  await expect(page.locator('.accounts__row').first()).not.toContainText('Local sign-in record found');
  await page.evaluate(() => (window as any).AccountsFixture.releaseRead()); await expect(page.locator('.accounts__refresh')).toBeEnabled();
  await expect(page.locator('.accounts__row').first()).toContainText('Local sign-in record found');
});
