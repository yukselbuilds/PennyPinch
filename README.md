# Headroom Desktop

Headroom Desktop is a local-first desktop application for managing a pinned Headroom engine and supported coding-client connections. The project is still under development; its current evidence is local development evidence, not a published or clean-install release.

## Current state

As of 3 November 2026, H023 content-free metrics normalization and its pinned-engine metadata adapter are accepted in local Windows 10 development checks; unlabelled legacy measurement pairs remain unknown. The H008 current-Codex slice now recognizes signed CLI0.166.0, backend0.166.0-alpha.5.1 and Desktop26.1027.3384.0, and supports guarded CLI/Desktop user-config Apply/Undo. A native product-verifier CLI response completed through Headroom with existing ChatGPT auth and normal model selection; two extra CLI notices remain unclassified. The final native gate passed289 tests (5 ignored), format and embedded build;98 matching frontend tests/build are reused. See the [current Codex proof](docs/proofs/codex-current.md), [metrics proof](docs/proofs/metrics-normalization.md), [tasks](TASKS.md) and [checkpoint](docs/IMPLEMENTATION_STATUS.md). H008 remains open for the notices, earlier shutdown observations, actual Desktop adoption and agent/auth coverage. This is not a live-savings dashboard, universal-provider, additional-OS or installer release claim.

The app uses Tauri 2, Rust, React, and TypeScript. The source-only frontend gate uses the checked-in package and lockfiles. Use Node.js 25.2.1 (recorded in `.node-version`) and pnpm 11.25.0:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm test:app
pnpm build
```

`test:app` runs the app contract tests listed in `package.json`; `build` includes the TypeScript project check before building the frontend. These commands check the frontend source only; they do not prove an engine is available or that a fresh clone can start the desktop runtime. The exact scope and external inputs are listed in [source checks](docs/validation/source-checks.md). The current engine package is a large, development-only payload. [`runtime/manifest.json`](runtime/manifest.json) points to `../work/proofs/packaging/artifacts/headroom-engine/runtime`, which is excluded from source import. The local H006 proof measured that payload at 523,394,231 bytes. The payload is not included in this repository, and no bootstrap or release-download flow is provided here.

Historical protocol fixture helpers still import runner modules from ignored `/work/` paths, and the Windows runtime smoke requires the absent engine payload. They are outside the frontend gate and are not fresh-clone checks. No helper or payload was copied into this repository as part of H046.

Consequently, a fresh clone is not engine-ready. The current runtime proof is limited to the recorded Windows 10 host; it makes no macOS or Linux support claim. See [the local runtime proof](docs/proofs/desktop-runtime.md) and [the H018 transaction proof](docs/proofs/config-transaction.md) for scope and limits.

## GitHub and evidence

The project is maintained in the private, source-only [Protocol272/headroom-desktop](https://github.com/Protocol272/headroom-desktop) repository. The first import was published through GitHub Desktop; [H046 evidence](docs/proofs/repository-import.md) records the scope. H047-a observed the GitHub Free plan and an account usage hard stop. The manual source-only [first CI run](https://github.com/Protocol272/headroom-desktop/actions/runs/39759872053) passed 49 app tests and the frontend build on a standard Ubuntu runner. [H047 evidence](docs/proofs/free-ci.md) records the fresh-source check, account hard stop and observed $0 billable usage; native platform support remains separate. The free-only CI decision and gates are recorded in [decision 006](docs/decisions/006-free-ci.md), and the workflow's scope and dispatch requirements are documented in [source checks](docs/validation/source-checks.md).

Some proof documents link to files beneath `work/`. Those paths refer to local development evidence and are intentionally ignored; the evidence files themselves are not part of the initial source import. Generated builds, downloaded engine payloads, temporary runs, and visual outputs are excluded from Git.
