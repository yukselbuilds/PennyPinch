# H016-b local discovery UI proof

The Applications screen now reads the backend-owned discovery snapshot and invokes the bounded `scan_discovery` command when the first snapshot is `not_scanned`. A shared scan coordinator prevents a second scan during React StrictMode or a remount, including a delayed stale `not_scanned` read after the first scan has already completed; stale component replies are discarded. Later mounts reuse the backend snapshot, and the toolbar can request a fresh scan. Browser preview has no native discovery bridge and displays no installation results.

The client accepts only the schema-1 snapshot/error envelopes, exact field sets, correlated RFC UUIDs, safe-integer generations, at most 512 unique installation IDs, the fixed source/location labels, version syntax used by the backend, and fixed warning/error allowlists. It reconstructs an allowlisted object and never renders installation IDs or file paths. The three cards distinguish detected CLI/Desktop surfaces and unknown versions; connection remains “not verified.” An installation is called absent only after a completed partial scan, with the limited-scope and absence-is-not-unsupported explanation visible. Connect and file-add actions remain disabled for H016-c.

## Verification

All checks ran on the local Windows 10 22H2 x64 host with Node `v25.2.1`, pnpm `11.25.0`, the locked dependency tree, and no provider request. No Vite server, Tauri dev process, native app, or listener was started.

| Command | Result |
| --- | --- |
| `pnpm test:integrations` | PASS, 9 focused tests: DTO allowlist/correlation/cap, fixed errors, no-bridge behavior, native command arguments, sanitized bridge failure, first-scan cache/coalescing, remount while scanning, delayed stale read, stale request sequence |
| `pnpm typecheck` | PASS |
| `pnpm build` | PASS, static Vite production assets built |

The focused UI tests do not run the Rust discovery scanner. The accepted H016-a backend remains the source of discovery results; its unchanged source hashes are recorded below. Native Applications-screen QA is still pending parent review. This proof does not establish a found client’s connection or protocol compatibility.

## Fingerprints

SHA-256 fingerprints for the final H016-b source, build inputs, reused backend, and generated static assets:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `src/lib/integrations.ts` | 12,639 | `c7b9f8ebcb631580378ae4a056b3a830ebab851318a22c815a99e2979b77ec47` |
| `src/features/integrations/Applications.tsx` | 12,398 | `346f2f68370a0c61f4e38efe03b736db58e8b559ab781f93cc0b208372afd4ac` |
| `src/App.tsx` | 27,997 | `9dc9d07bdaf84ed2f11feb7bcb872b6d69983881042a3685a00a120634a63613` |
| `src/styles.css` | 18,464 | `b03c00ba0a6fb2517174a184f421699f85eec25a28966d737eabffec7a75505d` |
| `tests/app/integrations.test.mjs` | 9,081 | `b98f608df52be187940d7edce5f4af03d190c0272c185ad83909e9d61593afe9` |
| `package.json` | 983 | `3a074f8c9713b1763173562fdae165f51f5ceb6dd203a15da93be320e810506d` |
| `pnpm-lock.yaml` | 28,140 | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |
| `tsconfig.json` | 532 | `d21dd9148330758646c6a1c605f83a88ecd9224126ff49c205e8d105525cc03b` |
| `vite.config.ts` | 292 | `dcb19fe986150626ba66f618f7a579bc1d1232ac2e27105bcab078e78e24b36a` |
| `src-tauri/src/integrations/discovery.rs` (reused H016-a) | 33,462 | `171d176ecc69b533a310eff4412ad85320ca5a0f9b34ea89366e29c1ad53f03c` |
| `src-tauri/src/main.rs` (reused command registration) | 5,349 | `c3f4cc4045d33c95b38513044c485bbe06199e65fbead305145a47a34bbd9906` |
| `dist/index.html` | 497 | `f26535a66165ce2234daa4b3b53f5e9207aa36832026425e2c564dc8bdafb846` |
| `dist/assets/index-ByzPXFBF.css` | 16,279 | `efdfbe2fb450e67bad3165cf924e416732a1513788353f433116ed000af2b923` |
| `dist/assets/index-vQ9OADDh.js` | 262,960 | `818cd48bd6c7f1a7902c79cd1a783a4252ea2466927277e30dd3bc32131d060b` |
