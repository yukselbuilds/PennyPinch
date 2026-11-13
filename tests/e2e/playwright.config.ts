import {defineConfig} from '@playwright/test';
export default defineConfig({
 testDir:'.',testMatch:['accessibility.spec.ts','accounts.spec.ts'],workers:1,timeout:120000,
 outputDir:'../../work/proofs/accessibility/playwright-output',
 reporter:[['list']],use:{browserName:'chromium',channel:process.platform==='win32'?'msedge':undefined,
  headless:true,timezoneId:'Europe/Istanbul',reducedMotion:'reduce',trace:'off',screenshot:'off',video:'off'},
});
