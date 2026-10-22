# Source-only checks

This check set is intended for a fresh source clone. It validates the app's TypeScript/React code and frontend build without using the local engine payload or proof-runner files under `work/`.

## Toolchain and commands

Use Node.js `25.2.1` from [`.node-version`](../../.node-version) and pnpm `11.25.0` from the `packageManager` field in [`package.json`](../../package.json). With network access to install the locked JavaScript packages, run:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm test:app
pnpm build
```

`test:app` names all six files explicitly so the command does not depend on PowerShell or shell glob expansion. They currently contain 49 Node tests in total. `build` runs `tsc -b` followed by Vite; a separate `pnpm typecheck` in the same gate would repeat the TypeScript check.

The test and build steps use the checked-in app sources, package manifest, and lockfile. Package installation requires the package registry. The steps do not use provider credentials, client configuration, a running engine, downloaded runtime payloads, proof-helper modules, or files beneath `work/` as inputs. The pnpm store path keeps downloaded packages inside the project workspace.

## Manual GitHub Actions run

`.github/workflows/source-checks.yml` has only a manual `workflow_dispatch` trigger. Its `free_only_confirmed` boolean defaults to false, and the job is skipped unless it is explicitly true. Before dispatching, the operator must confirm the current free standard-runner allowance and account hard stop in GitHub. The checkbox is a human confirmation; the workflow does not query live billing, remaining minutes, or payment settings.

The job uses one standard `ubuntu-24.04` x64 runner, a ten-minute job limit, read-only repository permission, and concurrency cancellation for duplicate runs on the same ref. Checkout and Node setup actions use immutable commit pins. Node is read from `.node-version`; pnpm 11.25.0 is installed under `work/ci-tools` with scripts disabled, and npm/pnpm stores stay under `work/`. No GitHub Actions dependency cache, build artifact, release, deployment, provider credential, or paid/larger runner is configured. The workflow captures command output in step memory and emits fixed success labels plus a numeric test count when Node's summary format is recognized; failure output is discarded and only a fixed command label and exit code are reported.

The [first manual run](https://github.com/Protocol272/headroom-desktop/actions/runs/39759872053) passed 49 app tests and the frontend build on 22 October 2026 (job 21s, total 29s); [H047 evidence](../proofs/free-ci.md) records the free-only account gate and post-run observation. This establishes only the source-only frontend checks described above. It does not establish Tauri/Rust native behavior, engine packaging or readiness, client/provider compatibility, macOS/Linux native support, or Windows 11/clean-machine support.

## Excluded checks and limits

This gate does not run the historical Python protocol/client proof helpers: several import helper modules from ignored `work/proofs/...` paths. It does not run the Python engine smoke or `scripts/package-engine.ps1`; those need the separately provisioned Windows Python/runtime payload and packaging inputs described by [`runtime/manifest.json`](../../runtime/manifest.json). It does not compile or test the Tauri/Rust native backend.

The Windows runtime and filesystem transaction paths are platform-specific. Non-Windows Rust fallbacks report unsupported behavior, so a Linux or macOS frontend check is not evidence of native runtime support on those systems. Windows 11 and clean-machine engine startup remain separate validation items. This source gate alone makes no engine, client-compatibility, installer, or cross-platform support claim.
