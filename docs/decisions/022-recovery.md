<a name="h034--hata-ve-toparlama-akışları"></a>

# H034 — Error and recovery flows

Completed 8 November 2026. 33/51. The lead agent owned runtime, file/settings ownership, diagnostic semantics, backend/integration, and acceptance. The same single Luna xhigh worker completed the read-only strict client/UI slice. Root took over after one fix batch. Ponytail full, Emil, and DESIGN.md were applied. H035 did not start.

<a name="amaç-ve-sınırlar"></a>

## Purpose and limits

One Check connection action reports the engine, persistent ingress, each app's saved config scope, and the latest recorded traffic signal separately. The result is a local snapshot. It does not mean provider/auth/main-agent/subagent success or that all models work. Diagnostics do not run a client, send a provider request, write config/auth/client preferences, or perform automatic repair/bypass/restart. Start stays on the existing owned lifecycle path. Config repair stays on the existing explicit Connect → preview → apply path.

Allow at most one diagnostic per user action. Pending/native single-flight rejects a new request. Do not blindly retry failures. Config/auth errors do not close the engine or other processes. Show a safe next step for missing runtime or lost ownership. An unobservable error source remains unknown. For 401/403 or login errors, direct the user to verify/sign in in the client. Do not read auth files.

Reobserve discovered file identity/size/last-write time. A changed or missing file is not current under older version evidence. Recommend a rescan and new config/protocol verification. Current means only that metadata matches the file at discovery time. It does not prove provider/feature compatibility. A rescan makes the old visible report stale. Normal flow does not ask the user to choose a binary.

<a name="ipc-sözleşmesi"></a>

## IPC contract

`get_recovery_snapshot({request:{schema_version:1,request_id:UUID}})`. Request uses deny_unknown_fields. The UI sends no file path, error text, token, or client selection.

The success envelope contains exactly `{schema_version:1,request_id,status:"snapshot",observed_at_utc,runtime,traffic,clients}`.

- For runtime, fields are `{engine_status,ingress_status,error_code}`. Engine states are unavailable/stopped/starting/ready/draining/failed/unknown. Ingress states are unavailable/starting/ready/unknown. error_code is a current runtime public-error allowlist value or null.
- For traffic, fields are `{status,period:"seven_days",last_observed_utc,outcome,error_layer:"unknown"}`. Status is observed/none/unavailable. Outcome is unknown/no_error_reported/error_reported. Observed covers only existing local records from the last seven days. If the timestamp is null, status is none/unavailable. This is not route/account/provider verification. No reported error does not mean success. Do not invent or mask raw error/exit code.
- For clients, return exactly three rows in codex/claude_code/opencode order. Each row has `{client_kind,detected_count,installation_status,compatibility,config_state,config_scope,endpoint_binding}`. Count is 0–512. Installation is current/stale/not_found/unknown. Compatibility is config_supported/not_verified and refers only to the current local config adapter, not live traffic. Config is matches_owned_endpoint/conflict/not_owned/unavailable/busy/not_rechecked. Scope is user_config/project_local_config/unknown. Binding is same/different/not_owned/unknown. Do not expose other fields, personal paths, model/provider labels, auth, config content, or owner/generation identity.

The error envelope is `{schema_version:1,request_id,status:"error",error:{code,retryable,message_key,diagnostic_id:null}}`. Codes are invalid_input (false), busy (true), and unavailable (false). The key is `recovery.<code>`. The UI also displays invalid_response/request_failed/desktop_unavailable. Correlation and exact-field validation are enforced.

<a name="kabul"></a>

## Acceptance

1. Normal local flow uses the real backend report and one action to show four distinct evidence areas in two languages/themes and narrow view. Empty traffic is not presented as a leak or 0%. Config match is not labeled live success. Client/auth/ownership bytes match before and after diagnostics.
2. Engine crash/missing runtime, config change/wrong scope, auth or unknown-source client error, busy/malformed response each show the right next step. One attempt per request. No false success or termination of foreign processes. Repair uses the existing lifecycle or preview flow. Raw errors, content, and secrets do not appear in reports.
3. After an update or scan change, the old report is stale. Metadata change invalidates the old current/compatibility acceptance and prompts a rescan and controlled verification. Run the final candidate's local quality gate and perform a real native click. Keep before/after source/test/config/tool/environment fingerprints. Do not exceed current evidence for Windows 11/other OS.

Focused gate includes new recovery Rust tests and strict client tests. A UI fixture is not real-provider evidence. Run the full WORKFLOW.md gate once on the final candidate. Evidence is in `work/proofs/recovery/`, acceptance is `docs/proofs/recovery.md`, and review is `docs/reviews/H034.md`. When H034 closes, notify the user and stop.

<a name="sonuç-ve-kanıt-sınırı"></a>

## Result and evidence limits

The final candidate passed 168 frontend/379 Rust tests, with 8 opt-in ignored. Fmt, TypeScript/Vite, and the embedded native build passed. 20 UI combinations/8 behavior groups, a real native stopped → start → ready check, invalidation of an old report, and strict decoder acceptance of three native responses were completed. 18 batched checks linked 187 current source/config inputs with the final binary/dist and tool/OS fingerprints. No new provider request was made. Normal exit0, owned-port closure, and preservation of original state/Run were recorded. [Acceptance report](../proofs/recovery.md).

No new writer was added for config repair. Bytes produced by pure undo are discarded. The existing record is used only for verification. Matching metadata alone is not binary-integrity/feature-compatibility evidence. Failure acceptance is limited to real manager public-state injection and real Windows file fixtures. No live auth failure or forced real-engine crash was performed. Unknown layers remain unknown. Windows 10 is local evidence. No result is inferred for Windows 11, other OS, installer, or long-duration load. Wait for the user's continuation before H035.
