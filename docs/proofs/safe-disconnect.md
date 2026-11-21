<a name="h029--güvenli-bağlantı-kaldırma-kabulü"></a>

# H029 — Safe disconnect acceptance

November 6, 2026. Local Windows 10 x64 acceptance. [Plan 017](../decisions/017-safe-disconnect.md), [review](../reviews/H029.md)

<a name="gerçek-davranış"></a>

## Native behavior

After confirmation, Remove reverts the endpoint changes that Headroom owns for the selected client and scope. It uses the existing `undo_connection` IPC command, `PlanManager`, and the `FileTransaction` and ownership path. It preserves the model, authentication, reasoning, and user additions. It leaves a later endpoint change in place. This slice uses the existing undo protocol and config writer. The `integrations/disconnect.rs` module supplies Windows tests only.

The engine keeps running after Remove. Reopen the client to load the updated setting if necessary. Routing for an already open chat remains unverified. The separate engine Stop action closes the route to new requests at stable ingress and leaves the active connection in its own generation. It waits for cleanup of the owned engine and relay.

<a name="kanıt-matrisi"></a>

## Evidence matrix

| Scenario | Observed result | Evidence and scope |
| --- | --- | --- |
| Later user edits, manager reopen, and endpoint conflict for all three adapters | Undo reverted the owned field and kept user edits and other files. After a user endpoint change, it left the config and active ownership bytes unchanged. | Two new `disconnect_all_adapters_*` tests. The final suite of 351 Rust tests used an isolated filesystem. |
| Remove during gated SSE with the real engine | After the first delta reached the client, the test reverted the Codex leaf field. The active generation and endpoint stayed the same. The other two client files matched. One POST completed with a terminal frame. | `live-17.json`. The actual pinned 0.41.0 engine, controller, helper, and manager used a loopback provider fixture. |
| Active SSE, Stop, then open the gate | The new stable ingress request closed before it received response bytes. The existing SSE connection stayed open until the gate opened and delivered its terminal frame. Graceful stop and owned port cleanup passed. | Same opt-in run. The test opened the normal gate within 2 s. The 2 s interval applies to the gate only. The private engine port has separate closure timing. |
| Active SSE and Stop with the gate closed | The request did not reach a successful terminal event. The Stop result reported incomplete shutdown with `cleanup_failed`. The test did not resend the POST. It recorded 0 active TCP connections and a closed owned backend port. The separate listener stayed up and the manager shut down cleanly. | Same run. Uvicorn kept its 10 s request drain budget. The process, controller, and actor budgets stayed at 20 s, 30 s, and 90 s, respectively. The Stop test kept its 60 s limit. |
| Native removal | The test used native preview and apply in a separate Codex profile without authentication. The new confirmation UI exercised Escape, Cancel, and confirmed Undo. Ownership and revision showed released status. The engine generation stayed the same. An explicit Stop separately received acknowledgement. | Native run `54557cfb-0979-4bac-bb62-44e0b6a33f34`. Global user config and app preferences stayed the same. The test used CAS to restore only its own metadata bytes. |
| Pending and failed UI states, and accessibility | The UI blocked a second Undo and closing while Undo was pending. It waited for acknowledgement before showing success. It read ownership again after conflict or commit_unknown. The test checked focus return and Tab wrapping. It covered Turkish and English, light and dark, widths 819–683, and actual WebView scaling of 125, 150, and 100. | The isolated component fixture in the same native run used a bundle guard that blocks real native IPC. Provider and account behavior remain outside this fixture. |
| Tokenizer startup | The test verified six public vocabularies by SHA256. It loaded seven tiktoken encodings with socket connections disabled. | [Preparation](../development/tokenizers.md) and `work/proofs/tokenizers/prepare-95e27441-9279-49bc-bb90-c3d5e8c58008.json` |

The native window exited with code 0 and its debug listener closed. The artifact and protected user and app settings matched throughout the run. The lead agent reviewed the native confirmation and the isolated light and dark renders.

The last native confirmation capture showed a dark or blank render. Visual acceptance used the earlier native capture from `9c8497df-f736-4a42-97ce-95b379aecf01` and the final run's isolated renders. Native acknowledgement and focus checks passed on the final artifact.

<a name="bulunan-iki-motor-sınırı"></a>

## Two observed engine limits

A cold token count could wait twice for 10 s if vocabulary downloads were missing. The isolated diagnosis measured 20.080 s. With the cache ready, the same transport check measured 0.381 s for the first request. The next three requests took 0.032–0.046 s. These timings apply to individual diagnostic runs. Benchmarks and latency guarantees require separate evidence.

The fix kept the health timeout. Before every engine start, the helper verifies cache data and sets `TIKTOKEN_CACHE_DIR` only for the engine process. Git excludes the data and binaries. H038 and H041 track separate installer verification.

The pinned Uvicorn version can cancel a remaining response after 10 s and exit with code 0. The adapter recognizes the fixed marker `timeout graceful shutdown exceeded` as a content-free flag. It reports incomplete shutdown in this case. The Stop DTO and schema stayed the same. Three new Python shutdown and cache tests and the existing controller tests passed in `units-2.json`.

The test fixed canonical Windows path and HEAD prewarm assumptions from early fixture runs. An accepted TCP socket without an explicit blocking mode could cause an early close, a 502, or a startup wait. The final fixture sets blocking mode and handles partial or empty prewarm input without looping.

The first suspicion was concurrent control load. A 502 also occurred with a single process, so the cause remained unproven. The final run 17 uses the corrected fixture. The product candidate removed temporary actor and HTTP diagnostics.

<a name="son-kalite-ve-yeniden-kullanım"></a>

## Final quality gate and reuse

`build-2.json` records the TypeScript and Vite checks and the embedded native build. `final-1.json` records 138 frontend tests. `native-final-2.json` records formatting and 351 Rust tests with 7 opt-in tests ignored. `live-17.json` records the separate actual-engine SSE opt-in pass. The files are in `work/proofs/disconnect/`.

Only the Windows test fixture in `cfg(all(test, windows))` changed at final review. The native suite builds and runs `integrations/disconnect.rs`. The Node, build, and Python results used the same relevant product, TypeScript, runtime, and script inputs and the same tools and environment.

A programmatic check identified one difference in the maps of 156 inputs. `acceptance.json` binds the artifact, assets, and documentation to the evidence. Reuse passing engine and UI checks while their inputs match.

<a name="kabul-sınırı"></a>

## Acceptance limits

The test made no live provider or model call and started no new account sign-in. SSE connected the actual engine to an isolated local provider. Live Codex, Claude, and OpenCode accounts, full model coverage, and subagent support have separate acceptance. Windows 11, macOS, Linux, clean installation, sustained load, stability, and the H041 API-key check remain open. Development token savings remain unmeasured. Work stopped after H029 and before H030.
