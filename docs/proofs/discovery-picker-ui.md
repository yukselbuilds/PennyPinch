# H016-c2: Applications file picker UI

Date: 2026-10-20. The Applications screen now calls the native
`select_client_file` command with only a correlated request ID and client kind.
It accepts only the exact picker cancellation envelope or the regular validated
discovery snapshot/error DTO. A manually selected entry is rendered as
`Selected file`, with unknown version and connection status still
`not_verified`. The renderer does not receive or retain a file path.

Each client card offers “Dosyadan ekle” / “Add from file” in the desktop app.
The UI names the accepted command files (`codex`, `claude`, or `opencode` with
`.exe`, `.cmd`, or `.bat`), explains that selected candidates stay in memory
until Headroom closes, and keeps Connect unavailable. Browser preview has no
native picker or fabricated result. Cancel preserves existing results; an
error preserves the last completed scan when one exists. Scan and picker
mutations are serialized, duplicate clicks are blocked, and a remount waits for
an outstanding picker before reading a fresh backend snapshot. Stale component
replies are discarded.

## Verification

On Windows 10 Pro 22H2 x64, Node `v25.2.1`, pnpm `11.25.0`, and the locked
project dependency tree, with no provider request:

| Command | Result |
| --- | --- |
| `pnpm test:integrations` | PASS, 14 tests: exact/correlated DTOs, picker-only cancellation, scan rejection of cancellation, typed invoke args, browser unavailability, manual metadata, mutation serialization, remount refresh, and stale request handling |
| `pnpm typecheck` | PASS |
| `pnpm build` | PASS, Vite production assets generated |

No Vite server, Tauri dev process, or native app was started for this slice.
The build is static frontend output; it does not exercise the native dialog or
the Rust command. H016-c1's accepted backend contract is recorded in
[`discovery-picker-backend.md`](discovery-picker-backend.md). Native WebView
selection/cancellation and full application acceptance remain for the parent
QA pass. A selected program is never launched by this UI proof, and no client
connection or compatibility is established.

## Fingerprints

SHA-256, byte lengths, and the generated asset names for this candidate:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `src/lib/integrations.ts` | 15,389 | `55c49ee40521a28f4f1bc1bbf01f447f93fe9eb407eda90f4344bc6ab0bb6cb6` |
| `src/features/integrations/Applications.tsx` | 16,983 | `7c96941aa48773fb7be93995a329f37c253629654ee75854f78c2fa82f579ff2` |
| `src/styles.css` | 18,570 | `ea86bb1bf6c820cae17a7a32eb342da78a5a2759cf4936d620e81d14271eef6c` |
| `tests/app/integrations.test.mjs` | 13,676 | `30a93aa560d6ab8d9a66e5b7e60994f6cf9f7aa5cba1f3a87d83908ae19431dd` |
| `src/App.tsx` | 27,997 | `9dc9d07bdaf84ed2f11feb7bcb872b6d69983881042a3685a00a120634a63613` |
| `package.json` | 983 | `3a074f8c9713b1763173562fdae165f51f5ceb6dd203a15da93be320e810506d` |
| `pnpm-lock.yaml` | 28,140 | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |
| `tsconfig.json` | 532 | `d21dd9148330758646c6a1c605f83a88ecd9224126ff49c205e8d105525cc03b` |
| `vite.config.ts` | 292 | `dcb19fe986150626ba66f618f7a579bc1d1232ac2e27105bcab078e78e24b36a` |
| `src-tauri/src/integrations/discovery.rs` (reused c1) | 40,654 | `caadea772b464df077550f6f3411a0926a18e1dd2c252f1dfcd4c1a4cb8519b8` |
| `src-tauri/src/main.rs` (reused c1 command registration) | 6,534 | `7dab6cf099ac7c1f2ed8b20975161936d35c97b9603c15f9d9fa0cb0217131a0` |
| `dist/index.html` | 497 | `e24960db854d6d14e72ee616df3074e477b2d50338e2f83f348c85a7880e67c6` |
| `dist/assets/index-Bq9w-_o-.css` | 16,373 | `b89004a155faa6b3146b7065b0975c27084a5e2d2b86ec38dccda1d2202d6afe` |
| `dist/assets/index-B0Q5-sFA.js` | 266,713 | `87a5db36d2abc071b79cce74cbb42f92c1f80eac2aa5252e13f7a8eefbb1440a` |
