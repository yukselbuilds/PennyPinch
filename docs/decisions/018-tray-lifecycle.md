<a name="h030--pencere-tepsi-ve-açık-çıkış"></a>

# H030 — Window, tray, and explicit quit

On 7 November 2026, H030 was complete. Automated native acceptance, the user's real tray-menu check, and the lead agent's final native state/fingerprint observation are linked. The total was 29/51. H031 did not start. The lead agent owned process/settings ownership and shutdown acceptance. The single Luna xhigh worker completed the frontend slice with a fixed contract. Root took over review, fixes, and native acceptance. Evidence is in [tray-lifecycle](../proofs/tray-lifecycle.md).

<a name="amaç-ve-kapsam"></a>

## Purpose and scope

When the native tray registration is ready, X hides only the main window. The engine, stable ingress, and collector keep running. Open or a tray left-click shows the window again. If tray registration fails, the window is not hidden and X requests explicit quit confirmation. Tray status is derived from the engine acknowledgement. Unknown is not ready. The menu's basic actions are Open, Start/Pause/Resume, and Quit. No Tray Stop is added. The engine Stop with a warning in the UI remains separate. Windows startup/preferences, automatic bypass/repair, the installer, and H031+ work are not added to this task.

Full quit always explains to the user that the engine/tray will close, saved routes will not be undone, connected clients may receive errors, and in-flight responses may not complete. Cancel receives initial focus. Cancel makes no engine/config changes. If an owner/revision-protected config operation is in progress at confirmation, the request returns busy and quit does not start. After confirmation succeeds, PlanManager is terminally closed to new operations. Then the existing ExitGate and verifier → collector → RuntimeManager shutdown chain runs once. If the result is not clean, the process exits with exit1. It is not presented as graceful. H029 drain/no-replay/owned-cleanup contracts and budgets do not change.

Window close, pause, disconnect, and process exit are different actions. Force-kill/OS shutdown/crash does not guarantee an uninterrupted connection or automatic return to a direct upstream. Foreign listeners and processes are not closed. First-release tray behavior is fixed. The user is not asked to authorize autostart/account/global settings, and no such setting is written.

<a name="ipc-ve-ui-sözleşmesi"></a>

## IPC and UI contract

`get_desktop_state(requestId)`, `request_desktop_exit(requestId)`, `cancel_desktop_exit(requestId, exitToken)`, `confirm_desktop_exit(requestId, exitToken)` use the same Tauri camelCase argument convention. UUIDs are canonical lowercase. The nonce exists only in process memory.

The snapshot contains exactly `schema_version:1`, `request_id`, `status:"snapshot"`, `tray_available:boolean`, `exit_state:"idle"|"confirmation_required"|"shutting_down"`, and `exit_token: UUID|null`. The token is present only for confirmation_required. The error contains exactly `schema_version:1`, `request_id`, `status:"error"`, and `error:{code,retryable,message_key,diagnostic_id:null}`. Codes are `validation_failed`(false), `state_conflict`(true), and `busy`(true). The message_key is `desktop.<code>`. A malformed correlation uses the zero UUID used by the other protocols.

`headroom:desktop-change` is a content-free change notification. The frontend subscribes first, then reads the snapshot. Reopen or a missed event is recovered from the snapshot. Cancel/confirm requires an exact confirmation-token match. Stale/repeated requests do not close the process. There is no optimistic success on pending/failed IPC. While the top-level quit modal is open, an underlying dialog does not interfere with its focus trap. TR/EN and the existing DESIGN.md apply.

<a name="kabul-ve-doğrulama"></a>

## Acceptance and verification

1. Real native X/hide → tray Open/left-click → same process, engine generation, endpoint, and open gated SSE. The terminal frame is complete and there is one POST. Tray state/primary acknowledgement and the liveness of a second fixture listener outside Headroom are verified. Closing the window does not start runtime shutdown.
2. Quit request/cancel/stale nonce/config busy preserves settings and processes. Confirmation starts only one shutdown. A normally completed flow exits with exit0. A stuck flow records incomplete completion/exit1/no replay/owned cleanup. An engine error is not shown as tray-ready, and the window can be reopened. If tray setup fails, the window is not hidden.
3. Verify TR/EN, keyboard/ESC/focus, narrow view/WebView125/150, and observe the real tray menu and window flow. Run focused frontend/Rust tests first. After the lead agent's connected-flow review, run the final candidate's local WORKFLOW.md gate once. Record before/after source/tool/artifact fingerprints in the same step. Evidence is in `work/proofs/tray/`, the report is `docs/proofs/tray-lifecycle.md`, and the review is `docs/reviews/H030.md`.

The local environment is Windows 10 x64 build19045. Preserve the previously accepted free-work exception from H047/[decision006](006-free-ci.md). Paid Windows 11 validation is skipped/not_tested and does not count as success. Read H030's general two-Windows row with this exception. Other OS, clean-machine, and release acceptance remain separate. No paid resource is added and no VM is installed on the user's PC. Do not report a development token-savings percentage unless it was measured. When the main task is complete, report it and wait for the user before H031.
