import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "retained-versions.spec.ts",
  workers: 1,
  maxFailures: 1,
  timeout: 30_000,
  outputDir: "../../work/proofs/macos/h048-b6/retained-ui-browser/playwright-output",
  reporter: [["list"]],
  use: {
    browserName: "chromium",
    channel: process.platform === "win32" ? "msedge" : undefined,
    headless: true,
    timezoneId: "Europe/Istanbul",
    reducedMotion: "reduce",
    trace: "off",
    screenshot: "off",
    video: "off",
  },
});
