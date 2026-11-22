# Claude Code local transport proof (H009-a)

Current main acceptance as of November 15, 2026: [H009 complete, local Windows 10 technical scope](claude-acceptance.md). Keep the H009-a and H009-b results below under their historical profiles. This does not add live account/provider, subagent or plugin support.

**PASS** for the local API-fixture stream with tools off on the Windows 10 22H2 x64 host (10.0.19045). The test uses synthetic transport. Live Anthropic and model behavior remain outside this result. The main H009 acceptance remained open at this checkpoint.

November 14 update: Acceptance also covered one enabled MCP tool and the CCR continuation loop with the real CLI. The historical tools-disabled scope of H009-a below remains unchanged. The new H009-b scope is at the end of this document. Live Anthropic/OAuth and main H009 remain open.

## What was exercised

The test started Claude Code CLI 2.1.158 with a disposable profile and a synthetic API-key sentinel. Its `ANTHROPIC_BASE_URL` targeted the pinned Headroom 0.41.0 runtime on loopback. The engine forwarded traffic to a loopback-only Anthropic SSE fixture. The packaged H006 runtime helper (`runtime/headroom_runtime.py`) started and owned the engine listener. The test left user credentials, settings, sessions, client configuration, and persistent state untouched and made no copies of them.

The fixture observed exactly one `/v1/messages` request. The request used the selected fixture label `claude-opus-4-6` and `effort=high`. It used streaming, zero tools, the sentinel credential, and an Anthropic version header. The model label identifies the test input. The request included a `thinking` object. The test verified the object's presence. Preservation of its value remained outside scope.

The fixture sent a partial text delta with the marker. It held the terminal SSE events until the CLI emitted that marker in a `stream_event`. The fixture then sent `message_stop`.

The CLI returned success and exited with code 0. It reported terminal usage of 12 input and 4 output tokens.

The usage parser uses the direct `usage` totals when it also receives the duplicate `modelUsage` breakdown. The focused tests cover that case, breakdown-only results, missing usage, and the partial-stream gate.

The engine used cache mode and the coding profile. It enabled cache optimization and used a CCR TTL of 7200 seconds. It disabled the output shaper and model router. It enabled local telemetry and disabled the anonymous beacon.

The engine PID and loopback listener PID both were 10444. Cleanup stopped the CLI and fixture, removed the isolated run directory, and confirmed closure of the owned engine port.

The consoleless controller reported `controller-console-missing`. It used the owned-process force fallback and observed engine exit code 1. Graceful shutdown and the Tauri GUI lifecycle remained unverified here. H014 tracks Tauri GUI lifecycle verification.

## Reproduction

Focused helper tests:

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I -m unittest discover -s 'tests/fixtures/claude' -p 'test_claude_proof_helpers.py' -v
```

One final end-to-end run:

```powershell
$claude = (Get-Command claude -ErrorAction Stop).Source
& 'work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I 'work/proofs/claude/run_claude_proof.py' --claude-exe $claude
```

Both commands passed for the fingerprinted candidate below. No raw CLI or failure logs are persisted.

## Fingerprints

| Item | SHA-256 / value |
|---|---|
| E2E command fingerprint | `8ea21e21cc615a667775c90e1e7e1eeb3d20d28458e07138fb2bfee0a7f8a4a4` |
| Isolated config fingerprint | `450f492af397f3cee74e98cd8b9d5baa54263e53183352807ef39cc9a01d3b52` |
| `work/proofs/claude/run_claude_proof.py` | `6034f8b65d162bd1824358e724f5c5e7c41699605d2ff4a0672902741146a120` |
| `tests/fixtures/claude/anthropic_fixture.py` | `e69c49f33437f679b3f271102a95db01354d9f075a4a7f7bf0572d6ebc2112a5` |
| `tests/fixtures/claude/test_claude_proof_helpers.py` | `64efada3c14ca2403206c3d57c5e890a47280ea8e848f8c5d000936a3b8a9f40` |
| Packaged Headroom helper | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Packaged runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Engine source `pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| Pinned engine lock | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| CPython 3.13.13 x64 executable | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Claude Code CLI 2.1.158 executable | `10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74` |
| Pinned `headroom/proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| Pinned `headroom/proxy/handlers/anthropic.py` | `a9e4ef859e2d7a5990db72117159751fb1b16a072bb047c503ceddf4d5ffd467` |
| Pinned `headroom/agent_savings.py` | `3d2c7c7035ff0c4765c04b873d6a22a7f75c79627ba0bf76b47718fd10a46502` |
| Pinned `headroom/cache/compression_store.py` | `1f269d508abe7590f0634654832bd6fd45786dc8f3e569d983966648c87b1a73` |

The runtime manifest identifies `headroom-ai[proxy]==0.41.0`, CPython 3.13.13 x64, 102 locked packages, and the package and source fingerprints above. The existing runtime tree contains 22,225 files and uses 523,394,231 uncompressed bytes. A separate Claude bundle-size measurement remains outside this result.

## Boundaries

This result applies to the local synthetic Anthropic API-fixture path on the recorded Windows 10 host. Live Anthropic requests, model inference, preservation of the `thinking` value, and user OAuth or authentication compatibility remain outside the scope. Claude Desktop, enabled tools, hooks, plugins, MCP, and subagents remain outside the scope. A clean machine, Windows 11, and the Tauri consoleless lifecycle remain untested.

The run disabled tools and made no paid provider calls. The fixture and temporary state were local. Unrelated engine asset egress remains outside the proof.

Claude's gateway/base URL configuration is described in [Anthropic's LLM gateway documentation](https://docs.anthropic.com/en/docs/claude-code/llm-gateway). Nonessential traffic suppression is described in [Claude Code data usage documentation](https://code.claude.com/docs/en/data-usage).

<a name="h009-b--gerçek-cli-araç-ve-ccr-döngüsü"></a>

## H009-b — Real CLI tool and CCR loop

November 14, 2026. The local slice passed on Windows 10 build 19045 x64 with Claude Code **2.1.158**, the same measured executable hash and pinned Headroom **0.41.0**. It used no real model or account. The upstream was a synthetic loopback fixture that implements the protocol. Model label `claude-opus-4-6` and `high` were test inputs. The product does not switch to another model/effort or tie support to this label.

The test started the real CLI in an isolated profile. It disabled built-in tools and allowed only `mcp__headroom_fixture__inspect_logs`. The stdio MCP server's only tool returns a fixed synthetic log or short witness. The test separately observes the CLI output before it enters the engine and its representation after it reaches upstream. It does not write raw bodies to the report.

| Stage | Expected and observed |
| --- | --- |
| First client tool call | Fragmented SSE tool_use became MCP tools/call in the real CLI with the same ID and arguments. |
| Original result | The original log was byte-identical through the CLI → test ingress → engine path: 120 lines / 14,999 bytes. |
| Compression | Upstream received 103 bytes and a 24-character CCR marker with the same tool_result ID. The retrieval tool definition was present. |
| Retrieval | The engine processed one upstream headroom_retrieve call. Original content in the JSON hash/original_content envelope matched by SHA-256 and bytes. |
| Continuation | After CCR, the second real MCP call/witness and expected final response completed. CCR did not leak as a separate client tool. |
| Shutdown | CLI exit0/stderr0. Engine graceful exit0, owned port closed and no force fallback. |

The run made **3 native Messages requests / 4 upstream Messages messages**. Upstream stream values were `true, false, false, true` in order. The engine used its buffered and server-side continuation path for CCR.

The native client requested streaming in all three requests. The test preserved model/effort/thinking and synthetic auth at every upstream stage. It did not measure first-token latency or real model response quality.

Byte reduction in one block is not a total request/token/billing saving. Retrieval sends the original content again.

**Important client-side limit:** In the first 75KB log runs, Claude CLI moved the result to a tool-results file in its isolated profile. The short reference sent to the provider was not the original log. Keep these runs as failures.

The passing fixture used 120 lines, below the client's file-move limit. The test did not override the CLI cap. Do not attribute Headroom token savings to large output that the client moved to a native file. A later run failed because the fixture assumed the wrong CCR JSON envelope.

The test corrected it to match pinned `ccr/response_handler.py`.

The final native record is `work/proofs/claude-tools/runs/0c5e4fe1-4cad-4d58-b0b5-24d878e9bed6/result.json`. Earlier failures and the pre-review PASS remain separate. Source, CLI, Python, engine `.py` and tokenizer-cache files matched before and after the run.

Four new focused normal/error/boundary checks passed. Production source/test/config and Node/pnpm/Rust tools did not change. The team reused existing 441 Rust, 221 Node, fmt and frontend/embedded build evidence with matching fingerprints.

It did not repeat the same tests without cause. [Programmatic acceptance](claude-tools.json), [review](../reviews/H009-b.md).

The engine used the coding/cache profile, CCR TTL 7200, local measurement on and beacon off. Router/output shaper/memory/learn were off.

The H041 fixture overlay enables HF/transformers offline only for local ML cache use. The protected-tool list was empty in this synthetic test. Built-in Read/Bash/Edit were off. This slice did not reverify real source/numeric Read protection.

The loopback observer is a test tool. It is not real Tauri UI/Rust ingress or live auth acceptance. The test recorded proxy/offline settings. It did not capture packets.

### Tekrarlama

Use the current pinned engine/Python/cache and a CLI with a matching hash. Refresh passing results only when relevant inputs change. The commands create a separate UUID test environment from the repository root without changing the user profile:

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/claude -p test_tool_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/claude/roundtrip.py --run --claude-exe (Join-Path $env:USERPROFILE '.local/bin/claude.exe')
```

The last command does not run a different binary hash. It does not change native CLI caps/settings, user auth or sessions in other projects. Evidence for live Anthropic/OAuth, general tools/hooks/plugins, subagents, Desktop and other operating systems remains open. Main H009 and total 42/54 do not close with this local acceptance. Do not upgrade the old tools-disabled catalog profile as if it accepted this different MCP profile.
