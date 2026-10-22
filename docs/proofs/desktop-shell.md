# H013-a — Desktop shell foundation

**Date:** 2026-10-20 · **Status:** local scaffold/build PASS; native UI screenshot/navigation not verified.

## Final UI-only revision

The final H013-a UI revision aligns semantic graphite/mint tokens with [decision 001](../decisions/001-ui-direction.md), changes the footer to `Yerel istatistikler / Local statistics`, removes the activity copy’s claim that content is never retained, and localizes the Headroom overview accessible name. It adds no sample metrics, detected clients, or demo values. The prior native run exercised the unchanged Rust snapshot command and UI invocation path; this revision changed only `src/App.tsx` and `src/styles.css`.

## Toolchain and fingerprints

Host: Windows 10 Pro 22H2 x64. Toolchain: Node 25.2.1, pnpm 11.25.0, Rust/Cargo 1.92.0. Frontend versions are pinned in `package.json`; pnpm store, Cargo home, and Cargo target were under `work/caches/`.

| Input | SHA-256 |
| --- | --- |
| `src/App.tsx` | `a81e93932b8410e49e2e3c7fb0c8c01ca695889c661ca63057179e24e2b3219e` |
| `src/styles.css` | `10f8cfbc03684854bfc3a59bf753aa86e24e69e644a2b2f96ca235043e56b7f6` |
| `src/runtime/snapshot.ts` | `530137113e4967903ea7295889bd57c33c08a0b29c6361ee1046fbffe6eba168` |
| `src-tauri/src/main.rs` | `4413adc291d93cb3d780bcadf388acedf0cf48ae98dcb1600ea81380ee6ce9b6` |
| `package.json` | `1a32c9eb5e8a81ade7e8b793ce78bdde43ad89960ce8cb1ee4348a3dde2eed2d` |
| `pnpm-lock.yaml` | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |
| `src-tauri/Cargo.lock` | `4475bc1476a867ce7da618672efd95764be735c556855f3617e3d6be7bca691f` |
| `src-tauri/tauri.conf.json` | `06612d83c9bf440ed22d6c58759f007e986f038b0412c37b1f9e69ebb0c91875` |
| `src-tauri/tauri.dev.conf.json` | `60635604c8ce0fd8071903d24dcea6ec308f7dcb229c218e5d3b4d100a5861c1` |

## Verification

After the final UI-only edits, `pnpm typecheck` and `pnpm build` both passed. Snapshot parser tests and Rust/Cargo checks passed on the unchanged parser/Rust sources earlier in H013-a; they were not repeated for copy/color-only changes. Earlier commands were `pnpm test:snapshot` (5/5), `cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml --bin headroom-desktop request_id_requires_uuid_version_and_variant_bits` (1/1), with `CARGO_HOME=work/caches/cargo` and `CARGO_TARGET_DIR=work/caches/cargo-target`.

The earlier `pnpm tauri:dev` session displayed a real Tauri window (`Headroom`, PID 32720; session 18732). Native accessibility inspection observed “Yerel durum güncel”, engine `unknown`, connection unverified, compression not implemented, both savings values blank, and disabled actions. That confirms the Rust snapshot reached the native UI. The process/session is no longer live. Native frame capture timed out twice and click geometry was unavailable, so screenshot, keyboard-navigation, and visual-layout acceptance are not claimed for that session.

A new hidden Vite launch for browser review was rejected by the automatic command policy with `blocked by policy`; that command did not run. A subsequent read-only port check found `127.0.0.1:1420` free. No alternate launcher was used. The separate built-artifact review below starts no server/listener.

## Built-artifact UI review

On 2026-10-20 the coordinator ran `node work/ui-review-h013/validate-shell.cjs`. Playwright fulfilled synthetic-origin requests directly from the built `dist` files and applied the production CSP; no HTTP server, Vite process, or socket listener was created. This is browser UI evidence, not a new native IPC/screenshot proof.

Result: 32 layouts PASS (four screens × TR/EN × light/dark × 1280×800/1024×720), no horizontal overflow, zero JavaScript errors/external requests. Empty amount/rate and disabled actions were checked. System-theme changes and visible keyboard focus passed. Source hashes before/after matched this document's final candidate. Node25.2.1, Playwright1.62.1, Edge155.0.4283.62; full metadata in `work/ui-review-h013/summary.json`. The two saved UI preview images were visually inspected; native graphics capture remains unverified.

This proof covers the local Windows 10 shell only. H014 runtime management, Windows 11/clean-machine validation, native screenshot/navigation QA, and release packaging remain open.
