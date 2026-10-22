# H010-a — OpenCode V1 local fixture transport

Status: **local proof PASS** on the attached Windows 10 host, 2026-10-19. This is one tools-disabled OpenCode CLI request through the pinned Headroom proxy to a local synthetic upstream. It is not a full H010 compatibility result.

## Observed behavior

The pinned OpenCode V1 CLI 1.19.3 sent one streamed `POST /v1/chat/completions` through Headroom 0.41.0 to the loopback fixture. The fixture saw model `headroom-opencode-fixture`, `stream=true`, the per-process fake API sentinel, and zero tools. It returned five OpenAI-compatible SSE frames: assistant role, text, finish reason, usage, and `[DONE]`. The CLI emitted `step_start`, a completed `text` part containing the fixture marker, and `step_finish`; the CLI reported 12 input and 4 output tokens and exited 0. The fixture recorded one total request and one chat-completion request.

This does not establish token-by-token latency or that CLI text was visible before the upstream terminal frame. The pinned [V1 `run.ts` output path](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts#L710) publishes a JSON `text` event after the text part has ended, so the test asserts the completed text marker and terminal usage only.

The test used an inline, process-only V1 config: only the `headroom-fixture` provider was enabled; external plugins and MCP were empty; built-in tools were disabled and wildcard permission denied; `--pure` was passed. Its API key was a synthetic sentinel. No real model, provider credential, OAuth session, desktop/TUI session, or user configuration participated. No reasoning variant was selected or claimed. The V1 config and custom provider shape follow the official [configuration](https://opencode.ai/docs/config/), [provider](https://opencode.ai/docs/providers/), and [tool](https://opencode.ai/docs/tools/) documentation; V2 behavior is not covered.

Headroom readiness was `0.41.0`, and its PID matched the loopback listener owner. `/health` reported the fixture as the OpenAI upstream, coding savings profile, cache mode, cache and optimize enabled, and effective `HEADROOM_OUTPUT_SHAPER=0`. `/v1/retrieve/stats` reported nested `store.default_ttl_seconds=7200`. The child engine received `HEADROOM_MODEL_ROUTER_ENABLED=0`, `HEADROOM_TELEMETRY=on`, and `HEADROOM_BEACON=off`; these three values are recorded as process environment settings, not as separately introspected health fields. The upstream readiness probe was disabled for this fixture run so the engine made no external readiness request.

The engine's normal control path could not use Ctrl+Break from this proof runner (`controller-console-missing`). The owned engine was terminated through the H006 process handle; exit code was 1, the owned port closed, and the new temporary run root was removed. This is a force-fallback cleanup result, not a graceful-shutdown pass. The CLI itself completed normally with exit 0.

## Reproduction

Run these focused checks from `D:\Projeler\Headroom` in PowerShell:

```powershell
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -m unittest discover -s tests\fixtures\opencode -p test_opencode_proof_helpers.py -v
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I work\proofs\opencode\run_opencode_proof.py
```

The test command reports four tests `OK`. The proof runner verifies the pinned archive and executable hashes before launch, creates a fresh UUID state/workspace/config tree with spaces in the path, captures bounded metadata only, and removes only that run's resolved child directory after owned processes and ports are closed. No raw CLI or engine failure log is persisted. The H010 run root count after the successful run was zero.

The child processes receive a scrubbed environment. OpenCode uses only the isolated HOME, XDG, config, temp, and workspace directories; its PATH is limited to the pinned CLI directory and Windows `System32`. `OPENAI_API_KEY` contains only the synthetic sentinel. Headroom binds `127.0.0.1` on a dynamic port and targets the local fixture. Local telemetry was explicitly configured on and anonymous beacon shipping off.

## Fingerprints

Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11, a clean machine, and Tauri process wiring were not tested.

Pinned OpenCode asset: baseline ZIP, 62,204,138 bytes, SHA-256 `0d59113484bfe8d6dc146fc981f5c33faa8318303c183711b7a9387060b289e8`; extracted executable reports version `1.19.3`, SHA-256 `cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375`. The runner checks both hashes before execution.

Pinned engine: `headroom-ai[proxy]==0.41.0`, CPython 3.13.13 x64. H006 packaged helper and runtime manifest hashes are checked before launch. The command fingerprint is the runner SHA-256; the inline config schema/model/provider and child environment are defined in that same runner. The selected fixture model ID is a test label, not a claim about an actual provider model.

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

This slice covers the V1 CLI's ordinary tools-off API transport with a synthetic OpenAI-compatible backend. It does not cover real credentials or inference, ChatGPT auth, tool execution, agents/MCP, TUI/desktop, V2 config, first-token streaming, graceful engine shutdown from the GUI, Windows 11, or clean-machine packaging. H010 remains open for those client paths and product integration.
