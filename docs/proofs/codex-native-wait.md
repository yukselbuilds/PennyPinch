<a name="h008--native-bekleme-ve-mcp-kontrolünün-ayrılması"></a>

# H008 — separate native wait from MCP control

4 November 2026, Windows 10 22H2 x64. The lead agent ran the experiment because of authentication/route and acceptance semantics. It used CLI `0.166.0` and pinned Headroom `0.41.0`. The existing main model/effort stayed `gpt-6-astra` / `max`. One separate, experiment-specific subagent used `gpt-6.1-sol` / `medium` and `fork_turns=none`. These selections are not product model requirements.

<a name="doğrulanan-native-akış"></a>

## Verified native flow

The native record for run `bf2d5c8e-ce03-4f42-b0f5-7f34fc4621ba` showed a real spawn, one wait call and two matching tool returns. The wait runtime event was completed, and the return JSON had `timed_out=false`. The single subagent started and completed. The parent's and subagent's final markers matched. Two different chat identities matched separate Responses inputs. Both ends of nine connections passed ownership checks. The CLI exited with code 0, and the normal config hash remained unchanged.

This is evidence of a completed native wait. It is separate from the earlier record of a wait cancelled before dispatch. The earlier partial result remains historical. The legacy stdout reader's partial result was not used in place of native file evidence.

<a name="başarı-sayılmayan-kontrollü-araç-senaryosu"></a>

## Controlled tool scenario not counted as a success

The same run was also intended to call a local metadata MCP gate. The gate request count remained zero. The run was not considered evidence of tool use just because the model returned a marker supplied in advance.

In the second run, `1e5b1874-1dca-4cd8-b111-c7763f33669d`, the expected marker was removed from the prompt. Tool discovery was allowed. The subagent and parent returned `HEADROOM_GATE_UNAVAILABLE`. The native wait still produced a matching completed event and `timed_out=false`. The gate request count was still zero.

Later read-only review matched the `exec` call and return in the same child record by call_id. The MCP call had been blocked because the client required approval under `approval_policy=never`. It was recorded as `environment_blocked` with fixed reason code `mcp_requires_approval_policy_never`. Missing from the tool inventory was not a valid explanation for this run.

Permission policy was not changed, and the rejected call was not retried. This is not evidence of a Headroom route failure.

In the second run, the engine reported 7 local requests, 0 failed requests and 0 failed WebSocket compression frames. Request purposes were not classified individually. The QA relay observed two `ConnectionResetError / 10054` events. Their source direction and timing relative to the terminal response were not recorded.

QA source `relay_duplex.complete` waits for both TCP EOFs and empty buffers. It does not parse the model's terminal event. The current record cannot determine the original reset direction or whether a reset occurred before or after terminal completion. These counters do not support a general transport-health, harmless-errors or zero-errors claim. Completed native response/role evidence remains separate.

<a name="model-çağrısı-yapmayan-doğrudan-kontrol"></a>

## Direct control without a model call

Run `4c322df7-d294-413f-ae6b-1f05c47427fa` used a separate profile and synthetic API key. It used only initialize, ephemeral thread/start, mcpServerStatus/list and mcpServer/tool/call. It did not call `turn/start`.

The real CLI listed and called the test tool. The local gate observed one request and returned the correct marker. The process exited with code 0 when stdin closed.

This control proves that the fixture works with the real client MCP protocol. It does not prove that a model or subagent can use the same tool.

`tests/fixtures/codex/native_wait_gate.py` uses only stdio JSON-RPC and a fixed loopback metadata endpoint. Three focused tests verified a valid call, rejection of foreign arguments and no false success for an error or wrong boolean. The earlier rejected OS/file-read action was not repeated. This fixture is not an OS tool test.

<a name="kanıt-ve-kalan-kapsam"></a>

## Evidence and remaining scope

Local reports are under `work/proofs/codex-current/gated-agent-runs/<yukarıdaki-UUID>/`. The combined re-review is in `work/proofs/codex-current/native-wait-verification.json`. Only the run's parent/child records were found by identity hash. Source/artifact and record fingerprints matched before and after. Re-review did not repeat a provider request. Test processes and checked listeners closed. The engine stopped gracefully with exit code 0. Content, auth and raw error text were not included in the report.

The content-free permission diagnosis is in `work/proofs/codex-current/approval-classification.json`. Only the child record with a verified matching run identity/fingerprint was inspected. No provider or MCP call was repeated. Raw error text, auth and conversation content were not recorded.

<a name="h008-kapsam-kararı-ve-kapanış"></a>

## H008 scope decision and closure

On 4 November, the user chose to close H008 with ChatGPT evidence and leave the live API-key test open in H041 pre-release acceptance. H008 was completed under this explicit scope decision. API-key support was not counted as live verified. No new paid resource use was authorized.

| Acceptance | Evidence and limit |
| --- | --- |
| Real CLI routing with existing login/model/effort | CLI0.166.0, ChatGPT and pinned Headroom. See [current usage](codex-current.md) and [diagnosis](codex-current-diagnostics.md). |
| Main agent and one subagent with same/different model-effort and native wait | Separate chat/ingress identities and completed spawn/wait. See the native record above. These test models are not a product allowlist. |
| Real Desktop with same chat and reopen | Desktop26.1027.6604.0/backend0.166.0-alpha.5.1, ChatGPT, visible completed response and same native chat. See [Desktop evidence](codex-desktop-live.md). Reopen is required after an open process receives a setting change. Desktop subagent scenario is separately not_tested. |
| Error/scope classification | MCP permission block is environment_blocked. Direction/timing of two QA TCP resets is unknown. No general tool/transport health claim. |
| API key and other environments | API-key HTTP/SSE fixture is separate. Live provider test remains open/not_tested in H041. Windows 11/macOS/Linux, all model features and long-duration stability are outside this acceptance. |
| Review and quality | The lead agent reviewed earlier UI→native manager→engine/client evidence and the new catalog/document changes. The 101 product inputs were unchanged. Evidence for 289 Rust/98 frontend tests and build/fmt PASS remains valid. 3 new Python fixture tests also passed. |

The final content-free acceptance/fingerprint record is `work/proofs/codex-current/h008-chatgpt-acceptance.json`. Version/scenario states were added to the [compatibility catalog](../../compatibility/clients.json). Evidence coverage in older records was not expanded.

The catalog is not yet connected to product UI (H036). UI/runtime and global model/agent/hook settings did not change. Two Python test fixtures were added in the previous slice. Savings or a quality guarantee for all models was not measured. Work stopped before H024 started.
