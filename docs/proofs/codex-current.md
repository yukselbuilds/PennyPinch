<a name="güncel-codex-kullanım-düzeltmesi--h008-yerel-dilim"></a>

# Current Codex usage correction — H008 local slice

3 November 2026, Windows 10 Pro 22H2 x64. Main task H008 remains open, with 20/51 items. This record does not accept an installer, a release or all Codex models and agent paths.

<a name="değişen-davranış"></a>

## Changed behavior

CLI 0.166.0 and Desktop backend 0.166.0-alpha.5.1 are detected by the Windows/OpenAI signature and a bounded version command, without adding a new hash. Immutable target and read-only process pinning remain. The standalone local junction and registered Desktop package are found. A Codex.exe launcher without metadata does not block discovery of the real GUI ChatGPT.exe. Desktop 26.1027.3384.0 is scanned before the long PATH search budget. Version and surface rules for other clients do not change.

The CLI and Desktop connection plan preserves model/reasoning/auth fields through the user config. The verifier workspace stays under app-local state. CodeGraph configuration in the checkout does not block verification with a normal account. The ancestor config guard, atomic write, ownership, stale selection/runtime generation and conflict-preserving undo remain. [Decision 011](../decisions/011-current-codex.md).

<a name="doğrulama"></a>

## Verification

- RED: current CLI/Desktop version and surface plan, app-state workspace selection, and registered Desktop losing its search budget among 61 PATH entries. Each failed before its related fix. Earlier successful commands were not repeated without a source, fixture or config change.
- Real installed binary pin plus long PATH for real CLI/Desktop discovery→PlanManager→Apply→Undo: 2 opt-in PASS, 24.45s. The config was a fixture. Model/reasoning/comment fields were preserved, then the original bytes were restored. The test did not write a real user config. `work/proofs/codex-current/installed-long-path-final.json`.
- Final native candidate: 289 PASS/5 ignored, cargo fmt check and offline custom-protocol build PASS. The final gate found and fixed an old debug-root test assumption and mixed path separators in a fixture. Then 2 workspace-focused tests and the full gate passed. `final-native-workspace-fixture-fixed.json` contains before/after source, tool and environment records. The third old filesystem child among the 5 ignored tests is a helper run only by its parent test. It is not normally run alone.
- 98 frontend tests passed at the `final.json` checkpoint. Later changes were limited to native/source-test files. The recorded frontend inputs and Node/pnpm pairing matched, so those tests were not repeated. The first build fingerprint lacked the root index.html, so only the frontend build was refreshed (`frontend-build-complete-inputs.json`). Source before/after matched, and dist and native binary bytes did not change (`frontend-output-equivalence.json`). The combined record is `work/proofs/codex-current/acceptance.json`.
- Real native Graphite: OCU opened Start/Applications/discovery. A foreground guard blocked a selection click, so it was not forced. The PID tree for the same build's loopback debug listener, owned only by its WebView, was verified. The browser connector had no tool to connect to that WebView, so DOM checks used this limited connection.

  Desktop selection showed the shared-config note and disabled CLI verifier. CLI 0.166 selection→Refresh status→Verify in a new process completed a response through UI→native manager→owned runtime→real CLI with a ready, normal ChatGPT account. 2 extra unclassified CLI notices were shown separately. Saved-setting adoption was not marked successful.
- Native result: `native-runs/3a26fac2-f554-4981-96b7-a88bb90a0b85/ui-e2e-accepted.json`. Binary and UI helper before/after matched. `ui-result.png` was reviewed visually. The product's final message verifies only this response's route. General protocol/tool health remains unknown.

  The long QA session ended at its 900s deadline with owned Job cleanup. `ui.json` records exit 1 and unchanged config/preferences. That run did not PASS normal shutdown after a response. Raw model content/auth/HTTP dumps were not written to evidence.
- Separate short, new owned GUI run: Start→real Ready in 13.8s→WM_CLOSE→exit 0 in about 0.82s. No extra model request occurred, and config/preferences matched. `native-runs/02a91875-ac41-46a9-b537-1df59104d791/{lifecycle-ready,close-request,ui}.json`. This shows the close path worked in a new engine run. It does not turn the earlier post-response deadline result into a normal-shutdown PASS. The record also confirms no debug/ingress listener or owned GUI remained.
- A second short run after a real response did not exit within 120s (`34531cd9-f568-42a6-9c52-c2eaebc0d341`). Runtime IPC remained Ready and responsive. Read-only review found no concrete lock defect. Only debug close-stage metadata was added.

  The final new short real run is `a6bbd355-db52-4020-967e-2a05dbae4716`: real CLI response/2 notices→WM_CLOSE→window-close/exit-requested→verifier clean→runtime clean in 879ms→exit 0. Config/prefs and binary/helper before/after matched. See `ui-e2e-accepted.json`, `close-message-diagnosis.json` and `ui.json`. No functional shutdown fix was made. The reason for the earlier wait is not considered resolved. The source's final full gate is `final-native-exit-observation.json`, with 289 PASS/5 ignored/fmt/build PASS.
- The final gate rebuilt the binary and changed its physical exe hash. Live UI evidence belongs to artifact 8b857e..., whose binary hash differs from the current artifact 4f67bd.... Recorded source/config inputs for both builds match.

  The current artifact was also checked in `native-runs/b2e163b3-899f-442d-90c7-f0ba7296adc7` without a model request. Native IPC/runtime snapshot, three installations and early rejection of a nonexistent selection were verified. Normal shutdown exited 0, config/prefs matched, and binary before/after matched.

  Live provider behavior is reused only as unchanged source/command/tool/environment evidence. It is not presented as a new provider request from the final artifact.

<a name="sentetik-backend-kanıtı-ve-sınırı"></a>

## Synthetic backend evidence and limits

One Luna production controller→helper→pinned 0.41 engine run gave CLI and app-server SSE marker/terminal/exit 0. Each separate client time window showed 2 new engine rows and 2 upstream POSTs.

Exact request/thread-ID matching was not established. CLI had one error item. App-server had warning/configWarning. Only a synthetic model/credential was used. Upstream auth forwarding was not proven.

Source/runtime/binary hashes matched. Graceful stop and owned listener closure were verified.

`fixture-research/runs/c46fcc2c-6d4f-4b47-8d86-08d8951f47ad/engine-fixture-summary.json` and `production-controller-observation.json`. The lead agent reviewed the critical controller/settings/cleanup path and programmatic hash matching. The harness file has only the post-run hash. It is not presented as full helper before/after acceptance. Earlier direct packaging start timeouts were harness attempts that bypassed production bootstrap. They do not show that the product engine failed to start.

<a name="açık-kalanlar"></a>

## Remaining work

[The next diagnosis](codex-current-diagnostics.md) classifies the CLI notice and verifies current CLI main/subagent traffic with separate identities. Inheriting saved config in open Desktop chats, different models/effort, API-key/general auth and broad WS remain open in H008. The same backend/endpoint/model does not prove these capabilities. Win11/macOS/Linux and a clean machine/installer were not tested in this slice. No new account/key was used, and no existing chat was restarted. No model allowlist or per-model/per-agent connection was added. Token savings/percentage were not measured for this development work.
