# H019-d4 — Manual Codex CLI launcher preparation

23 October2026, Windows10 Pro22H2 x64/build19045. **Preparation accepted; interactive user test and main H019 remain open.** The user asked whether they could test in a new chat. A separate CLI chat now has a local double-click launcher; opening another ordinary Codex Desktop chat alone does not route it through Headroom.

## Delivered behavior

`scripts/manual-codex.cmd` invokes Windows PowerShell without a profile and requests a bounded local statistics summary. The orchestrator imports `manual-codex-core.psm1`, verifies the known local CLI by installation layout/size/SHA, rejects redirected paths and checks the native process owning the single IPv4 loopback listener at51778. Owner PID/creation identity is checked around the bounded readiness request. Foreign/unready/ambiguous instances stop before Codex starts. HTTP requests disable redirects, proxy and default credentials, with a3s cancellation and16KiB readiness/1MiB stats cap.

The interactive CLI receives only `--no-daemon -s read-only -C <new project-owned UUID workspace> -c openai_base_url=http://127.0.0.1:51778/v1`. There is no model, reasoning, provider, auth, agent-count or log-directory override. The CLI uses its normal login mechanism; the launcher never reads or copies auth stores or modifies permanent client config. The user's normal CLI history settings still apply. No new plaintext TUI capture is enabled. The Turkish [user guide](../testing-codex-manually.md) leaves main/subagent models to the user and includes a model-change test.

After the CLI exits, the launcher may export up to10 newly observed Codex request rows: bounded request ID/model, token counts and measured percentage. Raw content, auth, errors, arbitrary tags and config are discarded. Missing values stay unknown. There are no totals or verified-turn/agent claims; concurrent traffic and warmup rows can be included. A bounded report uses create-new semantics under ignored `work/proofs/manual-codex/<UUID>/`. A zero measured saving remains0%; unavailable data is not0%.

## Review and focused checks

The same sole development worker implemented the launcher; root reviewed the source, guards, parser, report allowlist and failure behavior. Focused checks cover listener identity/readiness, ambiguous/unsafe CLI selection, real project-contained workspace/report creation, preserving an existing report, native argument marshalling with Unicode/spaces, arbitrary model metadata, malformed/partial stats and content/secret exclusion.

Two corrections were required during review/native preparation: actual runtime request IDs use underscores, and PowerShell5.1 cannot create a directory with `New-Item -LiteralPath`. Later, the Explorer-like registered-PATH `-NoProfile` process did not resolve `Get-FileHash`; that exception had been reduced to version-unverified. The final candidate uses bounded .NET SHA256 streams and rechecks the path chain. A small known-digest/size-limit regression covers the fix.

Final worker evidence: **Windows PowerShell5.1 and pwsh7.6.5 each88 assertion PASS**, all three PowerShell files parse with0 errors. Source SHA-256:

| File | SHA-256 |
| --- | --- |
| `scripts/manual-codex.cmd` | `b16df088957b97c38f337638a98135ac9c42990742a2ea6660969be2c14b7ec6` |
| `scripts/manual-codex.ps1` | `cbe8f1264f288271f5140212106e0b8cde5d15e0bcccb9b58a93a0d069f17477` |
| `scripts/manual-codex-core.psm1` | `60343d5bce1d08944294b5065a5ca98a4b3765f8edc42b6a48b4d8bb1d1c9762` |
| `tests/runtime/test_manual_codex_launcher.ps1` | `669480c72c5d8312ba283454611846bf0fa0deff32d55c96f107650cd9492d68` |

Installed CLI is `.11.2`,336,134,960B/SHA `c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a`. This local developer allowlist does not update the product discovery registry or certify later binaries.

The worker confirmed `--no-daemon` in previously read bounded local `.11.2 --help` output, supported by current official documentation; raw help was not retained. Focused tests ran `tests/runtime/test_manual_codex_launcher.ps1` directly in both shells. There is no separate saved worker test-output artifact; the worker's bounded result and the matching source fingerprints are recorded here and reused in root's native metadata.

## Native acceptance

Root reconstructed PATH from the registered machine/user values rather than the assistant's augmented PATH, then ran true Windows PowerShell5.1 `-NoLogo -NoProfile -File scripts/manual-codex.ps1 -CheckOnly`:

| Observation | Result |
| --- | --- |
| No Headroom process/listener | exit2, safe stop; `absent-v2.json` PASS |
| Owned native window, engine Stopped | exit2, not-ready; `stopped.json` PASS |
| OCU accessibility Start → real Engine Ready | exit0; `ready-v2.json` PASS |
| `cmd.exe /d /c scripts/manual-codex.cmd -CheckOnly` with the same PATH | exit0; `wrapper-ready.json` PASS |

The initial pre-fix selection failure is retained as `absent.json` FAIL. An early readiness observation was taken while the UI still showed Starting; it correctly stopped and is retained as `ready.json` FAIL against the premature expectation. Only after a fresh UI Ready observation was the ready check repeated. No failure output was persisted; reports contain fixed reasons and fingerprints.

The unchanged62 application source/config, frontend artifacts and native binary fingerprints matched the accepted H019-d3 candidate. Its185 Rust/65 Node/fmt/frontend/embedded-build PASS evidence was reused, not rerun. Native binary SHA `2d0f64d44cd48943e43b16ce2f5f7a7fe25bd1d3ef877732c59ddbeed768116a`,16,023,552B. No CI dispatch was needed or started for this developer-script slice.

Evidence lives in ignored `work/proofs/manual-codex-qa/`. The own window is PID5012/creation2026-10-23T15:08:42.416575+03:00, launched by the project QA helper. It owns127.0.0.1:51778 and was intentionally left **Ready** for the user's next interactive test. Preferences SHA remained `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`; user client configs/auth were not changed. `handoff.json` records process identity and the open-window handoff. No continuous monitoring was enabled.

This preparation sent **no Codex/model request**. CheckOnly's readiness request may cause the pinned engine's lightweight upstream health probe; it is not inference. A real interactive launch/exit, selected models, changed model, separately correlated main/subagent routes, product Verify, Desktop and general compatibility remain unaccepted. The earlier existing-login live CLI proof is separate and is not repeated here. H019 remains open,15/49 main tasks complete.
