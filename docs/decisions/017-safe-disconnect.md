<a name="h029--güvenli-bağlantı-kaldırma"></a>

# H029 — Safe connection removal

Local acceptance was complete on 6 November 2026, before H030 began. [Evidence](../proofs/safe-disconnect.md), [review](../reviews/H029.md).

<a name="sınır-ve-sahiplik"></a>

## Scope and ownership

The lead agent owns connection/settings and active-stream acceptance. The single Luna xhigh worker implements only the finalized confirmation UI. We use H022's existing `undo_connection` → `PlanManager` → `FileTransaction`/ownership path. No second config writer or new undo protocol is added. Model/auth/reasoning and other client settings remain unchanged. A new account, paid provider, release, or global setting is outside this task.

Removing a connection undoes only the endpoint field added by Headroom for the selected client/scope. It does not stop the engine. Other clients and already-started requests may continue on the same engine generation. Open client processes may retain the old setting in memory. The client may need to be reopened for new requests to use the changed setting. No automatic switch to a direct connection or adoption of an open chat is promised. Stopping the engine is a separate existing action that affects all requests using that engine.

<a name="korunan-sözleşmeler"></a>

## Preserved contracts

- No write occurs before confirmation. Cancel/ESC restores keyboard focus. While a mutation is pending, no second command, close, or success result is allowed.
- Canonical scope/path and owner/revision/fingerprint comparison, atomic config+ownership commit, and conflict-preserving undo remain unchanged. A later client edit to the endpoint is preserved. On a failed/uncertain result, removal is not retried until the record is reread.
- Ingress first stops accepting new routes. Existing sockets remain bound to their generation. Stop relies on the pinned engine's graceful shutdown and bounded drain. Forced close is not presented as success/graceful. A request is not replayed.

<a name="kabul-senaryoları"></a>

## Acceptance scenarios

1. Confirm undo for an owned connection across three adapters. Verify that undo removes only the owned field and preserves user additions and other client files. On endpoint/ownership conflict, bytes and active record remain unchanged. A reopened manager interprets prior ownership correctly. The native UI covers confirm/cancel/pending/success/error, TR/EN, keyboard, and narrow/scaled layouts.
2. Remove the connection during a gated active SSE request with the real pinned engine and isolated loopback upstream. Verify that the engine generation and other clients are preserved and the same POST completes exactly once. In a separate active SSE test, choose Stop and open the gate within the budget. Verify the full terminal frame, graceful stop and owned cleanup. An isolated engine settings file supplies the custom loopback upstream. User settings and vendor source are not changed.
3. Keep the gate open on the same path → when existing shutdown budgets expire, show incomplete/timeout. There is no terminal success or replay, and owned process/socket cleanup plus independent-listener liveness are verified. The test opens the normal gate within 2s after stop begins. The stuck-gate test waits at most 60s. Existing 20s engine graceful/30s controller/90s actor budgets are not changed to hide acceptance failure. This is not performance or long-duration-load acceptance.

<a name="kaynak-ve-doğrulama"></a>

## Source and verification

During implementation, a missing tiktoken vocab load was found to block the cold first request, and the pinned uvicorn could exit 0 after a canceled stream. Six checksummed local data files/seven offline encoding preparations and the helper's content-free request-drain-timeout flag were added. Timeouts/model/auth/reasoning/pause/CCR policy did not change. See [source and preparation](../development/tokenizers.md). The 2s fixture limit for a normal gate remains. The real stop boundary is closure of new routes at stable ingress. No SLA is added that the private engine listener will close within 2s.

The relevant UI files are `src/features/integrations/ConnectDialog.tsx` and the new `DisconnectDialog.tsx`. The native `main.rs` undo handler uses `integrations/connection_actions.rs` and `transaction.rs`, with corresponding tests. The runtime path covers `runtime/manager.rs`, `ingress_relay.rs`, the pinned `runtime/desktop_{controller,helper}.py` and engine `settings_store.py`/`proxy/server.py`.

First run focused three-adapter ownership tests and the opt-in pinned-engine SSE test, then review the native UI. Record source and environment fingerprints before/after in the same run. Adapt the existing proof helper. Run the five-command local quality gate in WORKFLOW.md once on the final reviewed candidate. Evidence goes in `work/proofs/disconnect/`, the short result in `docs/proofs/safe-disconnect.md`, and the review in `docs/reviews/H029.md`. Do not repeat successful old tests without a concrete input change. Local Windows 10 evidence is not acceptance for another OS, a real provider, or an installer. Do not report a development-savings rate unless measured.
