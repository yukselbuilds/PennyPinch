import { expect, test, type Page } from "@playwright/test";
import { build } from "vite";
import react from "@vitejs/plugin-react";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const root = process.cwd();
const proofRoot = path.join(root, "work/proofs/macos/h048-b6/retained-ui-browser");
const runRoot = path.join(proofRoot, `run-${crypto.randomUUID()}`);
const bundleRoot = path.join(runRoot, "bundle");
const origin = "http://retained-versions.fixture";
const sourceFiles = [
  "src/lib/retainedVersions.ts",
  "src/features/integrations/RetainedVersions.tsx",
  "src/features/integrations/retained-versions.css",
  "src/styles.css",
  "tests/fixtures/retainedVersions.tsx",
  "tests/e2e/retained-versions.spec.ts",
  "tests/e2e/retained-versions.config.ts",
  "package.json",
  "pnpm-lock.yaml",
];
const outcomes: Array<{ name: string; status: string; expected: string }> = [];
let beforeHashes: Record<string, string> = {};
let blockedExternalRequests = 0;
let browserVersion = "unknown";
let pnpmVersion: string | null = null;

function hashes(): Record<string, string> {
  return Object.fromEntries(sourceFiles.map((file) => [
    file,
    crypto.createHash("sha256").update(fs.readFileSync(path.join(root, file))).digest("hex"),
  ]));
}

function getPnpmVersion(): string | null {
  try {
    const executable = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
    return execFileSync(executable, ["--version"], { cwd: root, encoding: "utf8", timeout: 5_000, windowsHide: true }).trim();
  } catch {
    const match = process.env.npm_config_user_agent?.match(/pnpm\/([^\s]+)/);
    return match?.[1] ?? null;
  }
}

test.beforeAll(async () => {
  if (!path.basename(root).toLowerCase().includes("headroom") || fs.realpathSync(root) !== root) {
    throw new Error("unexpected_fixture_root");
  }
  fs.mkdirSync(runRoot, { recursive: true });
  beforeHashes = hashes();
  pnpmVersion = getPnpmVersion();
  const virtualModule = "\0retained-versions-native-stub";
  await build({
    configFile: false,
    logLevel: "silent",
    plugins: [
      react(),
      {
        name: "retained-versions-no-native-api",
        enforce: "pre",
        resolveId(source) {
          return source === "@tauri-apps/api/core" ? virtualModule : null;
        },
        load(id) {
          if (id !== virtualModule) return null;
          return `export function isTauri(){return false};export async function invoke(){window.__retainedNativeCalls=(window.__retainedNativeCalls||0)+1;throw new Error('native_api_forbidden')}`;
        },
        generateBundle() {
          const tauriModule = [...this.getModuleIds()].some((id) => /[\\/]node_modules[\\/]@tauri-apps[\\/]api[\\/]/.test(id));
          if (tauriModule) throw new Error("native_module_in_browser_bundle");
        },
      },
    ],
    define: { "process.env.NODE_ENV": '"production"' },
    build: {
      minify: true,
      emptyOutDir: false,
      outDir: bundleRoot,
      lib: {
        entry: path.join(root, "tests/fixtures/retainedVersions.tsx"),
        name: "RetainedVersionsTestBundle",
        formats: ["iife"],
        fileName: () => "fixture.js",
        cssFileName: "fixture",
      },
    },
  });
});

test.afterEach(async ({ page }, info) => {
  outcomes.push({ name: info.title, status: info.status, expected: info.expectedStatus });
  expect(blockedExternalRequests).toBe(0);
  expect(await page.evaluate(() => window.RetainedVersionsFixture?.nativeCalls() ?? 0)).toBe(0);
});

test.afterAll(() => {
  const afterHashes = hashes();
  const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  const summary = {
    result: outcomes.every((item) => item.status === item.expected) ? "passed" : "failed",
    command: "pnpm exec playwright test --config tests/e2e/retained-versions.config.ts --max-failures=1",
    root_realpath_matches: fs.realpathSync(root) === root,
    node: process.version,
    pnpm: pnpmVersion,
    playwright: packageJson.devDependencies?.["@playwright/test"] ?? null,
    vite: packageJson.devDependencies?.vite ?? null,
    react_plugin: packageJson.devDependencies?.["@vitejs/plugin-react"] ?? null,
    browser: browserVersion,
    browser_channel: process.platform === "win32" ? "msedge" : "playwright-chromium",
    external_request_blocks: blockedExternalRequests,
    native_api_calls: 0,
    outcomes,
    source_sha256_before: beforeHashes,
    source_sha256_after: afterHashes,
    source_unchanged: JSON.stringify(beforeHashes) === JSON.stringify(afterHashes),
  };
  fs.writeFileSync(path.join(runRoot, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`, { flag: "wx" });
});

async function setup(
  page: Page,
  options: { clientKind?: string; scope?: string; language?: "tr" | "en"; scenario?: string; disabled?: boolean } = {},
  heldReads: string[] = [],
) {
  await page.route("**/*", async (route) => {
    if (route.request().url() === `${origin}/`) {
      await route.fulfill({ contentType: "text/html", body: "<!doctype html><html><head><title>Retained versions fixture</title></head><body></body></html>" });
    } else {
      blockedExternalRequests += 1;
      await route.abort();
    }
  });
  await page.goto(`${origin}/`);
  await page.addStyleTag({ path: path.join(bundleRoot, "fixture.css") });
  await page.addScriptTag({ path: path.join(bundleRoot, "fixture.js") });
  await page.evaluate(() => {
    const api = (window as any).RetainedVersionsTestBundle?.retainedVersionsFixtureApi;
    if (!api) throw new Error("fixture_api_unavailable");
    window.RetainedVersionsFixture = api;
  });
  browserVersion = page.context().browser()?.version() ?? "unknown";
  await page.evaluate(({ options: next, held }) => {
    window.RetainedVersionsFixture.mount({ ...next, heldReads: held } as Parameters<typeof window.RetainedVersionsFixture.mount>[0]);
  }, { options, held: heldReads });
}

async function fixtureCalls(page: Page) {
  return page.evaluate(() => window.RetainedVersionsFixture.calls());
}

test("mount reads the selected scope but never archives implicitly", async ({ page }) => {
  await setup(page, { scope: "project_local_config" });
  await expect(page.getByRole("region", { name: "Korunan sürümler" })).toBeVisible();
  const calls = await fixtureCalls(page);
  expect(calls.map((call) => call.command)).toEqual(["get_retained_config_versions"]);
  expect(calls[0].request).toMatchObject({ client_kind: "claude_code", scope: "project_local_config" });
  await page.screenshot({ path: path.join(runRoot, "retained-panel.png") });
});

test("checkbox confirmation and one click archive the reviewed scope once", async ({ page }) => {
  await setup(page, { scope: "project_local_config" });
  const checkbox = page.getByRole("checkbox", { name: /Mevcut ayarı kullanmak istiyorum/ });
  const action = page.getByRole("button", { name: "Mevcut ayarla devam et" });
  await expect(checkbox).toBeVisible();
  await expect(action).toBeDisabled();
  expect((await fixtureCalls(page)).filter((call) => call.command === "archive_retained_config_versions")).toHaveLength(0);

  await page.evaluate(() => window.RetainedVersionsFixture.holdArchive());
  await checkbox.check();
  await action.click();
  await expect(page.getByRole("button", { name: "Sürümler korunuyor…" })).toBeDisabled();
  let calls = await fixtureCalls(page);
  const archiveCalls = calls.filter((call) => call.command === "archive_retained_config_versions");
  expect(archiveCalls).toHaveLength(1);
  expect(Object.keys(archiveCalls[0].request).sort()).toEqual([
    "client_kind", "expected_review_fingerprint", "request_id", "schema_version", "scope",
  ]);
  expect(archiveCalls[0].request).toMatchObject({
    client_kind: "claude_code",
    scope: "project_local_config",
    expected_review_fingerprint: "c".repeat(64),
  });
  expect(archiveCalls[0].request.request_id).toMatch(/^[0-9a-f-]{36}$/);

  await page.evaluate(() => (document.querySelector(".retained-versions__primary") as HTMLButtonElement).click());
  calls = await fixtureCalls(page);
  expect(calls.filter((call) => call.command === "archive_retained_config_versions")).toHaveLength(1);
  await page.evaluate(() => window.RetainedVersionsFixture.releaseArchive());
  await expect(page.getByRole("status").filter({ hasText: "Sürümler korundu" })).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.RetainedVersionsFixture.archivedCallbacks())).toBe(1);
  expect(await page.evaluate(() => window.RetainedVersionsFixture.busyTransitions())).toEqual([true, false]);
});

test("stale user-scope response cannot replace the newly selected project scope", async ({ page }) => {
  await setup(page, {}, ["user_config", "project_local_config"]);
  await expect.poll(async () => (await fixtureCalls(page)).length).toBe(1);
  await expect(page.getByText("1 / 2")).toHaveCount(0);
  await page.evaluate(() => window.RetainedVersionsFixture.setContext({ scope: "project_local_config" }));
  await page.evaluate(() => window.RetainedVersionsFixture.releaseRead("user_config"));
  await expect.poll(async () => (await fixtureCalls(page)).length).toBe(2);
  const calls = await fixtureCalls(page);
  expect(calls.map((call) => call.request.scope)).toEqual(["user_config", "project_local_config"]);
  await expect(page.getByText("1 / 2")).toHaveCount(0);
  await page.evaluate(() => window.RetainedVersionsFixture.releaseRead("project_local_config"));
  await expect(page.getByText("2 / 3")).toBeVisible();
  expect((await fixtureCalls(page)).filter((call) => call.command === "archive_retained_config_versions")).toHaveLength(0);
});

test("unsupported data stays hidden while error, unresolved, and capacity states stay safe", async ({ page }) => {
  await setup(page, { scenario: "unsupported" });
  await expect(page.locator(".retained-versions")).toHaveCount(0);

  await setup(page, { scenario: "error" });
  await expect(page.getByText("Başka bir ayar işlemi sürüyor. Biraz sonra yenileyin.")).toBeVisible();

  await setup(page, { scenario: "unresolved" });
  await expect(page.getByText("Bazı kayıtlar güvenle eşleştirilemedi; işlem yapılamaz.")).toBeVisible();
  await expect(page.getByRole("checkbox")).toBeDisabled();
  await expect(page.getByRole("button", { name: "Mevcut ayarla devam et" })).toBeDisabled();

  await setup(page, { scenario: "limit" });
  await expect(page.getByText("Korunan kayıt sınırına ulaşıldı. Yeni işlem için saklanan kayıtları inceleyin.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Mevcut ayarla devam et" })).toBeDisabled();
  expect((await fixtureCalls(page)).filter((call) => call.command === "archive_retained_config_versions")).toHaveLength(0);
});

test("TR and EN confirmation work from the keyboard; parent busy disables every control", async ({ page }) => {
  for (const language of ["tr", "en"] as const) {
    await setup(page, { language });
    const checkbox = page.getByRole("checkbox", {
      name: language === "tr"
        ? "Mevcut ayarı kullanmak istiyorum; korunan sürümler saklansın."
        : "I want to keep using the current setting and retain these versions.",
    });
    await checkbox.focus();
    await page.keyboard.press("Space");
    await expect(checkbox).toBeChecked();
    const action = page.getByRole("button", { name: language === "tr" ? "Mevcut ayarla devam et" : "Continue with current setting" });
    await expect(action).toBeEnabled();
    await action.focus();
    await page.keyboard.press("Enter");
    await expect.poll(async () => (await fixtureCalls(page)).filter((call) => call.command === "archive_retained_config_versions").length).toBe(1);
    await expect.poll(() => page.evaluate(() => window.RetainedVersionsFixture.archivedCallbacks())).toBe(1);
  }

  await setup(page, { disabled: true });
  await expect(page.getByRole("button", { name: "Yenile" })).toBeDisabled();
  await expect(page.getByRole("checkbox")).toBeDisabled();
  await expect(page.getByRole("button", { name: "Mevcut ayarla devam et" })).toBeDisabled();
  expect((await fixtureCalls(page)).filter((call) => call.command === "archive_retained_config_versions")).toHaveLength(0);
});
