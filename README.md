# PennyPinch

PennyPinch is a desktop app for local context optimization and token tracking across coding agents. The project is still under development; its current evidence is local development evidence, not a published or clean-install release.

PennyPinch uses the Apache-2.0 licensed Headroom engine for context compression. Thanks to the Headroom contributors. [Headroom project](https://github.com/headroomlabs-ai/headroom) · [Third-party notices](NOTICE.md)

## Start here

- [User guide](docs/USER_GUIDE.md) explains how to connect a coding agent, read token savings and check for updates.
- [Platform validation](docs/validation/desktop-platforms.md) tracks Windows, macOS and Linux coverage.
- [Releases](https://github.com/yukselbuilds/headroom-desktop/releases) is the distribution page for the first public preview.

## Current state

As of 13 November 2026, the local Windows 10 development app includes a measured-input dashboard, activity history, scoped client connections, safe pause/resume, tray controls, preferences, exports and read-only connection diagnostics. H008 was accepted with scoped ChatGPT evidence: Codex CLI0.166.0 and Desktop26.1027.6604.0/backend0.166.0-alpha.5.1 completed requests through the pinned engine; CLI parent/child routing is a separate recorded scope. Real API-key provider verification remains open in H041. Claude Code2.1.158 and OpenCode V1 1.19.3 product connections were tested with synthetic credentials and local upstream fixtures. See the [compatibility records](compatibility/clients.json), [Codex acceptance](docs/proofs/codex-native-wait.md), [Claude proof](docs/proofs/claude-product.md), [OpenCode proof](docs/proofs/opencode-product.md), [tasks](TASKS.md) and [checkpoint](docs/IMPLEMENTATION_STATUS.md).

H035 is accepted for the recorded Windows 10 development scope: accessibility, local performance, real display scaling and the user-confirmed tray menu. H036 provides the collapsed Compatibility section. It separates historical evidence by exact version, client surface, auth method and OS. An installation match does not verify the current account, conversation, model or subagent. Changed installation/settings context requires renewed verification; unknown scope is not silently labelled unsupported. Models are not selected or restricted by this catalogue.

H050 adds a local Accounts panel and explicitly confirmed official CLI sign-in. The Windows 10 native check observed a real Codex CLI stored ChatGPT record, synthetic OpenCode provider metadata, and an unknown Claude status because its status command writes settings. Local account records, Headroom routing and verified traffic remain separate. New provider sign-in completion, Desktop accounts and other-OS account adapters are not verified. See the [account evidence](docs/proofs/accounts.md). Open chats are the next H051 task; Linux H049 is deferred to preserve the user's remaining Actions quota.

The channel is **development**, app version0.1.0, engine `headroom-ai0.41.0`, embedded Python3.13.13. The engine manifest and adapter source fingerprints are pinned; automatic engine replacement is not offered. H042's signed application update, rollback and recovery passed isolated Windows 10 tests, and its local preparation command passed with a real Headroom QA executable. Production update hosting, the production signing key and Windows 11 x64 acceptance remain open; this build has no production update source. See the [update validation](docs/validation/windows-update.md) and [User guide](docs/USER_GUIDE.md). This is not a universal-provider, additional-OS or clean-install release claim.

The app uses Tauri 2, Rust, React, and TypeScript. The source-only frontend gate uses the checked-in package and lockfiles. Use Node.js 25.2.1 (recorded in `.node-version`) and pnpm 11.25.0:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm test:app
pnpm build
```

`test:app` runs the app contract tests listed in `package.json`; `build` includes the TypeScript project check before building the frontend. These commands check the frontend source only; they do not prove an engine is available or that a fresh clone can start the desktop runtime. The exact scope and external inputs are listed in [source checks](docs/validation/source-checks.md). The current engine package is a large, development-only payload. [`runtime/manifest.json`](runtime/manifest.json) points to `../work/proofs/packaging/artifacts/headroom-engine/runtime`, which is excluded from source import. The local H006 proof measured that payload at 523,394,231 bytes. The payload is not included in this repository, and no bootstrap or release-download flow is provided here.

Historical protocol fixture helpers still import runner modules from ignored `/work/` paths, and the Windows runtime smoke requires the absent engine payload. They are outside the frontend gate and are not fresh-clone checks. No helper or payload was copied into this repository as part of H046.

Consequently, a fresh clone is not engine-ready. The current runtime proof is limited to the recorded Windows 10 host; it makes no macOS or Linux support claim. See [the local runtime proof](docs/proofs/desktop-runtime.md) and [the H018 transaction proof](docs/proofs/config-transaction.md) for scope and limits.

## Windows development installer

With the pinned development toolchain and engine payload already prepared locally:

```powershell
pnpm.cmd build:windows
```

This verifies and stages the engine, then builds an unsigned x64, current-user NSIS preview under `work/caches/cargo-target/release/bundle/nsis/`. The installed app carries its Python engine and frontend; users do not install Python, Node or Rust to run it. WebView2 is required; the embedded bootstrapper needs Internet access if the runtime is missing. The installer checks the destination length before copying resources. This payload accepts an installation root of up to 100 UTF-16 units; a longer destination is rejected with an explicit error.

The release manifest points to bundled `runtime/payload`, while mutable application state is stored separately under LocalAppData. The H037 payload inventory is 22,325 files / 533,082,296 bytes, including the later tokenizer cache; it is recorded in [`runtime/windows-payload.json`](runtime/windows-payload.json). Vendor licenses and the development preview limits are described in [NOTICE](NOTICE.md). The source clone still does not download or bootstrap this payload. Local installer results, clean Windows 10/11 and missing-WebView2 gaps are tracked separately in [H037 validation](docs/validation/windows-install.md). H037 remains open; this is not a production release.

## GitHub and evidence

The project is maintained in the [yukselbuilds/headroom-desktop](https://github.com/yukselbuilds/headroom-desktop) repository. The first import was published through GitHub Desktop; [H046 evidence](docs/proofs/repository-import.md) records the scope. H047-a observed the GitHub Free plan and an account usage hard stop. The manual source-only [first CI run](https://github.com/yukselbuilds/headroom-desktop/actions/runs/39759872053) passed 49 app tests and the frontend build on a standard Ubuntu runner. [H047 evidence](docs/proofs/free-ci.md) records the fresh-source check, account hard stop and observed $0 billable usage; native platform support remains separate. The free-only CI decision and gates are recorded in [decision 006](docs/decisions/006-free-ci.md), and the workflow's scope and dispatch requirements are documented in [source checks](docs/validation/source-checks.md).

Some proof documents link to files beneath `work/`. Those paths refer to local development evidence and are intentionally ignored; the evidence files themselves are not part of the initial source import. Generated builds, downloaded engine payloads, temporary runs, and visual outputs are excluded from Git.

## License

The original desktop application source is available under [Apache License 2.0](LICENSE). Bundled components carry their respective licenses and attribution in [NOTICE.md](NOTICE.md).
