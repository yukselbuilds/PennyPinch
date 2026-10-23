# H019-d5 — Native child lifecycle and transport correlation

23 October 2026. Developer QA only; main H019 and product Verify remain open.

## Normal-history probe, v2

Run `01866b94-447a-421f-b764-44c4fc17ffd1` used the existing normal CLI login, a new read-only workspace and a process-only loopback endpoint override. CLI `.11.2` matched 336,134,960 bytes and SHA256 `c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a`. Main model/effort/provider/auth configuration was not edited. One Luna child was requested without an effort override; native metadata observed main `gpt-6.1-sol`, child `gpt-6-luna`, inherited child effort `medium` and `fork_turns=none`. This is one measured combination, not a product model restriction or universal compatibility claim.

Root reviewed the sole worker's reader and runner against the already observed native schema. The final changed QA candidate passed **35 tests** with `C:\Python314\python.exe -X utf8 -m unittest test_agent_check test_native_run_reader`, Python 3.14.0 on the current Windows 10 host. Source SHA256:

| Ignored QA file | SHA256 |
| --- | --- |
| `agent_check.py` | `910fef81546c06dd8f5cecc6436b76c54955e1ea62c14f8ccfadaada65c920b7` |
| `test_agent_check.py` | `5cd7f93db6bd504bf45207521f2a0600217ee7ec11ac4f804dee698ce57327f3` |
| `native_run_reader.py` | `1484cf89909bd2978d1e8d80ac7ab0ad70c12f6a50d30a140a334bcac3110556` |
| `test_native_run_reader.py` | `9c04870ce4cdaf1a958b242220f751b28309f78c0914dff5a531527cc8d07ac1` |

The native app was reopened because the earlier QA process had closed. Its launch helper checked all 65 application/source/artifact fingerprints against the accepted gate; the unchanged application tests were not repeated. GUI PID25108 owned ingress51778; engine PID19936 owned61664 and descended through11184/7728 from the GUI. The UI Start action reached Ready. Preferences SHA remained `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac` at launch.

CLI exited0. The bounded reader opened only the new parent UUID's rollout, checked its session ID and exact workspace, and confirmed one spawn, child started/completed, matching spawn task/model, successful wait, exact typed child-to-parent marker frame and one exact terminal `task_complete`. Those lifecycle checks passed. Normal CLI private history persists in its usual location; the probe exported only bounded metadata/hashes, not conversation content, auth or raw errors. The exec JSONL summary is separately partial because its short exporter omits native details.

The observer verified eight CLI-to-observer and eight observer-to-owned-ingress peers, with zero relay errors and normal cleanup. A separate 55.04-second/1,314-sample TCP watch observed the owned ingress-to-engine leg. However, the parser accepted no conversation UUID and marked two Responses headers unknown. **Native child-to-wire identity remains unproven**; exit0, the model labels and process peers do not close that gap.

Six new local stat rows were observed. Four output-bearing rows measured **0 tokens / 0% saved**. Two zero-output rows showed464/465 tokens and approximately3.35%/3.42%; they are excluded from response savings. Actor/turn attribution and concurrent traffic remain unknown, so no session or per-agent total is calculated.

Safe results remain under ignored `work/proofs/codex-agent-check/runs/01866b94-447a-421f-b764-44c4fc17ffd1/summary.json` and `work/proofs/codex-agent-observation/native-history-v2-route-watch.json`. The next narrow correction records fixed parsing reasons and separate thread/session identity provenance. Parent/turn identifiers must not be used as child conversation identity. No second live request was sent merely to retry the same candidate.

## Field diagnostics, v3

The next candidate accepted exact UUIDs from either `thread_id` or `session_id`, preserving their field names, while excluding parent/turn IDs from child identity. Fixed method/route/failure counters were added without widening supported routes. Its full QA suite passed **41 tests** under the same Python/host; root matched the frozen hashes and reviewed the changes, so the successful suite was not repeated. V2 sources were preserved under ignored `ignored-history/d5-v2/`.

| Changed ignored QA file | SHA256 |
| --- | --- |
| `agent_check.py` | `3095754a57bc68db7353804eebc91a760906f4cb744dcb6850dc6f6585ec6624` |
| `native_run_reader.py` | `92a0ee32ac2938d7911e6162489a9dd3582106b1d5770d40f1b593962637884f` |
| `test_native_run_reader.py` | `563262e82cc5be559944ba440c049fc5a031ccd9b5f60b22d3303be6aa5297ab` |

Run `e0c35447-bad8-4326-914f-c49a26293940` exited0 and again confirmed the complete native lifecycle, exact markers, model/fork preservation and8/8 owned peers. Diagnostics observed eight GET connections: six `/v1/models`, two WebSocket `/v1/responses`. Both Responses requests were rejected **by the identity parser**, with `invalid_identity`; the transparent relay still forwarded them and real responses completed. The offending field/value shape was not retained, so its cause must not be guessed. No conversation UUID matched. One relay error/closure remains unclassified despite normal CLI completion. A separate55.01s/1,315-sample watch confirmed the owned ingress-to-engine leg. Child-to-wire acceptance remains open.

Four output-bearing rows again measured0%; zero-output3.35%/3.42% rows are excluded, with no actor/session total. The safe summary SHA256 is `e3ab67b34f358ed1c16b3b10822d84bfd0411fabd89ce79a5a24574156998d15`; watch SHA256 `83db89fcb4b2a03c8e795eee7ad9d941877029009b1fc08144e7d31818b7db1c`. The probe now explicitly distinguishes `raw_content_persisted_by_probe=false` from `cli_ordinary_history_enabled=true`.

The next diagnostic must reject every Responses request locally before forwarding or upgrading. Only model discovery may reach the owned ingress. It may retain fixed field/source/value-shape categories, with no raw header, UUID or body. This can inspect the malformed identity shape without another real provider model request; another blind live retry would add no reliable evidence.

## Local rejection diagnostic: root cause isolated

Root reviewed the separate header-check mode and required rejecting any Upgrade header, including empty/h2c, on model discovery. The mode forwards only a bodyless, non-pipelined GET models request, handles one bounded Content-Length response and closes; all Responses/upgrade requests are rejected before an upstream socket is opened. Focused tests caught an unhashable bytearray status token in the bounded relay parser, corrected before acceptance. The new final candidate passed **46 tests** with the same full QA command. Source SHA256: `agent_check.py` `be6a4538954473ce143fbac4f38287eaa70a266c8393d174ce51ca62d4723beb`; `test_agent_check.py` `494c1dc6f747b1d86b1d5b66e8241964d525731b3da32d1215a0b1891c96548f`. Native reader/test hashes stayed at v3.

Run `c74d3b27-6f52-4489-8ff4-5c01601b8d81` rejected one Responses WebSocket upgrade locally and forwarded **zero provider Responses**. CLI exited1 without timeout, as expected after local rejection. The only Responses header had UUID-shaped metadata `thread_id` and `session_id`, but a **non-UUID string `turn_id`**. Fifteen model-discovery headers had none of these fields. No values, raw headers/body or auth were exported. This establishes that the earlier parser's mandatory UUID validation of an auxiliary turn field erased valid conversation identities.

The narrow diagnostic model-discovery relay completed0 of15 exchanges and recorded15 failures; the cause remains unclassified. It did not complete provider/model execution and is not general protocol or model-discovery acceptance. The Responses header was still observed from the normal CLI. This result only justifies ignoring an unusable auxiliary turn ID while preserving strict thread/session UUID and conflict checks; it does not justify accepting malformed conversation IDs or attributing an agent by parent/turn ID. The diagnostic observer closed and all its threads stopped.

## Accepted narrow child-route proof, v4

The sole worker added a failing regression for a non-UUID optional turn ID alongside valid thread/session UUIDs, then changed only that optional turn field to reduce to None. A separate regression keeps malformed conversation IDs fail-closed. Root reviewed the change and matched the frozen final fingerprints. Final offline command `C:\Python314\python.exe -X utf8 -m unittest test_agent_check test_native_run_reader test_schema_probe` passed **49 tests**, Python3.14.0/Windows10.0.19045. The successful gate was reused without a duplicate run.

Final hashes: `agent_check.py` `162f2b06e3fddced4541bd32e672a315394647def12b8f947c6e661dafc01b45`; `test_agent_check.py` `7ff7c3ef2834ab453f60f3f5aced71235f973cdd3dfbb8cd1fff0dfe0e20add1`. Native reader/test remain at v3; `test_schema_probe.py` SHA `2ec965d420d071121e10308867a5e17ae6105199ca34c4671fd9e59149f50031`. The preceding header-check sources were preserved byte-identically under ignored `ignored-history/header-check-c74d3b27/`.

After the measured root-cause correction, root ran one live test `29f3c4fe-6cc2-4828-83d3-13806367de82`. It exited0. The new parent's UUID/workspace, one native child spawn/start/complete, matching model/task/fork-none, successful wait, typed exact child return and terminal parent marker all passed. **The actual native child UUID exactly matched a validated `thread_id` in the incoming Responses WebSocket header.** The observer independently verified CLI and owned-ingress peers before forwarding; two Responses headers were valid, unknown0, peer8/8 and relay errors0. `native_run_metadata.status=complete`, `native_child_route_observed=true`, match fields `[thread_id]`. Its safe summary SHA256 is `a12c37e98f5326199a89e2a19b2b86c47411b636d8c224c333eae2a75c3049d4`.

The old `cli_events` section remains partial and its legacy CLI-event child counter stays0 because the short exporter did not expose the child ID. These are deliberately distinct from the successful native-to-header comparison. Two short exporter error items remain unclassified; no general error-free CLI claim follows. The observer's six unsupported-target counts are the separately identified model-list GETs, not failed Responses identities.

The actual combination was main `gpt-6.1-sol`, child `gpt-6-luna` with inherited `medium`; the QA did not override main model/effort/auth/provider or permanent client configuration. The model pair is a measured example only. All four output-bearing local rows measured **0 tokens / 0% saved**. Zero-output3.35%/3.42% rows remain excluded, and request-level engine stats still lack proven actor/turn scope; no per-agent/session saving total is accepted.

**H019-d5 narrow developer proof is accepted.** Product Verify/UI integration, exact-version product discovery, Desktop, broad model/provider/auth coverage and historical unnamed failures remain separate open work. Main H019 and15/49 completed-main count are unchanged. No application source, runtime payload or CI was changed by this QA slice.

The independent v4 TCP watch ran55.03s/1,313 samples and observed the owned ingress-to-pinned-engine connection; SHA256 `0314fe86df3ba2716ae912ce05078c840edf00c08f15205473eb2359e8e9d909`. Header-check safe summary SHA256 is `ca24ac8db382fe6f4c17464ad69b4fbc140de43b9972a83e9f05ff0e05a52622`.

All CLI/observer/watch processes completed. The minimized own QA window did not close through OCU Alt+F4; after checking its exact PID, creation time and executable, root requested normal `CloseMainWindow`, without force-killing. Native app exit0, preferences unchanged, owned GUI/engine descendant PIDs and listeners absent. Preferences SHA remains `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. The native helper's closed report is under `work/proofs/manual-codex-qa/live-8d1ec39d-a9c4-445c-b802-01d9efc58e0f/native-run.json`. No continuous monitoring remains.
