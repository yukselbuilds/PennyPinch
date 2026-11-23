# Source-only checks

This check set is intended for a fresh source clone. It validates the app's TypeScript/React code and frontend build without using the local engine payload or proof-runner files under `work/`.

## Toolchain and commands

Use Node.js `25.2.1` from [`.node-version`](../../.node-version) and pnpm `11.25.0` from the `packageManager` field in [`package.json`](../../package.json). With network access to install the locked JavaScript packages, run:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm test:app
pnpm build
```

`test:app` enumerates its contract-test files explicitly in `package.json`, so the command does not depend on PowerShell or shell glob expansion. Current local evidence is recorded in the [implementation checkpoint](../IMPLEMENTATION_STATUS.md). The historical H019 candidate contained 73 Node tests; the final local result is recorded in its [product verification proof](../proofs/codex-product-verification.md). `build` runs `tsc -b` followed by Vite; a separate `pnpm typecheck` in the same gate would repeat the TypeScript check.

The test and build steps use the checked-in app sources, package manifest, and lockfile. Package installation requires the package registry. The steps do not use provider credentials, client configuration, a running engine, downloaded runtime payloads, proof-helper modules, or files beneath `work/` as inputs. The pnpm store path keeps downloaded packages inside the project workspace.

## Manual GitHub Actions run

`.github/workflows/source-checks.yml` has only a manual `workflow_dispatch` trigger. Its `free_only_confirmed` boolean defaults to false, and the job is skipped unless it is explicitly true. Before dispatching, the operator must confirm the current free standard-runner allowance and account hard stop in GitHub. The checkbox is a human confirmation; the workflow does not query live billing, remaining minutes, or payment settings.

The job uses one standard `ubuntu-24.04` x64 runner, a ten-minute job limit, read-only repository permission, and concurrency cancellation for duplicate runs on the same ref. Checkout and Node setup actions use immutable commit pins. Node is read from `.node-version`; pnpm 11.25.0 is installed under `work/ci-tools` with scripts disabled, and npm/pnpm stores stay under `work/`. No GitHub Actions dependency cache, build artifact, release, deployment, provider credential, or paid/larger runner is configured. The workflow captures command output in step memory and emits fixed success labels plus a numeric test count when Node's summary format is recognized; failure output is discarded and only a fixed command label and exit code are reported.

The [first manual run](https://github.com/Protocol272/headroom-desktop/actions/runs/39759872053) passed 49 app tests and the frontend build on 22 October 2026 (job 21s, total 29s); [H047 evidence](../proofs/free-ci.md) records the free-only account gate and post-run observation. This establishes only the source-only frontend checks described above. It does not establish Tauri/Rust native behavior, engine packaging or readiness, client/provider compatibility, macOS/Linux native support, or Windows 11/clean-machine support.

## Optional Linux desktop check

Select `linux_desktop` in Source checks to add the native Ubuntu check. The job limit increases to 25 minutes.
This check runs isolated configuration and transaction tests on ext4. It builds the desktop app and opens its window in Xvfb.
Select `linux_config_transaction` to run the standalone file system probe. The probe tests rename behavior and process interruption.
[Linux transaction evidence](../proofs/h049-linux-transaction.json)
Use the standard runner in the public repository. [GitHub runner billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions) · [Linux evidence](../proofs/h049-linux-config.json)

Select `linux_cli_discovery` to check native CLI discovery and saved choices.
The test checks archive integrity and extracts the three pinned Linux x64 executables.
It scans a private home and PATH without launching the clients. [Discovery evidence](../proofs/h049-linux-discovery.json)

## Optional Linux engine check

Select `linux_engine` to verify the pinned engine on Ubuntu 24.04 x64.
The job verifies the archive hashes and installs the frozen binary wheels.
It tests the relocated engine against a synthetic Responses provider in a loopback-only network namespace.
The job checks socket peer identity, stream completion, shutdown, and restart.
[Engine evidence](../proofs/h049-linux-engine.json)

Select `linux_managed_engine` to stage the pinned resources and run the desktop manager tests.
This option also runs the isolated configuration checks and opens the native window under Xvfb.
The provider fixture and manager tests use a loopback-only network namespace.
[Managed engine evidence](../proofs/h049-linux-managed.json)

## Excluded checks and limits

These limits apply to the default frontend run.

This gate does not run the historical Python protocol/client proof helpers: several import helper modules from ignored `work/proofs/...` paths. It does not run the Python engine smoke or `scripts/package-engine.ps1`; those need the separately provisioned Windows Python/runtime payload and packaging inputs described by [`runtime/manifest.json`](../../runtime/manifest.json). It does not compile or test the Tauri/Rust native backend.

The Windows runtime and filesystem transaction paths are platform-specific. Non-Windows Rust fallbacks report unsupported behavior, so a Linux or macOS frontend check is not evidence of native runtime support on those systems. Windows 11 and clean-machine engine startup remain separate validation items. This source gate alone makes no engine, client-compatibility, installer, or cross-platform support claim.
