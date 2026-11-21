# H010-a — OpenCode V1 local fixture transport

15 November 2026. The [closeout record](opencode-acceptance.md) gives the current H010 acceptance and support limits at this checkpoint. The notes for H010-a/b below describe their historical scope.

The attached Windows 10 host passed the **local proof** on 2026-10-19. One OpenCode CLI request with tools off passed through the pinned Headroom proxy to a synthetic loopback provider. H010 compatibility needs separate evidence.

14 November follow-up. Actual CLI tool and CCR tests passed on two protocols in an isolated profile with a local MCP bridge. Automatic retrieval failed in the default endpoint-only profiles. The H010-a record below covers the earlier tools-off test. Later conditional evidence and open product work appear at the end.

## Observed behavior

The pinned OpenCode V1 CLI 1.19.3 sent one streamed `POST /v1/chat/completions` through Headroom 0.41.0 to the loopback fixture. The fixture saw model `headroom-opencode-fixture`, `stream=true`, the process-only synthetic API sentinel, and zero tools. It returned five OpenAI-compatible SSE frames for assistant role, text, finish reason, usage, and `[DONE]`. The CLI emitted `step_start`, a completed `text` part with the fixture marker, and `step_finish`. It reported 12 input and 4 output tokens and exited with code 0. The fixture recorded one total request and one chat completion request.

Token-by-token latency and visible text before the upstream terminal frame remain unmeasured. The pinned [V1 `run.ts` output path](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts#L710) publishes a JSON `text` event after the part ends. The test checks the completed text marker and terminal usage.

The run used inline V1 config for its process only. It enabled only the `headroom-fixture` provider. External plugins and MCP were empty. The run disabled built-in tools and denied wildcard permissions. It passed `--pure` and used a synthetic API sentinel.

The scope excludes real models, provider credentials, OAuth sessions, desktop and TUI sessions, and user config. The test selected no reasoning variant and makes no claim about one. The V1 config and custom provider follow the official [configuration](https://opencode.ai/docs/config/), [provider](https://opencode.ai/docs/providers/), and [tool](https://opencode.ai/docs/tools/) documentation. V2 requires separate evidence.

Headroom reported ready at `0.41.0`. Its PID matched the loopback listener owner. `/health` showed the fixture as the OpenAI upstream, the coding profile, and cache mode. It showed cache and optimization on and effective `HEADROOM_OUTPUT_SHAPER=0`. `/v1/retrieve/stats` reported nested `store.default_ttl_seconds=7200`.

The child engine received `HEADROOM_MODEL_ROUTER_ENABLED=0`, `HEADROOM_TELEMETRY=on`, and `HEADROOM_BEACON=off`. The proof records these three values as process environment settings. It did not inspect them through health fields. The run disabled the upstream readiness probe and made no external readiness request.

The runner could not send Ctrl+Break through the normal control path (`controller-console-missing`). The H006 process handle terminated the owned engine with exit code 1. The owned port closed, and cleanup removed the new temporary run root. This verifies forced cleanup. Graceful engine shutdown needs separate evidence. The CLI completed normally with exit code 0.

## Reproduction

Run these focused checks in PowerShell from `D:\Projeler\Headroom`.

```powershell
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -m unittest discover -s tests\fixtures\opencode -p test_opencode_proof_helpers.py -v
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I work\proofs\opencode\run_opencode_proof.py
```

The test command reports four tests `OK`. Before launch, the proof runner checks the pinned archive and executable hashes. It creates a fresh UUID state, workspace, and config tree with spaces in the path. It captures only bounded metadata. After owned processes and ports close, it removes only the resolved child directory for that run. The record excludes raw CLI or engine failure logs. After the successful run, the H010 run root count was zero.

The child processes receive only the allowed environment values. OpenCode uses isolated HOME, XDG, config, temp, and workspace directories. PATH contains only the pinned CLI directory and Windows `System32`. `OPENAI_API_KEY` contains only the synthetic sentinel. Headroom binds a dynamic port at `127.0.0.1` and targets the local fixture. The run enabled local telemetry and disabled the anonymous beacon.

## Fingerprints

The host ran Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11, clean machines, and Tauri process wiring need separate tests.

The pinned OpenCode baseline ZIP has 62,204,138 bytes and SHA-256 `0d59113484bfe8d6dc146fc981f5c33faa8318303c183711b7a9387060b289e8`. The extracted executable reports version `1.19.3` and SHA-256 `cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375`. The runner checks both hashes before execution.

The engine pin is `headroom-ai[proxy]==0.41.0` with CPython 3.13.13 x64. The runner checks the H006 packaged helper and runtime manifest hashes before launch. Its SHA-256 identifies the command. The runner defines the inline config schema, model, provider, and child environment. The fixture model ID labels a test. It provides no evidence for an actual provider model.

| Input | SHA-256 |
| --- | --- |
| `work/proofs/opencode/run_opencode_proof.py` | `0d6ff45ed4ea359c2ccb98111c7b8759ecad482530c210a6f5169725a2333d27` |
| `tests/fixtures/opencode/openai_fixture.py` | `6057e56ec426abd5b95e51518c346b9e095029eeef2048986ac54e696eb5fcbe` |
| `tests/fixtures/opencode/test_opencode_proof_helpers.py` | `3fb4a9fbbad66b4e699ecb0a5745066f3d76435ba6f704d107c566c69db4d293` |
| packaged `headroom_runtime.py` | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| packaged `runtime-manifest.json` | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| engine lock (`uv.lock`) | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| pinned `proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| pinned `proxy/handlers/openai.py` | `05f624cd0c704e9b8ba4b332284cd7fe7e81708f7e237da123fc216aabc8b4af` |
| pinned `agent_savings.py` | `3d2c7c7035ff0c4765c04b873d6a22a7f75c79627ba0bf76b47718fd10a46502` |
| pinned `cache/compression_store.py` | `1f269d508abe7590f0634654832bd6fd45786dc8f3e569d983966648c87b1a73` |
| pinned `proxy/runtime_env.py` | `38454fa8331c81158fa7c8e34ccadc0ea0499980ab0d37e6eab44d4d98e95392` |

## Limits

This slice covers ordinary V1 CLI transport with tools off and a synthetic OpenAI-compatible backend. Real credentials and inference, ChatGPT auth, tool execution, agents and MCP, TUI and Desktop, and V2 config require separate evidence. First-token streaming, graceful engine shutdown from the GUI, Windows 11, and clean-machine packaging also need evidence. H010 remains open for these client paths and product integration.

<a name="h010-b--araç-ve-ccr-yolları"></a>

## H010-b — Tool and CCR paths

14 November 2026. Windows 10 build 19045 x64 tested both protocols separately with pinned OpenCode 1.19.3 and Headroom 0.41.0. **Acceptance is conditional.** The positive runs added a narrow local retrieval MCP bridge to a separate test profile. At this checkpoint the Headroom Connect action managed only the endpoint. Product bridge setup and default tool and CCR acceptance needed separate evidence. Main H010 remained open at 42/54.

The actual data tool completed in the initial endpoint-only Chat `9519a08f...` and Responses `f46496df...` runs. The engine received all 14,999 original bytes and created a compression marker. Both runs failed because the automatic retrieval tool was absent. In streaming Chat, pinned `proxy/handlers/openai.py::_should_inject_openai_chat_ccr_tool` rejects automatic tool insertion because there is no continuation. Responses uses buffered server-side continuation when the declared headroom_retrieve tool is present. These behavior checks apply to the tested OpenCode paths.

The later test config declares one fixed log and witness tool and a second stdio MCP server. That server takes only a hash and calls the owned engine's loopback `/v1/retrieve` route. It accepts no URL, path, shell command, or auth. The test config uses the [pinned local OpenCode MCP schema](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/core/src/v1/config/mcp.ts) and the [tool naming source](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/mcp/catalog.ts). The file `work/research/h010-mcp/manifest.json` pins the sources and their hashes. The two runs below supply actual behavior evidence.

| Condition | Chat Completions | Responses |
| --- | --- | --- |
| Configured selector and wire model | lab/slot / headroom-opencode-tool-fixture | openai/slot / gpt-4.1 |
| Provider SDK | @ai-sdk/openai-compatible | @ai-sdk/openai |
| Retrieval declaration | Explicit MCP bridge in the test profile | Same explicit test MCP declaration |
| Retrieval action | OpenCode makes an actual MCP tools/call | The engine handles retrieval server-side. No MCP tools/call runs. |
| Native requests and upstream messages | 4 / 4 | 3 / 4 |
| Native tool calls | Data, retrieval, and witness all complete | Data and witness both complete |
| Result | Final reply, CLI exit 0, stderr 0 | Final reply, CLI exit 0, stderr 0 |

The native client requests streaming in both arms. The engine buffers the four Responses upstream messages with `stream:false`. Chat upstream continues to stream. Fragmented tool arguments, call_id, and final text passed through the actual SDK and CLI.

Native ingress kept all 14,999 original bytes. The upstream block had 103 bytes. The CCR JSON hash and original_content matched the ingress wire string and synthetic log byte for byte and by SHA. The test kept model selector and wire model separate.

Reasoning support is outside scope because the test selected no reasoning field. The model names are synthetic provider test inputs. Real inference and any product model allowlist require separate evidence.

The final Responses record is `work/proofs/opencode-tools/runs/5f4cdcee-f73f-4533-9ddf-9ea19f94960a/result.json`. The Chat record is `.../732cb0fe-23a1-4d03-909b-da5e9c69cba3/result.json`. Five focused tests check wrong call_id, marker, hash, content, model, and reasoning, plus Responses format conversion. Three boundary checks reject invalid hashes, tools, and extra URL fields without network calls. They confirm the pinned Chat streaming policy. Sources, CLI, engine, tokenizer, and tool fingerprints match before and after. Both engines exited gracefully with code 0 and closed the owned port without forced cleanup. [Programmatic acceptance](opencode-tools.json) · [Review](../reviews/H010-b.md).

The test reused the pinned H009-b MCP data server and BoundedOutput and the H041 controller and helper with matching inputs. It used HF and Transformers offline, the cache and coding profile, and CCR TTL 7200. Local measurements were on. Beacon, model router, output shaper, memory, and Learn were off.

The synthetic test used an empty protected-tools list. Actual Read source and numeric protection have separate evidence. The record contains proxy and offline settings. The run did not capture packets.

It preserved user auth, config, active chats, model, effort, and agent count. No Actions run, install, release, or paid call took place.

The product source and 325 prior gate inputs and tools matched. The run reused the applicable evidence from 441 Rust tests, 221 Node tests, formatting, and frontend and embedded builds. New checks covered the benchmark and fixture. The byte reduction describes one block. CCR sends the original content again. Total tokens, cache discounts, cost, TTFT, and model quality require their own measurements. One positive run per protocol covers the tested cases. General reliability and all versions, models, or subagents need separate evidence.

<a name="tekrarlama-ve-açık-ürün-işi"></a>

### Reproduction and open product work

Use the pinned payload and cache. Use the project-local 1.19.3 executable. Run the commands from the repository root. Repeat a passing check only after a relevant fingerprint changes.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_tool_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/roundtrip.py --run --surface responses --with-retrieval-bridge
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/roundtrip.py --run --surface chat --with-retrieval-bridge
```

The flag adds the explicit test-only bridge declaration. It installs nothing in user config. The run without a bridge records the expected initial failure. Treat it as failure evidence during later verification.

**H010-b3 local product acceptance passed on Windows 10.** Preview and Apply install the pinned retrieval bridge atomically through the native plan and receipt. The tests also verified conflict-preserving Undo, pause and reopen, and the release uninstall entry. Both protocols passed through the actual native UI → saved user JSONC → isolated OpenCode CLI path. The run used neither the test bridge flag nor an inline endpoint override.

See the [product retrieval evidence](opencode-retrieval.md), [programmatic result](opencode-retrieval.json), and [review](../reviews/H010-b3.md). The conditional b2 records above keep their historical scope. Main H010 remains open for live provider and auth, real subagents, TUI, V2 and Desktop, clean Windows, Windows 11, and other operating systems.

**H010-c later local acceptance passed.** The [native subagent evidence](opencode-subagents.md) verifies a real foreground child and a model change in the same chat. Same-model Chat, an explicit model change, and a Responses parent with a Chat child passed with separate identities. Both roles completed the tool, CCR, and final reply path. This slice covered b3's historical subagent limit. Live provider and auth, TUI, V2 and Desktop, and other systems remain open. [Programmatic result](opencode-subagents.json).

**H010-d full local TUI acceptance passed.** The same three scenarios passed with a real PTY, full interface, terminal input, and normal `/exit`. The user selected the model change explicitly in the TUI's `/models` menu. A CLI model hint overriding an earlier model on resume remains unverified. This version and Windows 10 fixture cover the earlier TUI limit. Live provider and auth, V2 and Desktop, and other systems remain open. [TUI evidence](opencode-tui.md) · [Programmatic result](opencode-tui.json).
