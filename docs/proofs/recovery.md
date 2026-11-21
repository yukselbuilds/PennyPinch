<a name="h034--hata-ve-toparlama-kabulü"></a>

# H034 — Error and recovery acceptance

November 8, 2026. **Complete. The total is 33/51.** The lead owned backend, integration, ownership, and acceptance. The existing single Luna worker completed the strict client and help UI changes. Work stopped before H035. [Plan 022](../decisions/022-recovery.md), [review](../reviews/H034.md)

Check connection is at the bottom of Overview and Applications. It shows separate statuses for the engine and stable ingress. It also shows the three apps' saved config scopes and local traffic observations from the last seven days.

Each user action starts one read. The UI blocks another read while that request is pending. The check writes no config or authentication data and starts no model or client. It makes no automatic bypass or repair. The next steps use existing lifecycle actions or open the connection preview.

If a recorded error exists, the report describes its source to the extent it can observe. In this version, the error layer remains unknown.

For a client 401, 403, or sign-in error, the app directs the user to sign in within that client. The check reads no authentication secrets. A lack of observed traffic leaves bypass and savings unknown. The lack of an error report leaves request success unverified. A local config match leaves provider, model, main agent, subagent, and chat verification open.

<a name="kanıt"></a>

## Evidence

The records are in `work/proofs/recovery/`. `acceptance.json` records 18 checks and 187 source and config inputs. It links the inputs, final binary, distribution, and UI bundle to the same tool and OS environment. Verification records results and before-and-after SHA256 values in the same step. The accepted inputs matched. The team reused the passing quality gate.

| Scenario | Observed result | Evidence and scope |
| --- | --- | --- |
| Strict IPC, correlation, field, count, date, and state consistency, raw error suppression, and browser unavailability | The 7 new client tests passed. An unavailable browser transport skips invoke and releases the single request guard. | `worker-final-20261108-358a1eb17f71448b869994f8b0c65245.json`. The lead reviewed the actual DTO and connected path. The final full Node gate covers the lead's final candidate. |
| Owned config for three adapters, different endpoints, malformed config, auth canary, exit busy, and a different Claude project scope | Settings and ownership fingerprints stayed the same. The check reports a match only when the scope is clear and there is no conflict. | Windows filesystem fixtures. `focused-4.json` records 7 initial Rust tests. The final gate has 8 new Rust tests, including a scan race test. |
| Installation metadata changes, missing files, rescan, and a new scan during the report | The report discards stale current and compatibility data. If the scan changes, it keeps the initial counts and reports unknown. | `discovery_tests.rs` and `recovery_tests.rs`. The test reads the file without executing it. Metadata equality supplies no proof of hash or protocol equality. |
| Missing runtime, failure, and timeout states | The actual manager report returned the expected category without starting a generation or engine. A foreign listener stayed active. | `manager_tests.rs` injects the public states. Actual engine termination and live provider failure remain outside this check. |
| Final local quality gate | 168 frontend and 379 Rust tests passed. The gate ignored 8 opt-in tests. Formatting, TypeScript, Vite, and custom-protocol builds passed. | `final-1.json` records matching fingerprints, tools, and OS. The team fixed the first fixture compile error and retained the failed result. |
| Turkish and English, dark and light, widths 1280, 1024, 819, 683, and 507, keyboard and focus, error and pending states, dates, and navigation and unmount | 20 combinations and 8 behavior groups passed with no overflow or page errors. | `ui-b73bf4d8-088b-48e1-9489-5a2669607e84/result.json`. The real App used isolated fixture transport. The lead reviewed Turkish dark and English light renders. |
| UI rescan and changes to engine or config context | The old report disappears. The app explains that it is stale and offers a new check. | Final UI run. The first run exposed a missing scan-generation link. The fix follows the actual `Applications → App → ProblemNotice` flow. |
| Actual native diagnostic click | The engine was Stopped and ingress was Ready. During Start, the previous report stayed visible and the button was disabled. Both then showed Ready. | Run `b4cbd558-306c-497e-8387-87c2f22465b6`. See `stopped-check.json`, `engine-changed.json`, and `ready-check.json`. Sky sent actual mouse and keyboard input. The passive observer left the results and native invoke unchanged. |
| Actual native next action | The action opened Applications. A context change invalidated the earlier report. | `applications-context.json`. The final real-App UI test covered repeated rescan. That case did not run in the native window. |
| Native frontend and backend contract | The strict UI decoder accepted three actual committed replies. | `decoder-acceptance.json` from the same run. The check decoded existing data and made no IPC or provider call. |
| Shutdown and ownership | Normal exit code 0. The owned debug and stable ingress ports closed, and the foreign listener stayed active. The test restored the original runtime, metrics, and preferences bytes. | `result.json` and `baseline.json` in the same run. The protected state and actual HKCU Run value matched. The helper verified processes, ports, and exit code. The final quit click lacked a separate observation. |

The native run used an empty isolated metrics store and made **0 provider calls**. Real user settings and authentication stayed unchanged.

Last traffic uses the latest timestamped request in the recorded Headroom layer. In a limited view, it stays unavailable when the report cannot establish absence. The report excludes paths, raw logs and errors, auth, model and provider labels, and owner, generation, and chat identifiers.

The environment was Windows 10 x64 build 19045, Rust and Cargo 1.92.0, Node 25.2.1, pnpm 11.25.0, and Python 3.14.0. Native checks used Turkish. An isolated UI and client check verified English.

Windows 11 remained not_tested under the H047 and Decision 006 free-use exception. Other OS combinations, installer behavior, sustained load, live provider auth failure, and actual engine termination remained untested. H035 accessibility and performance and H041 release gates have separate acceptance. Development token savings remain unmeasured.

H034 closed. Work stopped before H035.
