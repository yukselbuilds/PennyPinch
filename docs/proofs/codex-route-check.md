# H019-d3 — Current route eligibility regression

23 October 2026. **Accepted local slice; H019 remains open.** Root reviewed the sequential backend/frontend changes from the same GPT-6 Luna xhigh worker. This is a current-config correction, not live client/provider/agent acceptance.

## Change and review

Previously an active owned URL could still match after provider/profile/alternate-route fields changed beyond the planner's coverage, permitting misleading local-ready guidance. Both planning and the pinned read now share the existing route rules. A parsed unsupported configuration returns fixed `unsupported_route`; the strict decoder and localized caution preserve the review-setting guidance even with a ready engine. No new IPC shape, writer, auth reader, client executor, polling or automatic repair was added.

Root checked rule extraction, blocker ordering/deduplication, safe read/ownership shortcuts, allowed built-in/empty-provider cases and the UI-to-manager path. Missing files remain missing; malformed/unreadable files remain unavailable; unsupported installations and released/absent ownership still avoid config reads. Ordinary model/reasoning changes do not create a route blocker. The initial failing regression passed after the correction. Focused Codex20, planner17 and status16 tests passed; successful unchanged focused checks were not repeated during review.

## Final local candidate

The changed source invalidated the preceding full candidate. After review, one final gate passed: **65 Node tests,185 Rust tests,3 opt-in ignored**, format, TypeScript/frontend build and embedded `custom-protocol` debug build. No failed tests. Embedded build emitted two existing `dead_code` diagnostics (`preview_config`, `config_was_missing`); the wrapper's count3 includes the compiler summary. The prior d2 build recorded the same warning count. No raw failure log was saved.

Toolchain: Node25.2.1, pnpm11.25.0, Cargo/rustc1.92.0, rustfmt1.8.0. Host: Windows10 Pro22H2 x64/build19045; Windows11/macOS/Linux native acceptance remains absent. `final-quality.json` records62 source/config SHA-256 fingerprints, toolchain/OS, dist and executable hashes; `acceptance.json` rechecked them without rerunning tests.

Executable:16,023,552B, SHA-256 **`2d0f64d44cd48943e43b16ce2f5f7a7fe25bd1d3ef877732c59ddbeed768116a`**. Latest Ubuntu source CI [run39922540641](https://github.com/Protocol272/headroom-desktop/actions/runs/39922540641) covers the preceding d2 source, not this candidate. No new CI dispatch or billing change was necessary for this local native regression check.

## Actual native observations

The embedded app used owned synthetic npm installation metadata and a child config root under the project; no Codex binary was executed. Open Computer Use's name lookup initially matched the repository's Chrome window. Root sent no input there, inspected the pinned resolver and selected the verified Headroom PID instead; no guard was disabled. Accessibility Invoke and scrolling operated the real React/Tauri/manager path. No new physical mouse/keyboard claim.

The real owned engine reached Ready. After UI Preview→Apply, ten explicit status observations passed:

| Synthetic current config | Setting / next step |
| --- | --- |
| Owned baseline | Match / local prerequisites ready |
| Custom provider | Unsupported route / review setting |
| Nonempty provider overrides | Unsupported route / review setting |
| Selected profile | Unsupported route / review setting |
| Profile table | Unsupported route / review setting |
| Realtime route override | Unsupported route / review setting |
| Explicit built-in provider | Match / local prerequisites ready |
| Empty provider table | Match / local prerequisites ready |
| Only model/reasoning changed | Match / local prerequisites ready |
| Original owned config restored | Match / local prerequisites ready |

All reads preserved fixture config, ownership and preferences by SHA-256. Client traffic remained **Not verified**; live auth and main/subagent coverage stayed unknown/not tested. The real UI Undo restored the initial file bytes. The verified owned process received `CloseMainWindow` and exited0 without force; owned GUI, known/project runtime processes and stable ingress listeners were all0. Preferences remained unchanged.

Local evidence root: `work/proofs/codex-route-check/` — `frontend-quality.json`, `final-quality.json`, `native-fixture.json`, `native-run.json`, `native-observations.json`, `owned-runtime-processes.json`, `native-close.json`, `native-cleanup.json`, `acceptance.json`. Screenshot `native-route-guard.png` SHA-256 **`79c9854225d280b6a6397a16f865c7daf41c3a38376523b52b91af23605ab6e1`**. Generated files remain ignored.

## Remaining boundary

The user's sign-in deferral is preserved. No login/auth-store read, real provider/model request, credential substitution, existing client config change or active-session/model/reasoning change occurred. This observation cannot prove effective process adoption, HTTP/model completion, Desktop support or agent coverage. Product live verification and real auth/provider acceptance remain open. The historical unnamed Rust failure is still unexplained; this candidate's PASS does not resolve that anomaly. Measured development token savings are unavailable.
