# H015-b: persisted preferences UI

The settings screen now reads and saves the native preference store through the typed `get_preferences` and `save_preferences` commands. A selection is confirmed only after a correlated, exact-schema response acknowledges the save; a failed save restores the last confirmed selection and disables editing until a fresh read succeeds. Corrupt, conflicting, unavailable, or malformed replies never trigger a reset or overwrite. In a browser without the native bridge, theme and language remain session-only and the UI says so. No localStorage fallback or provider request is used. Runtime snapshot polling and engine controls remain independent of preference errors.

The decoder accepts only schema 1, matching RFC request IDs, safe-integer revisions, lowercase SHA-256 fingerprints, the pinned engine manifest reference, and the five fixed error envelopes. It reconstructs accepted data from the allowlist. Preference read/write sequence IDs reject stale replies and are invalidated on unmount. The browser preview does not claim persistence.

## Verification

On Windows 10 Pro 22H2 x64, Node `v25.2.1`, pnpm `11.25.0`, Rust `1.92.0 (ded5c06cf 2025-12-08)`, and Cargo `1.92.0 (344c4567c 2025-10-21)`:

```powershell
pnpm test:preferences
pnpm typecheck
pnpm build
$env:CARGO_HOME='work/caches/cargo'
$env:CARGO_TARGET_DIR='work/caches/cargo-target'
cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

Results: 6 focused preference tests passed; TypeScript check and production frontend build passed; the embedded standalone debug application build passed. The Rust store/manager tests were not repeated because no Rust source, lockfile, or runtime source changed. No Vite server or application process was started by this slice; native UI interaction and preference reload across application restarts remain for the parent’s native QA. Existing backend store evidence is in [preferences-store.md](preferences-store.md).

## Fingerprints

SHA-256 for the final UI candidate and embedded build:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `src/lib/preferences.ts` | 8,621 | `e70dcc851a39395734d60a12b189274209383d9e484368ebcf67ea586c5c1ed9` |
| `src/App.tsx` | 28,312 | `a1ada9ca78ae0d2e527ca98ac06ad0a86d120348b024852a5d750ee832646712` |
| `tests/app/preferences.test.mjs` | 5,623 | `60309327e5c22c5c4df26ce5ddf180ae91c39b5dd47e92c274d70848840ed948` |
| `package.json` | 884 | `4733528a7d7cd40280215151e17acbfe32f307197cff1e914f1aa35249da044d` |
| `pnpm-lock.yaml` | 28,140 | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |
| `dist/index.html` | 497 | `a464ebeccb846215000c55100115ba1a05da83cacb1a588cc2ce4d749c54c2c8` |
| `dist/assets/index-C9qh_heg.js` | 249,263 | `75e1954766bcbadb0d6ba43038b3295aa3f9b22fa4c72dfdc749d6af032f3b38` |
| `dist/assets/index-DE88eyXW.css` | 11,541 | `9f96ecd32aa8be2f0f55fc8459b417dc142c6eba08ae73dc3d11903a1a379cd4` |
| `work/caches/cargo-target/debug/headroom-desktop.exe` | 13,680,640 | `e4a7629f391d8389f6b9311710ee8fcb1867065f41054345d8a02dd1b09ca4c3` |

The build used the project-local pnpm store and `CARGO_HOME=work/caches/cargo`, `CARGO_TARGET_DIR=work/caches/cargo-target`. No dependency or lockfile update was required.

## Coordinator acceptance

Source and binary fingerprints matched. `node work/ui-review-h015/validate-preferences.cjs` passed32 layouts and6 explicitly mocked, in-memory bridge flows; zero browser errors/external requests, no server/Vite. The tests cover loading/save locks, correlated revision/fingerprint, save failure rollback, same-revision external edit conflict, language/system theme reload and media listener, and corrupt state leaving runtime controls independent. This is component evidence, not native IPC. Report `work/ui-review-h015/summary.json`; preview `settings-tr-system.png` was visually inspected.

The user reported real native Settings→Dark→English→“Preferences saved.”→close succeeded. Coordinator process handle recorded first native exit0 and zero Headroom processes. The actual schema1 preference file contained only the documented preference/manifest allowlists, revision4, `en`/`dark`; SHA-256 `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. The coordinator reopened the unchanged embedded EXE in a new process (PID880) and read the native accessibility text: `WORKSPACE`, `Overview`, `Settings`, `Local state updated`. This confirms the saved language was loaded rather than the Turkish default. Alt+F4 then closed the reopened app; its retained process handle returned exit0 and the process was absent. H015 accepted on this Windows10 host; no native color screenshot, automated mouse action, Windows11 or installer claim.

Windows capture/input limitations and the manual-vs-automatic evidence split are recorded in [the review](../reviews/H015-b.md). Preferences remain en/dark in the project's development app state. Both QA app processes are closed.
