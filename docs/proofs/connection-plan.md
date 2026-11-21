<a name="h017--bağlantı-planı-ve-önizleme"></a>

# H017 — Connection plan and preview

October 21, 2026. **The H017 local preview passed on Windows 10.** Config writes and actual client connections have separate acceptance in H018–H021. The A–C1 sections keep the results from those checkpoints. This document ends with the final candidate and its acceptance. [Design 004](../decisions/004-connection-plan.md), [final review](../reviews/H017.md)

<a name="a--saf-planlayıcı-kabulü"></a>

## A — Planner acceptance

The Codex TOML, Claude JSON, and OpenCode V1 JSONC fixtures propose a change to the endpoint field only. The plans preserve the model, provider, auth, reasoning effort, and agent count. They mark effective runtime settings `unknown`. They require a new process or session with `restart_required=true`. Each plan links to a UUID valid for five minutes, the config SHA-256, and the ID and port of the persistent endpoint from the backend. The planner reads and writes no auth files, config files, or chat databases.

The document limit is 256 KiB. The JSONC parser scanner checks the limit of 64 nested structures and the token limit before it builds the AST. After parsing, the planner checks nodes and duplicate keys. The TOML parser uses its own recursion limit of 80, followed by a tree check. The planner blocks custom gateways, profiles, cloud routes, and multiple or overridden provider routes because support is unknown. The output excludes original secrets, URLs, paths, and hook commands.

**13 focused Rust tests, rustfmt, and check passed.** The test used Rust and Cargo 1.92.0, Windows x64 MSVC, and a project-local cache and target. At this checkpoint, the check reported 41 dead code warnings because the planner had no production command. The final candidate has its own results below.

The lead agent reviewed the tests, parser, and planner. This candidate fixed the depth, route, generation, and restart issues from early review. Reuse the A command result while its sources match. A test, command, source, and config fingerprints (local receipt `work/proofs/connection-plan/planner-a.md`)

<a name="b1--opaque-seçim-ve-ingress-erişimi-kabulü"></a>

## B1 — Installation selection and ingress access

`select_installation(id, expected_scan_generation)` returns only the summary from the discovery manager. It checks the UUID, a positive JavaScript-safe generation, existence, and the current scan. The renderer receives no file path.

`ready_stable_endpoint()` returns a current loopback endpoint with no health errors. The route is absent when the engine is stopped. When it is Ready, the health check must pass and the route and runtime generations must match. This slice left the runtime lifecycle unchanged.

1 discovery test and 4 runtime tests failed before the fix and passed after it. Format and check passed. The 48 temporary dead code warnings remained until a later command connected the code. The lead agent reviewed the accessors and tests. The run made no real client config, engine, or process calls. B1 source hashes are discovery `d361b8aca433327dd7cd3e600e3db659a1436a6e3e98c8fa6c8938a8584ef4dc`, discovery_tests `4d6f54ecc0be9196375477a883095e31192c4acde68e6fa195335e54c00e7ce0`, manager `7a87cdf639510a2e1cab2d3e55038b66e1985d18ac51f82628b1465043297f99`, and manager_tests `7b5bb168099c8d91e991aa4edcc42bff80403150c394faf882b781dae0f73e09`.

<a name="b2--bounded-config-servisi-kabulü"></a>

## B2 — Bounded config reads

The service gets the selected client's user config path only from the backend environment or default roots. An invalid override prevents fallback. JSON and JSONC files in the same slot cause a conflict. The service distinguishes a missing file from an existing file. A plan for a missing file creates no files or directories.

Between reads, the service compares the content, source file slot, volume, file identity, discovery state, and endpoint again. Plans contain no secrets. The service stores them in memory with a UUID and a five-minute lease. This slice has no Apply command.

**15 I/O tests, 16 plan tests, format, and check passed.** The Windows tests confirmed writer-open and parent rename denial. They also rejected a real junction that needed no special privileges and preserved its target.

The earlier READ_ATTRIBUTES-only handle allowed rename. The team checked the Microsoft sharing rules. The final LIST_DIRECTORY|READ_ATTRIBUTES|SYNCHRONIZE guard with no delete sharing passed the regression.

The first symlink test failed because it needed privileges. The test did not count it as a silent skip or PASS. The tests used project fixtures only. [Microsoft CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew), command, source, and toolchain fingerprints (local receipt `work/proofs/connection-plan/planner-b2.md`)

The lead agent reviewed the guard, source and identity binding, shared error response, and cache. C1 strengthens the framing of the content and context hashes for variable input. The B2 evidence applies to its recorded hashes. B2 added presence, version, and source fields to A's source, so the team ran 16 tests again. The final gate remained open at this checkpoint.

<a name="c1--typed-komut-ve-bridge-kabulü"></a>

## C1 — Typed command and bridge

`main.rs` connects the existing RuntimeManager and DiscoveryManager to one PlanManager. The command `preview_connection_plan` runs blocking file reads separately and returns no raw join or OS error.

The strict TypeScript decoder sends only the required arguments. It checks the client, request, ID, generation, and consistency across all fields. Review tests reject a mixed client, outdated expiry, and conflicting missing-config fields.

AtomicBool and RAII allow one preview read at a time. The second call receives a correlated busy response. The guard releases after a conflict.

**6 TypeScript bridge tests and typecheck passed.** The 1 new concurrency regression, 1 real missing-config regression, and Rust format check also passed. The first C1 run passed 1 wire-name, 4 fingerprint, and 2 same-byte file or slot tests. The fingerprint now uses a length-framed content digest. Reuse the format result in the final gate while its Rust sources match. Native command success remained unverified at this checkpoint. C1 (local receipt `work/proofs/connection-plan/typed-bridge-c1.md`), review fixes (local receipt `work/proofs/connection-plan/typed-bridge-c1-review-fixes.md`), format fingerprint (local receipt `work/proofs/connection-plan/format-pass.json`)

<a name="c2--graphite-önizleme-ve-inceleme"></a>

## C2 — Graphite preview and review

In Applications, the user selects an installation and chooses Preview. The strict bridge sends the request to PlanManager. A bounded reader gets the config for the planner, and the UI shows a redacted preview. The user makes an explicit choice when discovery finds multiple installations. Opening the screen or polling the runtime does not read config.

This scope covers only `user_config`. The UI marks other config layers and effective settings in open sessions as unknown. The plan proposes one endpoint field. It preserves the model, effort, auth, provider, and agent count.

The UI shows the presence of hooks with a fixed label. The app neither displays nor runs their contents. Apply stays disabled. The UI explains that the action previews changes only.

After review, the team clarified endpoint field names, improved row readability, removed the task ID from UI text, and suppressed late responses. The built artifact test showed that Tab could move focus to BODY. The fix uses a capture listener to keep focus within visible and enabled controls. Final TSX hash `d3f711b7518de45ebd48736a3b86956ab03ddb394ecfad3a2998381fb3450265`. [Initial UI slice](connection-plan-ui.md), [first fixes](connection-plan-ui-review-fixes.md), [focus fix](connection-plan-ui-focus-fix.md)

<a name="son-yerel-kalite-kapısı"></a>

## Final local quality gate

The gate used Windows 10 Pro 22H2 x64 build 19045, Rust and Cargo 1.92.0, rustfmt 1.8.0, Node 25.2.1, and pnpm 11.25.0. Browser checks used Edge 155.0.4283.62 and Playwright 1.62.1. The project held the Cargo cache and target. Gate script (local receipt `work/proofs/connection-plan/final_gate.py`), 47 final source, config, and artifact fingerprints (local receipt `work/proofs/connection-plan/final-quality.json`)

| Check | Result |
| --- | --- |
| `node --experimental-strip-types --test tests/app/*.test.mjs` (the report lists 5 files) | 38 passed, 0 failed, 0 warnings |
| `pnpm build` (includes typecheck) | Passed in 1.999 s with 0 warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline -- --test-threads=1` | 107 passed, 0 failed, 2 opt-in tests ignored. Duration 32.950 s. 0 warnings. |
| `cargo build --manifest-path src-tauri/Cargo.toml --offline --features custom-protocol` | Passed in 34.803 s with 1 warning for an unused test adapter function |
| Format | The gate reused the result after checking that the Rust, config, and toolchain hashes matched. Earlier PASS (local receipt `work/proofs/connection-plan/format-pass.json`) |

The warning for `preview_config` refers to the pure adapter that the tests use. The production path uses `preview_config_with_presence`. The metadata counts the warning and the Cargo summary as 2 lines. The build produced one distinct warning. It did not block acceptance.

After the first frontend gate, only the TSX focus fix changed. The 38 tests had the same `.ts`, `.mjs`, dependency, and toolchain inputs, so the team reused them and rebuilt the app. The final gate reused frontend and format results after checking source, artifact, and toolchain fingerprints. Credit and context interruptions did not invalidate this evidence. Frontend metadata (local receipt `work/proofs/connection-plan/frontend-quality.json`)

<a name="statik-artifact-ui-kabulü"></a>

## Static UI artifact acceptance

`node work/proofs/connection-plan/validate_ui.cjs` passed **32 base layouts, 8 dialog layouts, and 5 flow groups**. The test observed 0 browser errors and 0 external page requests.

The test covered Turkish and English, dark and light themes, and sizes 1280×800 and 760×620. It checked explicit selection, preview only on request, stable polling, and focus with Tab, ShiftTab, and Escape. Other checks covered Refresh, exact names for all three fields, and responses for blocked, missing, expired, conflicting, and invalid plans. The test checked that a late response after the dialog closed did not override the new client.

It loaded the built distribution with `route.fulfill` and a synthetic memory bridge. It started no Vite or server process. This result covers the static artifact with a fixture. Native IPC has separate evidence below. The lead agent reviewed two screenshots. Metadata (local receipt `work/proofs/connection-plan/ui-review/summary.json`), dark English screenshot (local receipt `work/proofs/connection-plan/ui-review/plan-en-dark.png`), narrow light Turkish screenshot (local receipt `work/proofs/connection-plan/ui-review/plan-tr-light-760.png`)

<a name="gerçek-native-kabulü"></a>

## Native preview acceptance

The embedded EXE used **15,298,560 bytes**. Its hash was `4ee7b53789d8829a66a7fd8e8974d369fd4017e8d754770a8e3436172f818bf1`. The launcher supplied disposable CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_CONFIG_HOME, and PATH values only to the child Headroom process. The test used no real config, auth, or chat database.

The real backend read three files from roots with Unicode characters and spaces. The `.cmd` candidates had 0 bytes. The test ran neither the candidates nor the installed Claude candidate found during discovery. It started no engine and sent no provider requests. Launcher (local receipt `work/proofs/connection-plan/native_fixture.py`)

The operator used indexed mouse input through the standard OCU MCP to select Connect and Preview on all three cards.

The Codex TOML field `openai_base_url` and OpenCode JSONC field `provider.openai.options.baseURL` showed `http://127.0.0.1:51778/v1`. The Claude JSON field `env.ANTHROPIC_BASE_URL` showed the same persistent address without `/v1`. With multiple Claude installations, the dialog selected none initially. The operator selected the PATH candidate and then chose Preview.

The UI showed the config notices and preservation states correctly. The effective runtime setting showed unknown. The unknown-version blocker and disabled Apply button were visible. Codex Refresh updated the expiry. The operator used ScrollIntoView to inspect the lower Claude rows.

The test observed Tab and ShiftTab wrapping between the last and first controls in OpenCode. Escape returned focus to the card. The header Close button also worked.

The OCU overlay guard rejected a physical click on the native dropdown option and sent no click.

The operator used End and Return in the same open, focused combo box to select the option. Generic scroll did not move the content. The operator used the element's ScrollIntoView action.

After Alt+F4, the tool returned `appNotFound`. The launcher independently verified **normal exit 0**. The record includes these automation failures. Observation metadata (local receipt `work/proofs/connection-plan/native-proof.json`), actual hash and exit report (local receipt `work/proofs/connection-plan/native-run.json`)

The three config files and the fixture tree of 6 files kept the same bytes. The test created no additional files. The initial and final preferences hash was `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. After exit, app PID 10152 was absent and port 51778 had 0 listeners. No monitoring or test window remained open.

<a name="kabul-ve-kalan-sınırlar"></a>

## Acceptance and remaining limits

The H017 preview met its acceptance criteria. An unknown version produces a blocked draft. The native test covers configuration preview for the three installed clients. Live auth, models, transport, and subagents remain outside this checkpoint. The compatibility catalog retained its previous status.

At that checkpoint, Windows 11, clean installation, atomic Apply and Undo (H018), and real client traffic (H019–H021) had separate open gates. Pause, CCR, and actual savings measurement also had open gates. The team updated TASKS and the checkpoint. **Work stopped before H018.**

The final validator checks only 45 unique main tasks and 12 completion marks, local links, recorded hashes, and exit reports. The hashes cover source, artifacts, native fixtures, and preferences. The result is in `work/proofs/connection-plan/handoff-quality.json`. The validator runs no test, build, or UI commands. Validator (local receipt `work/proofs/connection-plan/validate_handoff.py`)
