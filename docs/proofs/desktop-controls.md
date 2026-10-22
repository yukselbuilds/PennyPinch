# H014-c desktop controls proof

This slice connects the Graphite overview to the existing Rust runtime manager. The UI polls the real Tauri snapshot command every second and invokes `start_engine(request_id)` or generation-bound `stop_engine(request_id, expected_generation)`. It validates and reconstructs the exact DTO allowlist, accepts only the pinned `0.41.0` engine version and a 32-hex runtime generation, and maps the manager's ten fixed PublicError codes to localized messages. It enforces manager state invariants: `stop` matches generation presence; generation and engine version appear as a pair; `ready` and `stopping` require that pair; `stopped`, `unavailable`, and `starting` forbid it; `ready` has no error code. `failed` and `unknown` retain either valid nullable state. A failed fresh read clears the previous snapshot so stale Ready state and its controls cannot remain active. Cleanup error copy asks the user to check current state rather than asserting that start is disabled. Poll results are sequence-guarded so an older response cannot replace a newer lifecycle observation. Lifecycle actions are locked while pending. Pause and client connection remain disabled; savings stay unknown (`—`).

## Verification

Focused frontend checks passed on Windows 10 Pro 22H2 x64:

```powershell
pnpm test:snapshot
pnpm typecheck
pnpm build
$env:CARGO_HOME='work/caches/cargo'
$env:CARGO_TARGET_DIR='work/caches/cargo-target'
cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

Results: snapshot contract tests 9/9 passed; TypeScript typecheck passed; Vite production build passed; embedded-dist Tauri debug binary built successfully. Toolchain: Node `v25.2.1`, pnpm `11.25.0`, Vite `8.3.4`, TypeScript `7.0.2`, Rust/Cargo `1.92.0`. Cargo used project-local `CARGO_HOME=work/caches/cargo` and `CARGO_TARGET_DIR=work/caches/cargo-target`.

## Fingerprints

| Input or artifact | SHA-256 |
| --- | --- |
| `src/runtime/snapshot.ts` | `3a49966bc2e4fdbd85718a66f0e4ab26dca84bc8097665448c27a2a631f95af7` |
| `src/App.tsx` | `d98872d2794be664ced81d1d602d4ce9441ba07ef555a9c02eb50b0e90585c90` |
| `tests/app/snapshot.test.mjs` | `6809a053329842383c9033704943b1d621effc0b0238c374549c90f9fb567c9e` |
| `package.json` | `1a32c9eb5e8a81ade7e8b793ce78bdde43ad89960ce8cb1ee4348a3dde2eed2d` |
| `pnpm-lock.yaml` | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |
| `src-tauri/Cargo.toml` | `b2b3b4c237e30fdb0b196b57110383273219b62bb2c58c613dcf6761871d4de4` |
| `src-tauri/Cargo.lock` | `61f5fd42ecdff941fa80f0ade6a16c0611c403106b2f3448a95e68a93ae90319` |
| `src-tauri/tauri.conf.json` | `06612d83c9bf440ed22d6c58759f007e986f038b0412c37b1f9e69ebb0c91875` |
| `src-tauri/src/runtime/manager.rs` | `be8a16024caa5db36b10535a37b3851d2a342c564e1044916baddf09ddee34d9` |
| `dist/index.html` (497 bytes) | `8b1bbe47596bdf48064cd11fb521890047b08efbb185cabbc8a6f68b315a515c` |
| `dist/assets/index-C48FrDVZ.js` (241,675 bytes) | `c5d1c975f414da80b8ed205f1f4ea98fa3b5c50cd09f383e749702eeb0336587` |
| `dist/assets/index-DE88eyXW.css` (11,541 bytes) | `9f96ecd32aa8be2f0f55fc8459b417dc142c6eba08ae73dc3d11903a1a379cd4` |
| `work/caches/cargo-target/debug/headroom-desktop.exe` (13,541,888 bytes) | `46286697eb4a1d70dbb8df2b3b8791310515294e415be5412fd636b45bf856e7` |

## Boundaries

The build embeds the production frontend in the debug desktop binary. At the worker handoff, native GUI start/ready/stop and window-close behavior were pending the coordinator's check; the completed manual native acceptance and independent exit/process checks are recorded below. The H014-b Rust manager lifecycle proof remains separate backend evidence. This does not establish release bundle resource resolution, Windows 11, clean-machine installation, client connection, pause, or compression measurements.

## Coordinator UI review

The three requested source fixes are included: a failed fresh snapshot clears prior ready state, state/generation/version/stop fields must agree, and cleanup error copy no longer assumes that start is always disabled. The final decoder suite has 9 passing tests. The coordinator matched the source and binary fingerprints without repeating the successful build or backend gates.

`node work/ui-review-h014/validate-controls.cjs` passed 32 built-artifact layouts (four screens, two languages, two themes, two widths), empty amount/percentage, no horizontal overflow, no browser errors and no external requests. It also passed three explicitly mocked bridge UI checks: duplicate start suppression, failed polling clearing ready/actions, and recovery with generation-correlated stop. All assets were fulfilled in memory from `dist`; no server/listener/Vite was started. These are browser/component checks, not native IPC or engine evidence. Report: `work/ui-review-h014/summary.json`; preview: `work/ui-review-h014/overview-tr-dark.png`, visually reviewed by the coordinator.

The embedded native binary was launched consoleless with `.NET ProcessStartInfo` (`UseShellExecute=false`, `CreateNoWindow=true`), PID21176. The real window's accessibility text showed “Yerel durum güncel” on Applications, confirming a decoded native snapshot. After restoring the initially minimized window, graphics capture failed with `FrameArrived timed out: timed out waiting on channel`; indexed click failed with `coordinate input geometry is unavailable`. Tab/F6 produced no observed focus/navigation change. No automated native-button success is claimed.

The user reported the requested start→ready→stop→stopped→start→ready→close-window sequence worked, with no observed issue. The coordinator separately collected `headroom_native_exit_code=0` from the retained process handle and confirmed zero remaining Headroom and packaged Python/runtime processes through read-only process queries. H014-c/H014 were accepted on this host using that manual native UI report plus automatic backend/exit/process evidence. This is not an automated native interaction test or a Windows11/installer/client compatibility claim.
