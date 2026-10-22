# Headroom Desktop

Headroom Desktop is a local-first desktop application for managing a pinned Headroom engine and supported coding-client connections. The project is still under development; its current evidence is local development evidence, not a published or clean-install release.

## Current state

As of 22 October 2026, H018's configuration apply/undo path has local acceptance on the Windows 10 development host. H019, the first real Codex traffic path, has not started. This does not establish live provider compatibility, Windows 11 support, or a release-ready installer.

The app uses Tauri 2, Rust, React, and TypeScript. Local source checks use the checked-in package and Cargo lockfiles. On a machine with the required Node.js/pnpm and Rust toolchains, the frontend commands are:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm typecheck
pnpm build
```

These commands check the frontend source only; they do not prove an engine is available or that a fresh clone can start the desktop runtime. The current engine package is a large, development-only payload. [`runtime/manifest.json`](runtime/manifest.json) points to `../work/proofs/packaging/artifacts/headroom-engine/runtime`, which is excluded from source import. The local H006 proof measured that payload at 523,394,231 bytes. The payload is not included in this repository, and no bootstrap or release-download flow is provided here.

Some historical protocol fixture helpers also import runner modules from ignored `/work/` paths. Those proof helpers are not fresh-clone checks yet; H047 must make the intended source-only checks reproducible before CI is enabled. No helper or payload was copied into this repository as part of H046.

Consequently, a fresh clone is not engine-ready. The current runtime proof is limited to the recorded Windows 10 host; it makes no macOS or Linux support claim. See [the local runtime proof](docs/proofs/desktop-runtime.md) and [the H018 transaction proof](docs/proofs/config-transaction.md) for scope and limits.

## GitHub and evidence

The project is maintained in the private, source-only [Protocol272/headroom-desktop](https://github.com/Protocol272/headroom-desktop) repository. The first import was published through GitHub Desktop; [H046 evidence](docs/proofs/repository-import.md) records the scope. No CI workflow is configured or has been run. The free-only CI decision and gates are recorded in [decision 006](docs/decisions/006-free-ci.md); automated checks remain deferred until H047 verifies the account's applicable quota and hard-stop conditions.

Some proof documents link to files beneath `work/`. Those paths refer to local development evidence and are intentionally ignored; the evidence files themselves are not part of the initial source import. Generated builds, downloaded engine payloads, temporary runs, and visual outputs are excluded from Git.
