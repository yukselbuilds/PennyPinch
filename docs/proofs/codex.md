# H008 Codex CLI local transport proof

**Status: partial; H008 is not complete.** The installed Codex CLI completed an isolated Responses turn through the pinned local proxy and fixture, but it did not advertise `update_plan` for H008-a2. The separate H008-a3 `get_goal` callback reached a matching client output and completed the fixture turn; its result was an unclassified string rather than a parseable null value, so the expected goal-null state remains unproven. The runner has no shell/filesystem fallback.

The a2 run used Codex CLI `0.163.0-alpha.10.2` with a fresh temporary `CODEX_HOME`, `--ephemeral`, `--ignore-user-config`, `--disable plugins`, read-only sandbox, `gpt-5-codex` fixture label, `reasoning_effort=high`, and synthetic API key only. The one observed request was an HTTP `POST /v1/responses`, `stream=true`, with model, reasoning, and sentinel auth preserved; `headroom_retrieve` was absent and there was no WebSocket upgrade. The fixture withheld `response.completed` until the CLI emitted the assistant message item containing the unique pre-completion marker. The CLI then completed that message-only turn with exit 0. Its advertised tool inventory did not contain `update_plan`, so no native tool call/output, second request, final fixture response marker, or full function-call loop was proved.

The prior H008-a read-only `exec_command` attempt is **blocked**, not passed. Its tool result reported an automatic policy rejection when the CLI tried to start PowerShell for the read-only fixture command. That path will not be retried through another tool. The attempt also exposed that Responses `usage` lacked required `total_tokens`; the fixture now supplies it. A local fixture-only HTTP check verified totals 53 (`42+11`) and 67 (`51+16`), and a focused sanitizer check passed for plain/JSON-escaped absolute paths and the synthetic key. Neither check substitutes for the blocked client tool loop.

## Reproduction and result

From the project root on the verified Windows 10 host:

```powershell
& '.\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe' -I '.\work\proofs\codex\run_codex_proof.py'
```

Result: `PARTIAL`, `update-plan-not-advertised`; exit code 1 is intentional for the incomplete acceptance. In the same run, the proxy request reached the loopback fixture, the engine-owned listener was closed during cleanup, and the isolated run directory was removed. The controller had no console, so engine shutdown used the owner-bound force fallback; this is not a graceful GUI shutdown proof.

## Fingerprints

The a2 result was produced on Windows 10 Pro 22H2 x64 (`10.0.19045`), CPython `3.13.13`, and pinned `headroom-ai==0.41.0`. The model/backend was a local fixture; no provider credential, paid request, user auth, persistent client config, or active user session was used.

| Input | SHA-256 |
| --- | --- |
| H008 runner `work/proofs/codex/run_codex_proof.py` | `aa5fec78e311fb646af3d1a6ee5845bf1c93142fb8de3cea3f77df1757b8a47f` |
| Gateway fixture `tests/fixtures/codex/local_gateway_fixture.py` | `4d7e15f00fb17c07ef46a402fae2c3453376650885156d180727ca007a097597` |
| Packaged H006 runtime helper | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Engine lock `work/proofs/engine/uv.lock` | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| Engine project `work/proofs/engine/pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| CPython runtime executable | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Pinned OpenAI proxy handler | `05f624cd0c704e9b8ba4b332284cd7fe7e81708f7e237da123fc216aabc8b4af` |
| Pinned Headroom server | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| Pinned Headroom CLI | `19713bfbb09f6506ad51035925eaee20342ab6834294535047d90a3b81d57a74` |
| Installed Codex CLI executable | `f6dbf48cb70f62649c645f26c87dc9bf84cc36986202dfad8e12fb5b8ca70312` |

| Run configuration | SHA-256 |
| --- | --- |
| CLI command | `9b7fccb7cec7574ee9c775358ef7c9648ae6f7af9dcb3c17f1d5e2c1de85039f` |
| Per-process model/provider config | `3ea33ba2a08e8e5f68d449c0823d157f4c228cba4d58ef5925377bb5f483d89f` |
| Isolated client environment | `2391728b790b2ce4a56a21b914e02b3efee3cf4716c0961c28ccbc34faf0a050` |
| Engine command | `eed30a8dc180450fd41342f7b31cc4f9a97b17a97229ed9948d6ebc5807de245` |
| Engine environment | `e4e8f3195fcd3fed5d2660a1bb5e163c40cbb7ba3069260d2d93808de270a64c` |

## H008-a3 native `get_goal` query

Run the separate scenario with:

```powershell
& '.\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe' -I '.\work\proofs\codex\run_codex_proof.py' --scenario goal-query
```

The local fixture observed two `POST /v1/responses` requests over HTTP/SSE. The CLI's request advertised a zero-argument `get_goal` function; the CLI returned one `function_call_output` with the same call ID. The gated assistant message item arrived before the fixture released `response.completed`, then the CLI observed the final fixture marker, emitted `turn.completed`, and exited 0. Both requests preserved the `gpt-5-codex` fixture label, `reasoning_effort=high`, and synthetic sentinel auth. No WebSocket or `headroom_retrieve` path was used.

The new isolated profile had no configured MCP servers and disabled plugins. The native `get_goal` output was a string that did not parse as JSON: `output_type=string`, `parsed_json=false`, `goal_field_present=false`, `goal_is_null=false`. Its SHA-256 was `c96c57c6d2306165a2dcd6e9f34950b6dcf072c4fa825f090cbd25a2df6886bb`. The output text was not printed or persisted; its semantic class is **unclassified text**, so this run proves the native callback/transport/finalization path but not a null goal value. The runner returned `FAIL` with `get-goal-result-not-null` to prevent a false PASS. Owner-bound cleanup passed and closed the engine port; the controller had no console, so shutdown used the force fallback (`controller-console-missing`, engine exit code 1).

The focused safe-shape and redaction tests passed (3 tests), including plain and JSON-escaped path/sentinel scrubbing:

```powershell
& '.\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe' -I '.\tests\fixtures\codex\test_get_goal_scenario.py'
```

The final a3 run used Windows 10 Pro 22H2 x64 (`10.0.19045`), CPython `3.13.13`, pinned `headroom-ai==0.41.0`, and Codex CLI `0.163.0-alpha.10.2`. It used a fresh temporary `CODEX_HOME`, synthetic API key, `--ignore-user-config`, `--ephemeral`, `--disable plugins`, read-only sandbox, and isolated workspace. No paid model call, actual provider credential, persistent client setting, or user session was used.

| A3 input | SHA-256 |
| --- | --- |
| H008 runner | `79ab0f8fa1b39c6073af4da8346b870fb37e16996ac6025b84837be2ae5ba084` |
| Gateway fixture | `bb696c883c1ab802909f5a93b08314292d64b00f6818adac142371d723e5c52d` |
| Focused test | `396c9ccb0572667cfc8a4a7cb648400bc537c516470a55b171260d29fbd7cd93` |
| CLI command | `0c1a883273b17dd6e555b263fedfa96e742fdb55324db823222b875cccc253b5` |
| Per-process model/provider config | `9d89c1041fbdc8af23025c4a6ebcd4143cb141eb270131c18ba2fac1f70f9c33` |
| Isolated client environment | `2391728b790b2ce4a56a21b914e02b3efee3cf4716c0961c28ccbc34faf0a050` |
| Engine command | `eed30a8dc180450fd41342f7b31cc4f9a97b17a97229ed9948d6ebc5807de245` |
| Engine environment | `e4e8f3195fcd3fed5d2660a1bb5e163c40cbb7ba3069260d2d93808de270a64c` |

## Limits and next proof

This is Windows 10 local transport evidence only. It does not prove Windows 11 or clean-machine support, real API/model behavior, ChatGPT authentication, Desktop wiring, or main/subagent behavior. H008-a remains blocked on the CLI's policy rejection of OS process launch. H008-a2 remains partial because the installed CLI advertises no `update_plan` function. H008-a3 proves the native `get_goal` roundtrip but leaves its semantic result unclassified. H006/H014 continue to own GUI lifecycle and Tauri wiring evidence.
