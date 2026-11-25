# Development workflow and task acceptance

3 November 2026. This workflow governs tasks and development tooling in `TASKS.md`. The product specification defines scope and `AGENTS.md` sets fixed boundaries. Apply this workflow to future acceptance and preserve the scope of historical evidence. Instruction maintenance leaves product-task status unchanged.

## Tool selection

| Need | Tool |
| --- | --- |
| Known file or symbol | Start with `rg` and read the necessary lines. |
| Callers, dependencies or change impact | Ask CodeGraph one focused question. Start with `maxFiles: 1` and extend to two or three when needed. Verify results against the source. |
| Implementation or fixes | Use the project `ponytail` skill at `full` level. Make the smallest complete change that preserves existing structure and required checks. |
| Accepting a change | The lead agent uses `ponytail-review` to inspect the diff, affected callers and tests in the regular quality review. |
| Auditing historical code | Use `ponytail-audit` for a one-time report on the defined scope. Avoid repeating a repository-wide audit for individual tasks. |
| Compressed or missing tool output | Retrieve the necessary original excerpt through `headroom_retrieve`. Use the original source and test results as evidence. |

Update the CodeGraph index when source or exclusion settings change, with one updater at a time. Use `scripts/codegraph.ps1` if the MCP tool is unavailable. Use the graph for discovery and verify Tauri IPC, real provider traffic and agent identity independently. [Installation and commands](codegraph.md)

Ponytail applies to the lead agent and single worker. Write repository reports in English and communicate with the repository owner in Turkish. The default level is `full`. The skill provides development guidance. Preserve security, ownership, atomic writes, conflict-safe rollback, error handling and required tests when simplifying. [Pinned source and usage](ponytail.md)

## Starting a task

1. Find the active task ID, last evidence and next missing behavior in `TASKS.md` and the short checkpoint. Reuse current sources and documents already in context. Refresh the relevant parts when inputs change, context is missing or an uncertainty needs resolving.
2. Check the plan or specification for the goal and exclusions, contracts to preserve, critical assumptions and verification path. Update only missing or conflicting decisions. Describe normal, error and boundary behavior in up to three observable scenarios with a starting state, action and expected result. Use a separate plan file when the task needs one.
3. Select the implementer and briefly explain why. The lead agent owns critical authentication, route, ownership, measurement and recovery decisions and unresolved performance causes. Delegate clear, verifiable work that justifies a handoff to the single Luna xhigh worker. Keep small tasks together.
4. Keep each slice to one behavior and acceptance scenarios that can be verified together. Roughly one to five files is a handoff guide. Use the short handoff below for Luna. Assign one writer to each area. The lead agent stops the worker and confirms it has stopped before taking over.

```text
Task ID and goal
Implementer and brief rationale
Starting state and expected behavior
File ownership, exclusions and contracts to preserve
Acceptance scenarios
Critical assumptions and relevant sources
Existing verification commands and reusable evidence
Report changes, results and fingerprints, unknowns, and the next action.
```

Before implementing, check the affected interfaces, storage, measurement and the contract from UI to manager to engine or client. Start with existing helpers and standard structures. Add a dependency, abstraction or setting when the task has a concrete need for it. Keep fixes and broad cleanup in separate slices.

The worker stops decision-dependent edits and reports the observation, attempted approach and decision required when a contract conflicts, a retry produces no new evidence or acceptance would need to be weakened. The lead agent sends one set of corrections. If the problem persists, take over or narrow the work while preserving acceptance. Reopen a worker from the checkpoint only when the original session ends or the handle is lost. Keep the worker and model through timeouts. Review the actual diff and connected path alongside the worker's summary. Cosmetic preferences do not warrant another review cycle.

<a name="bir-görevin-bitiş-kabulü"></a>

## Task completion criteria

Mark a main task `[x]` when all five conditions hold:

1. **Behavior.** The expected result was demonstrated in all acceptance scenarios. For a UI change, review the path from the UI through the manager to the engine or client. Mock evidence applies to the scenario exercised by the mock.
2. **Errors and scope.** Verify affected cancellation, failure, conflict and restart scenarios. Reuse evidence for unchanged paths when it remains valid. Respect model, authentication, project and process ownership boundaries.
3. **Review.** The lead agent reviewed the change and relevant callers. Concrete findings that would break acceptance have been fixed. Record optional cleanup separately from verified defects.
4. **Verification.** Confirm and run the existing plan command, or use TASKS criteria and the local gate below. Focused checks passed. For product code, run the full local gate once on the final candidate after review and fixes. For docs, skills or tool configuration, verify the affected setup, configuration and connection as needed. Instruction-only maintenance calls for checks of the affected instructions. A task remains partial while critical external evidence is missing.
5. **Record and stop.** Keep the task list and checkpoint consistent about status, evidence, limits and the next action. Tell the user when the main task is complete, stop and wait. Report acceptance of an individual slice at its own scope.

Keep the evidence record short and include the scenario, expected and observed results, evidence path, source and configuration plus tool and environment fingerprints, and limits. Capture results and before-and-after fingerprints programmatically in the same step. State a limit when the tooling cannot capture it. Keep raw prompts, authentication data, HTTP bodies and error dumps out of persistent evidence.

Reuse a PASS only when the workspace, relevant source, tests, configuration, dependencies, toolchain, command and environment match. Name the concrete change before repeating a previously successful check. Compaction, a new day or interrupted credits carry forward valid evidence. Fixes invalidate related evidence. Refresh live connection, CI, deployment, migration, database and production checks at their stage. Define performance inputs, operating state, load, duration and thresholds before running. Long-term reliability needs corresponding evidence. Keep the acceptance threshold when a check fails.

## Task status

| Status | Meaning |
| --- | --- |
| Planned | Implementation has yet to start. Keep the box open. |
| Active | The current main task or development tooling work undergoing acceptance. |
| Partial or awaiting external evidence | Record completed local work and leave the missing evidence open. Continue independent work. |
| Complete | The task meets its scope and all shared completion criteria. |

Record an unavailable account or second OS in the support matrix. Use `not_tested` or unknown for an unverified case. Record local success, provider verification, Windows 11, macOS and Linux success and release-candidate readiness individually. Audit read-only and scope repairs separately. Resolve critical correctness or data-loss findings before dependent work. Planned features and optional suggestions are separate from defects. Track findings on the relevant open task or create a small fix item for an independent concrete defect.

<a name="bugün-çalışabilen-yerel-ürün-kapısı"></a>

## Available local product gate

These commands match the existing `package.json` and the H021 evidence. For new code acceptance, run the final candidate gate from the repository root.

```text
pnpm.cmd test:app
pnpm.cmd build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

Set process-scoped `CARGO_HOME=<repo>/work/caches/cargo-home` and `CARGO_TARGET_DIR=<repo>/work/caches/cargo-target` for Cargo. Use the recorded toolchain and existing offline cache. The build includes TypeScript checking, so one pass covers it for the candidate.

The current pnpm scripts include the checks above. Add `lint`, `test` or `test:e2e` to the gate when the corresponding script exists. Verify the native user flow separately for UI behavior. This gate covers the debug embedded product. Installer, update and release acceptance have separate gates in H038 and H039. CI, push and release are handled in their own stages under the free-use limit.

H035 and H036 have additional checks. `pnpm.cmd test:a11y` runs the real App with isolated transport in headless Edge and requires the browser in the test environment. Real OS display scaling has its own verification. `pnpm.cmd test:compatibility` focuses on the strict catalogue client and discovery fixture contract, with the same tests included in `test:app`. Verify the native product separately.

`pnpm.cmd build:windows` verifies the pinned payload against the full inventory under `work/build/windows/pennypinch` and builds a current-user NSIS preview. It requires the pinned project Python and engine payload. Run the focused packaging tests with that Python and `-I -B -m unittest discover -s tests/runtime -p 'test_windows_*.py'`. Create the delivery ZIP bound to the verified 0.44.0 receipt with `-I -B scripts/package-windows-delivery.py --receipt docs/proofs/windows-installer-044-notices.json`. The opt-in Rust test `windows_bundle_real_staged_payload_starts_and_stops_without_development_layout` uses actual staging resources. Observe installation, resource parity, native engine operation and normal exit separately. Record clean Windows 10, Windows 11 and a host without WebView2 individually. Tests use isolated client settings. [Windows validation](../validation/windows-install.md)

H038 verified uninstall on Windows 10 with real NSIS and an isolated application identifier. Reuse existing product-gate results when inputs match. For an uninstaller change, observe a running app, a locked or conflicting config, and the retain and delete choices for local data. Unattended uninstall retains data. Update has its own acceptance. [Uninstall validation](../validation/uninstall.md)

For the H039 Windows release check, `build-windows.ps1` points `HEADROOM_RELEASE_EXE_CANDIDATE` at the owned release executable after packaging, runs `test_windows_release_executable.py` and restores the previous process environment. The test reads the actual file's PE, x64, PE32+ and GUI subsystem=2 fields. The Windows release build remaps the checkout and profile paths during Rust compilation. The test checks the executable for both original paths. A direct run without that environment variable explicitly skips the check and leaves release acceptance open. [Release candidate evidence](../validation/release-candidate.md)

Install the pinned administrator dependency with `pnpm --dir services/reports/admin install --frozen-lockfile` from the repository root. Run `node --test` from `services/reports` with Node.js 24. The suite tests the reporting service, scheduled cleanup, administrator access, and restore cleanup. Verify the deployed Neon trigger, database permissions, managed sign-in, and restore in the pilot. [Restore evidence](../proofs/h052-neon-restore-local.json)

## Historical code audit scope

The initial DEV002 report prioritizes configuration Apply and Undo, ownership, runtime and proxy, and UI connection paths. Use CodeGraph to find relationships and read the relevant source. Each finding includes the file and line, conditions, user impact and smallest fix. State what the audit covered. The audit records findings and leaves fixes and task acceptance to their own work.

Report development token savings only when actual measurements use the same scope. Ponytail benchmarks, CodeGraph call counts and deleted lines measure different things.
