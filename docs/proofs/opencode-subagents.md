<a name="h010-c--native-opencode-alt-ajan-ve-model-değişimi"></a>

# H010-c — Native OpenCode subagents and model changes

14 November 2026. **Local slice accepted. Main H010 and total 42/54 remain open.** Windows 10 22H2 x64/build19045, real OpenCode 1.19.3, Headroom 0.41.0. [Programmatic acceptance](opencode-subagents.json), [plan](../decisions/009-opencode-provider-routes.md#h010-c--yerel-alt-ajan-kabulü), [review](../reviews/H010-c.md).

OpenCode was not accepted as a system-wide install. The hash of the official project-local binary `work/toolchains/opencode/v1.19.3/runtime/opencode.exe` matched the pinned version. The CLI ran the real task tool and real MCP processes. A local synthetic provider supplied the model responses. No real model inference or billing was measured.

<a name="ürün-yolu-ve-sonuç"></a>

## Product path and result

The same release executable/package manifest accepted in H010-b3 was used. Product source was not rebuilt or changed. Native Start → OpenCode Preview → Apply saved the two eligible providers and the product retrieval bridge once. The CLI read the saved user JSONC normally. No inline config or endpoint override was provided. Test subagent/model definitions were in fixture files before Apply. Headroom did not change them.

| Scenario | Parent / child selector | Real identity check |
| --- | --- | --- |
| Same model | `lab/slot` / `lab/slot` | New parent and separate child. The child inherited the parent's model. |
| Open model change in the same chat | `lab/other` / `lab/other` | The previous parent session ID remained. A different child started and inherited the new model. No reconnect occurred. |
| Different provider/protocol | `openai/slot` Responses / `lab/child` Chat | New parent and separate child. The explicit child model/agent remained. Both saved routes were used. |

Three CLI runs executed sequentially. Each started one foreground child. There were two parent identities and three distinct subagent identities. The task's `sessionId`, `parentSessionId` and model metadata matched `info.parentID` and assistant agent/provider/model fields in the real subagent export. A matching model name alone did not prove a subagent. Export stdout was parsed in memory only. No transcript or HTTP body was copied to the report.

Every role completed data → CCR retrieval → witness. Each parent also completed wait for the task result → final response. There were 27 upstream messages. Each of six log blocks was reduced from 14,999 to 103 bytes. The retrieved content matched the original bytes and hash.

Chat made a real product MCP retrieval call. The engine processed retrieval server-side in the Responses parent. Parent/subagent tools completed. Synthetic auth and wire models were correct. CLI exit0/stderr0.

No product rule or model allowlist that changes model/effort was added. Fixture selectors/wire names do not claim real model support. No reasoning selection occurred in this scenario.

Saved config and owner fingerprints stayed unchanged during all CLI runs. The GUI closed through the normal approved exit path. The same release's existing exclusive-gate `--headroom-uninstall keep` entry removed the connection. Exit0, byte-exact original JSONC, released owner, preserved state and closed owned listener were verified. No force-close fallback occurred. The final app setting returned to the fixture's own original bytes.

UI Undo was separately verified in H010-b3. This slice sent no physical input to the foreground app for a removal popup that did not open in the background.

<a name="kanıt-ve-tekrar"></a>

## Evidence and repeat

Final record: `work/proofs/opencode-subagents/runs/5efdb40e1eea/result.json` passed:true. Nine focused tests passed. They covered a wrong parent/model/agent, partial retrieval, incomplete task, mixing the user marker with the tool prompt and Responses user/CCR format differences. `focused-responses.json` and `focused-responses-extra.json` contain matching before/after source/tool fingerprints. Canonical JSON records hashes for native inputs and result references programmatically.

The earlier 332 product inputs and tool versions matched. Evidence for 222 Node/448 Rust (18 ignored)/fmt/frontend/embedded build was reused. The new benchmark/fixture was tested separately. The old product gate does not claim to include files it did not test. Six hashes for pinned OpenCode task/config/agent/run/export sources are in `work/research/h010-subagents/manifest.json`. The engine's real packaged CCR response handler is also included in the native fingerprint.

Three earlier failed attempts remain: `e2d4db02d026` stopped at app-state namespace validation before GUI opened. `55d8ed1099e3` stopped at Responses `input_text` format. `6dcbb93aa03d` stopped at the extra server-side CCR `original_item_count` field. The same-model/model-change steps passed in the latter two runs, but the overall runs were not accepted.

The final fixture validates the real source envelope. Content/hash equality remains mandatory. Native runs closed normally and removed owned connections. The first native run's separate cleanup is in `cleanup.json`.

Prerequisite: the locally accepted QA release/package from H010-b3 and pinned OpenCode/Python must be ready. The same QA state must have no active connection. The opt-in command below opens its own client state/workspace. Native UI requires Start/Preview/Apply. After the `run-all` trigger in the run directory, the three scenarios run. At the end, close the GUI normally. The benchmark cleans up through the same native Undo entry.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_subagent_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/subagents.py --run
```

Do not repeat successful checks unless their fingerprints change. This benchmark does not start automatic CI or a paid call. Permissions are limited to the fixed data MCP, retrieval and two named test child types. Shell/file/network tools, nested/background agents and the auto/yolo permission flag are off.

<a name="tasarruf-ve-açık-sınır"></a>

## Savings and open limits

Byte reduction in this run is not total-session token/billing savings. Retrieval carries the original content again. In H041's separate local `o200k_base`/full canonical request JSON comparison, repeated logs measured %98.15–98.23. Source/numeric/short JSON measured %0. This is not live provider usage/cache or an average of daily use. [H041 measurement basis](../validation/optimization-benefit.md), [local summary](optimization-local-acceptance.json).

The eight RTK-OSS 1.1.0 examples did not pass H043's safety/rollback/minimum-benefit gate. Managed enablement remains pending in H044. RTK was not silently installed into this chat or product traffic. [RTK results](rtk.md). Headroom routing/savings in this development chat were not verified. No percentage is assigned.

Live provider/auth and real model quality/TTFT/billing, TUI/V2/Desktop, clean Windows/Windows 11 and other OS remain open. Coverage is not guaranteed for other user/config layers or all models. A normal-use measurement requires requests that actually pass through a connected session and can be counted on the same basis. Billing and cache require separate evaluation. No Actions/account sign-in/release/paid call occurred. Linux/Cloudflare deferrals remain.
