<a name="h036--uyum-kataloğu-ve-güncelleme-davranışı"></a>

# H036 — Compatibility catalog and update behavior

8 November 2026. **Local task acceptance complete.** Windows 10 22H2 x64, build 19045, development version 0.1.0, and engine pin 0.41.0. At the user's explicit request, H036 ran while the manual tray check in H035 remained pending. H035 stayed open. The product total was 34/51. [Plan 024](../decisions/024-compatibility-catalog.md), [lead review](../reviews/H036.md).

The Compatibility section on the Applications screen starts closed. It shows found installations separately from 12 historical test records. It shows version, CLI/Desktop surface, account method, OS, engine, and evidence date. It separates live-verified scope from experimental local tests, not tested, unknown, and unsupported states. It keeps partial or blocked sub-capabilities in the details.

A historical record does not mean that the current account or an open chat was verified. No model allowlist was added. No model, effort, or agent change was made.

<a name="davranış-ve-kanıt"></a>

## Behavior and evidence

| Scenario | Expected and observed result | Evidence | Limit |
| --- | --- | --- | --- |
| Open Compatibility while the engine is stopped | Real IPC returned 12 embedded records. The engine stayed stopped. | Native `stopped-read.json` | This checked catalog reading, not a live provider. |
| Read, scan, and refresh with the engine ready | The engine kept the same generation and version 0.41.0. Old records stayed hidden during and after the scan. Refresh showed the new reply for the second scan. | Native `ready-before/ready-read/during-scan/stale-scan/refreshed.json` | A later external change requires a new scan or refresh. |
| Late reply, changed installation/config, or malformed reply | A late reply did not become the result for a new context. A metadata change made the result stale. No new supported-version evidence means the installation stays unverified. User edits to client config changed the fingerprint in all three adapters. Files and auth canaries stayed intact. | Rust discovery/config tests, strict client, and real-App UI | Fixture changes are not real account or provider support. |
| Wrong engine pin and fallback | Older, newer, and floating pins were rejected. The original full pin was accepted again. Catalog reading did not update or change processes. | `compatibility_engine_pin_rejects_version_transition_and_accepts_restored_pin`, native same-generation check | Downloads, installed-version changes, and installer rollback remain outside this test. H042 remained open. |
| Keyboard and normal exit | Tab/Space closed the details. Real Sky exit/confirmation completed with exit0. State and real HKCU Run stayed unchanged. Only owned ports closed. The foreign listener stayed open. | Native `keyboard-closed/exit-dialog/result.json` | This does not accept the H035 tray menu. |

The final native run is `work/proofs/compatibility/native-runs/c9259b0a-b9d3-4ad4-aea5-9740fc81a28c/`. Sky supplied real Windows input. CDP read-only observation compared the committed public reply with the strict decoder and canonical source. It did not call IPC or change the reply or native API.

Model/provider calls: 0. No auth login or current client setting write occurred. The helper process completed normally, not by force. The last window list did not contain Headroom. The test engine did not remain running.

<a name="son-adayın-yerel-kapısı"></a>

## Final candidate local gate

| Check | Result | Programmatic record |
| --- | --- | --- |
| `pnpm.cmd test:app` | 173 PASS, including 5 new compatibility tests | `work/proofs/compatibility/post-review-final.json` |
| `pnpm.cmd build` | TypeScript + Vite PASS | Same record |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS | `reused-rust.json` → `final-1.json` |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | 386 PASS, 8 existing opt-in ignored | Same unchanged Rust inputs and tool/OS binding |
| `cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol` | Native build PASS with the final frontend | `post-review-final.json` |
| `pnpm.cmd test:a11y` | 3 scenarios PASS. 84 audits, 0 violations. Across 48 navigation samples, p95 was 21.8 ms against a 250 ms limit. | `work/proofs/accessibility/ui-5c51934c-954b-4f74-8997-ee61ebad2fb9/result.json` |
| Combined H036 acceptance | 28/28 PASS. Source/artifact/evidence and tool/OS before/after matched. | `work/proofs/compatibility/acceptance.json` |

The final scan-busy fix changed only frontend and isolated fixture/test files. The frontend, build, and UI gates were therefore refreshed. The unaffected 386 Rust/fmt result was bound again programmatically to the same inputs and toolchain. The checks automatically record source, catalog/manifest, EXE/dist/helper, and referenced-evidence hashes. No hashes are copied by hand.

Tools: Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0. The lead agent reviewed the final TR-dark and EN-light images. Automation covered all four language/theme combinations and narrow viewports. A viewport result is not a real OS DPI result.

The first fixture/UI runs failed because two discovery fields were old and an asynchronous readiness race occurred before the audit. The fixture contract and deterministic wait fixed those issues. The first native run showed old records during a scan. The final candidate fixed that issue. Old runs do not count as current acceptance. The thresholds did not change. Successful test-harness cleanup and state restoration did not hide the product issue.

<a name="destek-ve-yayın-sınırı"></a>

## Support and release limits

The Live verified label applies only to the catalog's recorded Codex scopes for ChatGPT on Windows 10 with CLI 0.166.0 and Desktop 26.1027.6604.0. The real API-key row was not tested. Claude/OpenCode synthetic local fixture rows were not promoted to live success. This task did not verify a new provider, model, or subagent. Windows 11/macOS/Linux, clean install, signed release, long-term stability, and live performance were not verified.

README, application version, runtime manifest pin, and adapter source fingerprints match. The channel is development. Automatic update and installed-version switching are not offered. Real signing/download/install/migration/rollback remain open in H042. The installer remains open in H037. Live API-key work remains open in H041.

Earlier H035 performance/DPI records prove their historical candidates. The old combined H035 acceptance is not current evidence for the new H036 binary/UI. When work returns to H035, only the affected scope needs review.

The lead agent retained implementation and acceptance. The single Luna worker produced this read-only evidence report, reached its usage limit, and stopped. The lead agent checked the source and took over. No other worker or model was opened. Development token savings and worker-time gains were not measured.
