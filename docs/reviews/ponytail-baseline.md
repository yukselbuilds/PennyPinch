<a name="dev002--mevcut-kodun-ilk-ponytail-denetimi"></a>

# DEV002 — Initial Ponytail code audit

26 October 2026. The source baseline was `0a031d2`. The lead agent and the existing single Luna worker applied `ponytail-audit` and the project acceptance criteria. Product source, settings, and tests stayed the same.

<a name="bu-depo-ne-yapıyor"></a>

## Repository scope

Headroom Desktop manages a local engine and proxy connections for supported coding clients. It previews and applies configuration. It uses its ownership record to undo its own changes.

The audit assumed one user on one computer. It considered concurrent client and subagent connections, multiple app processes, and later changes to client settings. The relay defaults to 32 concurrent TCP connections. This limit applies to connections. HTTP request counts have a separate scope.

<a name="bulgular--düşük-öncelikli-belge-düzeltmesi"></a>

## Findings

<a name="1-dev002-d01--readme-güncel-geliştirme-durumunu-anlatmıyor"></a>

### 1. DEV002-D01 — Outdated README summary

**Source.** The review compared `README.md:7` and `README.md:17` with the status in `TASKS.md` and `scripts.test:app` in `package.json`.

- **Purpose.** The README gives new developers the current product state and the checks to run.
- **Condition.** The Current state section described H018 and H019 as of 22 October. The team accepted H021 before this audit. The README also said “all six app test files”. The command listed nine `.test.mjs` files. The date made the old entry valid as a historical record. The current summary needed an update.
- **Smallest fix.** Link the H022 documentation update to TASKS and the checkpoint. Keep the limited amber result for H019. Describe the command's scope. Let the command define the current test count. Keep the first CI run's historical result of 49 tests separate.
- **Impact.** A new developer could mistake completed work for planned work. The developer could also misunderstand the source gate. This finding concerns the documentation.

**Confidence.** High. The review compared the files and the nine entries in the command directly. **Follow-up.** H022 tracks the fix. This audit left it open.

The priority paths yielded no confirmed new crash, data loss, or ownership violation. The audit covered the inspected paths. Release readiness needs its own acceptance.

<a name="izlenen-akışlar-ve-doğrulama-düzeyi"></a>

## Inspected paths and verification scope

| Area | Path reviewed | Result and limit |
| --- | --- | --- |
| Config Apply | `ConnectDialog.applyPlan` → TS action client → Tauri `apply_connection_plan` → `PlanManager` → config and ownership transaction | The UI sends the plan ID and fingerprint. The audit checked the native preconditions and the transaction ownership boundary. Real user config remained outside the review. |
| Undo and ownership | `undoOwnership` → `undo_connection` → ownership revision and fingerprint check → transaction seal and commit | The transaction handles config and ownership together. The audit checked the revalidation around `connection_actions.rs:879` and `transaction_fs.rs:350`. It found no confirmed path to false success. |
| Preferences | `App.changePreferences` → preferences command → `PreferenceStore` and `save_document` | The audit checked revision and content fingerprints, the cooperative process lock, bounded file reads, and temporary file replacement. Atomic CAS covers writers that use the lock. |
| Runtime | Tauri start, stop, and exit → `RuntimeManager` actor → `ManagedIngress` routing and drain | The audit checked active generation, uncertain cleanup, shutdown, and restart. A full review of the Windows process, Job Object, and manifest launch helpers remained outside this scope. |
| Relay | Accept and register → fixed backend target → peer check → bounded two-way buffer → drain and shutdown | The audit checked connection limits, backpressure, half-close, timeouts, and the guard against payload replay. This review used source evidence for traffic and performance paths. |
| Interface | `App`, `Applications`, `ConnectDialog`, and `ConnectionStatus` action, sequencing, and mutation paths → TypeScript bridges → native records | The audit checked late replies and mutation locks. Visual design, all JSX and copy, and every decoder combination needed their own review. |

The lead used CodeGraph to find `ManagedIngress`, `RuntimeManager.stop_engine`, and their callers. The audit checked the relationships in source. A graph summary that says it found no tests supplies no evidence of test coverage. The lead used `rg` and narrow source reads for known files.

<a name="var-olan-görevlerde-kalan-konular"></a>

## Issues on existing tasks

- **H022.** The task already covers safe Claude project folder creation, remembered selection, and picker start and fallback scenarios. The audit kept those scenarios under the existing task. It also tracks the DEV002-D01 documentation fix.
- **H023, H025, and H026.** In the earlier H021 Chat fixture, `original=2306` and the synthetic provider value `optimized=12` use different measurement bases. The existing tasks own normalization and real platform, model, and agent display. A saving requires comparable measurements.
- **H008.** The historical amber result for H019 remains open. The CLI returned a real reply and an additional unclassified error. The audit retained that result. A new provider attempt remained outside this audit.

<a name="bulgulaştırılmayan-sınırlar"></a>

## Limits outside the findings

A writer that bypasses the cooperative lock could theoretically write between the final fingerprint check in `state/store.rs:547` and replacement at `:566`. The contract in `docs/proofs/preferences-store.md:7` covers cooperating processes. The external edit test at `store_tests.rs:350` changes the file before save starts. Normal theme and language use yielded no confirmed loss or security issue. A broader guarantee would need a separate threat model and a real race test.

The general race hypothesis around path and reparse checks needs concrete evidence of a write outside the project. The audit kept the security checks. It found no dependency or code that it could safely remove. It made no estimate of line or dependency savings.

<a name="kanıt-ve-kontrol-kapsamı"></a>

## Evidence and check scope

The config and state review covered `transaction.rs`, `transaction_fs.rs`, `connection_actions.rs`, `config_io.rs`, `config_document.rs`, `state/ownership.rs`, and `state/store.rs`. The audit checked the transaction, action, config, and ownership test contracts. It sampled the relevant preference store test bodies.

For runtime, the lead checked accept, forward, and drain in `ingress_relay.rs`. The ManagedIngress, RuntimeManager, and actor sections in `manager.rs` were at 2963–3314, 3338–3632, and 3722–4111. The UI review checked component actions and effects. The audit also checked relevant manager ownership and cleanup, relay generation and drain, half-close, and TypeScript sequencing tests.

The local receipt `work/proofs/ponytail-baseline/audit.json` records the source, test and config fingerprints, README comparison, and matching inputs for the earlier gate. The audit reused prior gate evidence because product code stayed the same. This review reports source checks. Native, UI, and provider behaviors retain their separate evidence.

**Decision. The DEV002 audit report is complete.** H022 is next, with H008, H022, and H023 follow-ups preserved. H022 owns the small DEV002-D01 documentation fix. Work stopped at the end of the task.

The following remained outside the review scope.

- New live OAuth and API-key provider calls.
- Actual traffic from other chats or subagents.
- A new native GUI session.
- Clean install, installer and uninstaller, Windows 11, macOS, and Linux.
- All packaging and CLI helpers, all third-party engine and dependency source, and current security advisories.

Development token savings remained unmeasured.
