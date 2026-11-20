<a name="h036--uyum-kataloğu-ve-güncelleme-davranışı"></a>

# H036 — Compatibility catalog and update behavior

8 November 2026. While awaiting the manual H035 tray check, the user explicitly requested moving to the next task. This is an order exception for H036 only. H035 remains open and the product total stays 33/51. Stop when H036 is complete. The lead agent owns evidence/auth/version trust, native work, and acceptance. The same single Luna worker first performs read-only evidence matching. A frontend slice may be assigned separately once its contract is settled. Use Ponytail full and the existing Emil/Graphite guidance.

<a name="amaç-ve-kapsam"></a>

## Purpose and scope

Applications has an optional “Compatibility” detail that starts closed. Recorded validations are shown with their version/surface/auth/OS and evidence scope. Live-verified, local-fixture/experimental, unsupported, and not-tested states remain distinct. Matching metadata does not verify an existing chat/auth/agent. No new model allowlist or automatic model/effort change is added.

Bundle `compatibility/clients.json` into the app. Preserve historical records. Correct outdated pause/native-wiring and README status using current evidence. Cross-check the runtime pin against the real `runtime/manifest.json`, native version constants, and adapter-code fingerprints. The channel is “development.” Do not imply that a distributed stable/preview release exists.

Discover clients through the existing read-only `recovery_scan` and observe config through the existing ownership/scope-preserving `recovery_config`. Do not return or persist config contents. Use only an opaque context fingerprint compared in memory. Do not show an old result as current during or after a new scan. Notify the user to reverify when installation metadata/version or owned config/layer/environment changes. Lower confidence when scope is unknown. No config/model/auth writes, client execution, or account sign-in are required.

This task adds no engine install/update button. Reading the active engine/catalog does not change it automatically. Preserve existing version/hash guards. Verify version-transition safety by rejecting an incompatible pin and accepting the previous exact pin when restored. Real installer/manifest signatures, downloads, and workable rollback between installed versions are separate H042 acceptance. Do not present them as ready in this task.

<a name="sıralı-dilimler-ve-kabul"></a>

## Ordered slices and acceptance

1. Native canonical catalog and read-only snapshot. Validate records and evidence paths. Bind an exact version/surface/OS match only to the relevant historical evidence. A fixture is not live auth success. A malformed catalog/request/race returns a public error or unknown and does not change settings/engine.
2. Strict client and calm, collapsible UI. Support TR/EN, themes, keyboard, and pending/error/unavailable/stale states. Show when an unknown or changed installation/context needs reverification. Allow only one refresh request at a time. Do not apply an old response to a new context. Do not add a stack of technical cards to the main screen.
3. Acceptance covers version/metadata/config-fingerprint-change fixtures, evidence for every recorded verified row, and consistency among runtime/adapter/channel/README. Reject a changed pin → restore the original pin → accept it, with the same active engine generation and normal owned cleanup. Do not repeat live-provider/account/second-OS checks. Preserve the historical scope of previous evidence.

Run focused Rust `compatibility`/related config and manifest tests, frontend compatibility-client tests, and real-App UI checks. Run the final candidate's full local WORKFLOW gate once. Record source/config/test/tool/environment and before/after fingerprints in the same verification step. Native UI uses the real get_compatibility_snapshot path. Record mocks only as isolated UI evidence. Do not repeat passing independent checks unless their inputs changed. Evidence is in `work/proofs/compatibility/`, the summary is `docs/proofs/compatibility.md`, and the review is `docs/reviews/H036.md`.

<a name="sonuç-8-kasım"></a>

## Result — 8 November

H036 was accepted with 173 frontend/386 Rust PASS and 8 existing opt-in ignored. Fmt/TS/Vite/native build passed, and the audit reported 84 checks/0 violations. Real native Sky read/scan/refresh and normal exit passed on the same engine generation. 28 checks verified source/artifact/tool/OS links. The stale-data-during-scan-busy finding was closed on the final candidate. The affected frontend/build/UI checks were refreshed, and the unchanged Rust result was programmatically rebound. [Evidence](../proofs/compatibility.md), [review](../reviews/H036.md).

The lead agent completed the implementation and acceptance. The same single Luna worker reached its usage limit after the read-only evidence report. No second worker/model change was made. No development savings were measured. Total is 34/51. H035 manual tray acceptance remains open, and H037 does not start. H035's historical performance/DPI evidence is not aggregate acceptance of the new binary. Evaluate relevant fingerprint scope when returning to the remaining work. Real downloads/installer/installed-version changes/rollback remain part of H042.
