<a name="h025--canlı-metadata-ve-genel-bakış-kabulü"></a>

# H025 — Live metadata and Overview acceptance

5 November 2026, Windows 10 22H2 x64/build19045. [Plan 013](../decisions/013-live-overview.md), [lead review](../reviews/H025.md). The lead agent owned runtime/generation, measurement, persistent storage, connection trust, and acceptance. The same single GPT-6 Luna xhigh worker wrote only three presentation files with fixed props and separate frontend contract tests. The completed/stopped state was checked after merged fixes. The lead agent inspected the real connected path and completed the remaining implementation. Model/effort did not change. No development token savings or duration was invented.

<a name="sonuç"></a>

## Result

- While the app is open, it collects metadata every 5 seconds only from its own ready engine. It checks the exact peer PID, held lease, and generation. It keeps the fixed loopback route, 2 MiB body limit, and 1.5-second HTTP budget. Prompt/auth content does not enter storage or the renderer.
- H023 → H024 uses upsert. Repeated polling does not increase totals. The app can read history while the engine is stopped. A monitoring session means this app opening. It does not mean a client chat/account.
- The periods are the monitoring session, today, and 7 local days. Token values and percentages use the same totals. The graph shows only comparable points. Empty and unknown values stay distinct from zero. A missing/mixed basis or counter overflow does not produce an aggregate percentage. A negative result is shown as an increase. Source/time/retention/partial coverage are visible.
- One next action and a real connection record. The app does not select an old/mixed target automatically. A saved connection does not prove an account, open chat, or model traffic. Technical details are in disclosures. Settings 100/125/150 refer only to window scaling.

<a name="doğrulama"></a>

## Verification

| Scenario | Observed / scope |
| --- | --- |
| Owned HTTP → SQLite | Fixed GET, correct peer/lease/generation, large but bounded metadata, malformed and non-owner rejection. The same HTTP source twice → one event. Body/auth canaries stay out. This local fixture is not a real provider. |
| Replay/generation/source failure | The collector continues. A new generation has a separate identity. A source failure does not delete history and shows an interruption. A stopped source does not start the engine. |
| Period/race | A late reply does not appear under a new period label. An error in the same period keeps the prior view stale. Graph/totals come from one local query. Hour/day buckets and mixed-basis checks PASS. |
| Real pinned engine | The real app Start/native manager path reached ready. Collection recorded. A new capture occurred outside Overview. UI Stop kept history and capture time. Restart used a different engine generation and the same monitoring ID. Model requests: 0. Events: 0. Percentage: unknown. |
| Numeric UI | Isolated fixture 120,000→75,000 = 45,000/37.5%. Period change: 14%. Increase: 25%. Real 0, B=0, unknown/mixed, stale/error, and TR/EN passed. These are not real user savings. |
| Advanced/concurrency | Rescan passed with real read-only Advanced open. A separate module mock in a fixture excluded native calls while verify was pending. It preserved collapse and navigation locks until completion. No native verification/provider test ran. |
| Connection choice | Unit tests passed for temporary merging of 2–4 equivalent candidates in the native plan and rejection of different/stale/unsupported plans. The old saved choice was unavailable when the app found the three installed CLI and Desktop candidates. The app did not fall back to another target. Config/ownership did not change. |
| Screen/keyboard/scale | 1280×800/1024×720 and narrow CSS viewports, period keyboard focus, and zero page errors. Real Tauri WebView zoom at 125/150 with devicePixelRatio 1.25/1.5 and return to 100 had no horizontal overflow. OS DPI did not change. This is not an OS DPI test. |
| Exit | The final UI/live/zoom windows received WM_CLOSE and exited 0. Owned Jobs and debug ports closed. Normal Codex config, preferences, onboarding, and ownership fingerprints stayed the same. |

The final candidate is `work/proofs/overview/final2.json`: **120 frontend PASS, 327 Rust PASS, 5 existing opt-in ignored**, plus TypeScript/Vite build, fmt, and offline embedded build PASS. Focused records are `generation-red3.json`, `generation-green2.json`, `generation-transport-red.json`, `generation-transport-green.json`, and `metrics-green3.json`.

Initial tests had an invalid dashed-ID assumption. Live acceptance showed this, and a real 32-hex regression test was added. The first final-candidate gate did not count as proof after live acceptance changed source. No raw error/HTTP body was stored. Records contain only fixed metadata, results, and fingerprints.

Current native evidence is `ui-latest.json`, `live-latest.json`, and `zoom-latest.json`. Combined acceptance is `work/proofs/overview/acceptance.json`. These are project development artifacts. They do not put real app state or a browser profile in Git. Rust1.92.0, Node25.2.1, pnpm11.25.0, and Python3.14.0 were used. No new browser, Vite server, or Codex installation was started. CodeGraph was synced after source changes through the project launcher. A graph result is not test evidence.

<a name="ortam-olayı-ve-sınırlar"></a>

## Environment event and limits

The Windows reserved port range 51717–51816 blocked the old development endpoint at 10013. The OS reservation did not change. No active owner existed, and the known configs did not use the old port. Only the development app-state endpoint was changed to 52175 with backup, atomic write, and fingerprint checks. Three released ownership records were preserved.

The helper's initial revision increment was corrected to the fixed 1 contract. The native validator was not weakened. Evidence: `work/proofs/overview/endpoint-refresh/0ae1a8d8-3ff5-4680-8bb9-d3753d720d3a/`.

The pinned 0.41.0 counter basis remains unknown. The app stores those events as observations. The main card leaves savings unknown for those observations. Model/agent/platform distribution and account/chat lists are outside this task's acceptance. Windows 11/macOS/Linux, installer, real OS DPI, power loss, long run/load, and live API-key were not tested in this run. H026 is the next task. H041 live API-key acceptance stays open. Work stops after H025.
