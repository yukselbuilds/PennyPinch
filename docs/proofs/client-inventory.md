<a name="istemci-ön-envanteri"></a>

# Preliminary client inventory

19 October 2026 — Read-only `Get-Command`, PE version metadata, and SHA-256 checks. This document is not H008–H010 live-traffic or support evidence.

| Surface | Local finding | Version / status |
| --- | --- | --- |
| Codex CLI | Found `%LOCALAPPDATA%\OpenAI\Codex\bin\896a0166b774037a\codex.exe`. | `--version`: `codex-cli 0.163.0-alpha.10.2`. Login status reported the existing sign-in as `chatgpt`. Real proxy routing will be checked separately in H008. |
| Claude Code CLI | Found `%USERPROFILE%\.local\bin\claude.exe`. | `--version`: `2.1.158 (Claude Code)`. PE `2.1.158.0`. Auth status: `loggedIn=true`, `authMethod=oauth_token`. Proxy/auth forwarding was not yet verified. |
| OpenCode CLI/TUI | `opencode` was not found on the current PATH. | This does not show total system absence or protocol incompatibility. H010/H016 cover known installation paths. |

Binary SHA-256:

- Codex: `f6dbf48cb70f62649c645f26c87dc9bf84cc36986202dfad8e12fb5b8ca70312`.
- Claude: `10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74`.

No client settings/auth file changed, and their contents were not copied into the report. Finding a client does not prove real proxy traffic, an active auth method, or main/subagent coverage. This inventory can be reused. A binary/config fingerprint change during route verification invalidates old compatibility evidence.

The inventory used `codex login status` and `claude auth status --json` to inspect status. Both commands exited with code 0. The runner parsed the outputs in memory. The report records only the auth type/sign-in status above. No token, API key, email, or organization information appears in the report.

The snapshot reports the auth type at that checkpoint. Proxy inference, account quota, and future sign-in validity need separate evidence. Recheck during the live-route stage if the auth method/environment changes.

<a name="h008-için-güncel-resmî-kaynak-notları"></a>

## Official source notes for H008

On 19 October 2026, the team found and reviewed the relevant sections of these official OpenAI Docs pages.

- Provider/base URL keys in project `.codex/config.toml` cannot serve as the effective routing method. Verify process or supported user-scope behavior separately in the real client. [Configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).
- For a custom provider, `requires_openai_auth = true` is a candidate auth path that can use the existing OpenAI/ChatGPT sign-in. In this case, the configuration does not use `env_key`. This document does not prove local Headroom upstream compatibility. H008 still needs real route/model/reasoning checks. [Authentication — Alternative model providers](https://learn.chatgpt.com/docs/auth#alternative-model-providers).

The team did source research only. It did not apply these settings to a user profile. The test does not require changing the selected model/effort or subagent count.

In the continuation session, the team read the same binary's `--version` and `exec --help` output. This version offers process-scoped `-c` override, `--ephemeral`, `--ignore-user-config`, and `--sandbox read-only` options. The help says that `--ignore-user-config` still uses auth from `CODEX_HOME`. These options are candidates for an isolated experiment. Do not present disabling the whole user's settings as a product integration.

A gateway that carries ChatGPT sign-in is not equivalent to the API-key `/v1` route. The official document describes routing `/models` and `/responses` requests to `https://chatgpt.com/backend-api/codex` and preserving existing auth headers. Headroom's ability to carry this route must be tested separately. [Sign in with ChatGPT through a gateway](https://learn.chatgpt.com/docs/enterprise/sign-in-with-chatgpt-through-a-gateway).

H008 requires more than HTTP 200. It checks incremental SSE, the full tool-call → matching result → final response loop, and a follow-up turn. If the selected transport uses WebSocket/incremental continuation, it must also check the related continuation behavior. [Gateway compatibility requirements](https://learn.chatgpt.com/docs/enterprise/gateway-compatibility).

Source review of pinned Headroom 0.41.0 shows that `providers/codex/runtime.py` recognizes the ChatGPT account header/OAuth claim as a routing hint. It leaves authorization to the upstream. In this branch, `proxy/handlers/openai.py` selects the fixed Codex backend target.

Therefore, setting `OPENAI_TARGET_API_URL` alone does not keep a ChatGPT-auth experiment inside the fixture. H008 must separate local and live steps according to this behavior. Fixture checks must not use real account credentials. This source finding does not prove successful live auth/routing.

The same pinned handler's `_should_buffer_openai_responses_stream_ccr` branch may select buffered JSON → SSE for an API-auth request when Headroom enables the `headroom_retrieve` tool. ChatGPT auth is exempt from this condition. Incremental SSE success in an ordinary tool loop does not prove the same latency behavior for every API request with CCR enabled. Keep the real retrieval loop, transport, and latency scope separate in later client/compatibility testing. Do not hide this difference by disabling core CCR.

<a name="h009-için-yerel-cli-hazırlığı"></a>

## Local CLI preparation for H009

The team read Claude 2.1.158 `--version` and `--help` output only. The following options are available for a local API-fixture experiment: `--bare`, `--print`, `--no-session-persistence`, `--output-format stream-json`, `--include-partial-messages`, `--setting-sources`, `--settings`, `--tools`, and `--strict-mcp-config`. Permission modes are separate. Do not change existing user settings or repeat a denied Codex/PowerShell action through Claude.

Help says `--bare` skips hooks/LSP/plugin sync/auto-memory/keychain and CLAUDE.md discovery. It restricts Anthropic auth to an API key or explicit apiKeyHelper and does not read OAuth/keychain. Therefore, a bare+sentinel test is not evidence of compatibility with the existing OAuth sign-in. Test a live OAuth path separately while preserving the currently selected model/effort. This work collected CLI help metadata only. No model call occurred.

<a name="h010-için-resmî-sürüm-ve-izolasyon-notları"></a>

## Official version and isolation notes for H010

On 19 October 2026, the team reviewed the [official landing page](https://opencode.ai/docs/) and the [V2 landing page](https://opencode.ai/v2/docs/). V1/V2 use the same default command. No automatic global install or major upgrade will occur. Do not assume the V2 provider schema matches V1. The first H010-a candidate is a pinned V1 CLI that can be kept inside the project. V2/Desktop/TUI support remains open for separate review.

GitHub published the [official v1.19.3 release](https://github.com/anomalyco/opencode/releases/tag/v1.19.3) on 16 October 2026 with prerelease=false. The recorded GitHub release metadata lists `62,204,138` bytes and SHA-256 `e4ce9117f55fc068414513789f8c7d1049f855d7518bb4e350b9170a3a20d8c3` for the Windows x64 ZIP. The baseline ZIP has the same size. Its hash is `0d59113484bfe8d6dc146fc981f5c33faa8318303c183711b7a9387060b289e8`. Asset names: `opencode-windows-x64.zip` / `opencode-windows-x64-baseline.zip`.

This metadata is not download or runtime evidence. If downloaded, verify the hash locally and record the real exe version.

The [V1 configuration document](https://opencode.ai/docs/config/) describes JSON/JSONC and merged settings. `OPENCODE_CONFIG` alone does not isolate user/project settings. `OPENCODE_CONFIG_CONTENT` is a separate process-scoped override. Validate scope with a new temporary HOME/XDG/workspace, isolated auth/config, and read-only help/debug output from the real binary. The fixture test also needs to disable the client's own tool/MCP/agent calls and prevent model requests from going beyond loopback. No installed OpenCode or real-traffic PASS exists here.

In the pinned [v1.19.3 run.ts](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts#L710) source, the `--format json` path emits a text event when `part.time.end` appears. Therefore, an H010 fixture that waits for the first token on CLI stdout may block upstream incorrectly. Test the gate between the completed text part and the terminal message separately. Do not report this as first-token-delta latency. No event in CLI JSON output alone does not mean the network lacks incremental SSE.
