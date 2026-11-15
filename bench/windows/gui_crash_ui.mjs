import { chromium, expect } from "@playwright/test";

const [port, phase] = process.argv.slice(2);
if (!/^\d{1,5}$/.test(port) || Number(port) > 65535 || !["start", "stop-exit"].includes(phase)) {
  throw Error("invalid_fixture_arguments");
}
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 15000 });
let stage = "owned_webview";
try {
  const pages = browser.contexts().flatMap(context => context.pages());
  const page = pages.find(page => /^https?:\/\/tauri\.localhost\//.test(page.url()));
  if (!page || pages.length !== 1) throw Error("unexpected_webview_target");
  page.setDefaultTimeout(15000);
  await expect(page.locator(".brand")).toBeVisible();
  await page.locator('button[data-screen="overview"]').click();
  const panel = page.locator(".optimization-control");
  if (phase === "start") {
    stage = "initial_stopped";
    await expect(panel.locator(".optimization-control__dot.is-stopped")).toBeVisible();
    stage = "native_start";
    await panel.locator(".optimization-control__primary").click();
    await expect(panel.locator(".optimization-control__dot.is-enabled, .optimization-control__dot.is-ready"))
      .toBeVisible({ timeout: 120000 });
    const value = await page.evaluate(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      const runtime = await invoke("get_runtime_snapshot", { requestId: crypto.randomUUID() });
      const ingress = await invoke("get_ingress_snapshot", { requestId: crypto.randomUUID() });
      return { status: runtime.engine.status, generation: runtime.runtime_generation,
        version: runtime.engine_version, endpoint: ingress.endpoint, backend: ingress.backend.status };
    });
    if (value.status !== "ready" || value.version !== "0.41.0" || value.backend !== "ready") {
      throw Error("runtime_not_ready");
    }
    console.log(JSON.stringify({ phase, stage, passed: true, initial_status: "stopped", ...value }));
  } else {
    stage = "native_stop";
    await panel.locator("details summary").click();
    await panel.getByRole("button", { name: /^(Motoru durdur|Stop engine)$/ }).click();
    await expect(panel.locator(".optimization-control__dot.is-stopped")).toBeVisible({ timeout: 90000 });
    stage = "normal_exit";
    await page.locator('button[data-screen="settings"]').click();
    await page.locator(".desktop-lifecycle__exit").click();
    await page.locator("dialog[open][data-desktop-exit-confirmation] .desktop-exit-dialog__confirm").click();
    console.log(JSON.stringify({ phase, stage, passed: true, stopped_before_exit: true }));
  }
} catch (error) {
  console.log(JSON.stringify({ phase, stage, passed: false, error_kind: error.name }));
  process.exitCode = 1;
} finally {
  await browser.close();
}
