# Project CodeGraph

25 October 2026. CodeGraph is a development tool for this checkout. PennyPinch's desktop bundle and runtime dependencies were unaffected. The main task order and the 18/51 count were unchanged, and H022 had yet to start.

## Installation and scope

- Official [colbymchenry/codegraph v1.6.2](https://github.com/colbymchenry/codegraph/releases/tag/v1.6.2) Windows x64 package: `codegraph-win32-x64.zip`, 54,260,850 bytes. SHA-256 matched the publisher's `SHA256SUMS`: `7455e817ee8032666867a1badeb799b8a884c5f4fea9b444e088a02069b6deea`.
- The runtime is under `work/toolchains/codegraph/1.6.2/codegraph-win32-x64` and uses the package's bundled Node v24.16.0. System Node, PATH and npm settings were unchanged. Supply the package locally. The launcher requires it to be present.
- `scripts/codegraph.ps1` selects the pinned runtime, checks the path and reparse points, and runs in this repository. It invokes the bundled node.exe and JavaScript entry point directly with the vendor's Node flags. It forwards arguments and exit status and restores the previous directory and four environment variables in a finally block. Ownership is limited to this project's state.
- From its first run, the launcher sets `DO_NOT_TRACK=1`, `CODEGRAPH_TELEMETRY=0`, `CODEGRAPH_NO_UPDATE_CHECK=1` and `CODEGRAPH_DIR=.codegraph` in the launch process. The opt-out settings were checked against pinned upstream source. The package download uses HTTPS. Packet-level network isolation was outside the check.
- `codegraph.json` uses the official exclusion format and indexes source, tests and scripts. It excludes work, caches, toolchains, proofs, app state, docs, .codex, .codegraph, dependencies, dist, target, lockfiles, binaries and .env files. Git ignores `.codegraph/` database and cache files and the machine-specific `.codex/config.toml`. Keep live credentials, client config and other projects outside the index.

## Daily use

The 26 October update excluded `.agents/`. Sync, status and the file list showed 76 source inputs and zero skill files. A real CodeGraph MCP symbol and source query also worked in this chat. The one-file query returned 15,331 characters. Use a focused question to limit response size. See [WORKFLOW.md](WORKFLOW.md) for tool selection and acceptance, and [DEV001](../proofs/dev-tools.md) for evidence.

Run these commands from the repository root in PowerShell.

```powershell
.\scripts\codegraph.ps1 status
.\scripts\codegraph.ps1 query --json --limit 5 undo_owned_endpoint
.\scripts\codegraph.ps1 query --json --limit 5 createConnectionActionClient
.\scripts\codegraph.ps1 query --json --limit 5 apply_connection_plan
```

`status` shows index state and statistics. Use `sync` after source changes, `init` for initial setup and indexing, and `index` for full reindexing. Check the pinned launcher's `--help` and individual command help for supported options. This version has no standalone `build` command. Reuse valid query results while the relevant source is unchanged.

Read known small files or line ranges with `rg` or direct reads. Use narrow graph queries to find callers, dependencies, impact or an unknown symbol. Request only the relevant source and context. Fall back to source and `rg` when the graph is wrong, stale or broken. AGENTS.md describes the same workflow for the lead agent and single worker, independently of the model name.

## Codex MCP

The trusted checkout's local `.codex/config.toml` holds the server registration. It runs `powershell.exe -NoLogo -NoProfile -NonInteractive -File <checkout>/scripts/codegraph.ps1 serve --mcp --path <checkout>` from this repository. The setup is scoped to this checkout. Git ignores the registration because it contains machine-specific paths. Each other clone requires its own setup.

MCP also sets `CODEGRAPH_NO_DAEMON=1` and `CODEGRAPH_MCP_TOOLS=codegraph_explore`. The pinned version's default shared service can stay active for 300 seconds after the client exits. Direct mode was selected and verified to stop the engine when the test client exited. `--path` sets the default project. The optional `projectPath` tool parameter can point elsewhere, so treat workspace authorization separately. Use this repository by default.

The `codegraph_explore` tool is available to new project chats and after the project MCP reloads. Use the CLI when the tool is unavailable in the current chat. The configuration allows only that MCP tool. Keep `maxFiles` at one to three for most queries and name the target. The acceptance run used the following query.

```json
{"name":"codegraph_explore","arguments":{"query":"undo_owned_endpoint callers","maxFiles":2}}
```

That two-file query returned 17,291 characters. A small CLI JSON query can serve simple symbol lookups. Report savings only when measured in a token benchmark.

## Verification

- The publisher checksum and bundled runtime version matched. The first launcher `init` exited 0 after about 10.8 seconds.
- `status` reported 76 indexed files, 3,539 nodes, 11,456 edges and approximately 14.41 MB. The file list excluded work, docs, .codex, .codegraph, lockfiles and binaries.
- The Rust query for `undo_owned_endpoint` found `src-tauri/src/integrations/transaction.rs`, the TypeScript query for `createConnectionActionClient` found `src/lib/connectionActions.ts`, and the native `apply_connection_plan` query found the expected symbol. The checks retained metadata only.
- Real project MCP over stdio passed initialize, tools/list and tools/call with server 1.6.2, protocol 2024-11-05 and one tool, `codegraph_explore`. It found the target symbol. Every stdout line was JSON within the limit, and stderr recorded a size of 172 bytes. It exited 0. Zero Node processes from this installation remained after client exit.
- Windows PowerShell 5.1 restored the four environment variables and the previous working directory. PowerShell 7.6.5 also distinguished an empty value from an undefined variable. The installation evidence records the final launcher fingerprint.
- The installed Codex CLI reported the project server enabled with the correct working directory and launcher through `mcp get codegraph --json`. An app update had removed the earlier CLI path. The check used the current installation and made zero authentication, model or provider calls.
- Verification focused on installation, CLI and MCP. Product sources, dependencies and build settings were unchanged. The lead agent and worker share the launcher and index, independently of their model.

The evidence under `work/proofs/codegraph-setup/` includes `installation-v1.6.2.json`, final launcher review metadata, `project-config.json`, `codex-registration.json`, the direct-mode `direct-mcp-discovery.json` and `direct-mcp-acceptance.json`, and the combined `acceptance.json`. It retains metadata rather than raw errors, HTTP, tool output or source dumps. Cleanup verified ownership of leftover test processes from the shared-mode trials. The final direct-mode run exited normally and installed no persistent watcher or web server.

Use the graph for discovery. Verify dynamic Tauri invoke-to-Rust dispatch, real client and provider traffic, subagent attribution, runtime behavior and test results independently. Upstream savings figures apply to their own measurements. Follow the existing native, protocol and evidence checks for this repository.
