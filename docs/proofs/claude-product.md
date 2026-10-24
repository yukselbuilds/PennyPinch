# Claude Code product connection — H020

24 October 2026. Accepted local product slice on Windows 10 Pro 22H2/build19045 x64. This is fixture/config coverage, not live provider or universal Claude compatibility.

## Product behavior

The common Graphite connection dialog supports user settings and an explicitly chosen project's `.claude/settings.local.json`. Native selection accepts no renderer config path. Preview, atomic Apply, scoped ownership and conflict-preserving Undo use the same manager and writer as the existing clients. One Claude target may be active; project selection is memory-only and the original directory must be reselected after restart for Undo. An existing safe parent directory is required. Parent creation and remembered context remain H022 work.

Readonly discovery recognizes the measured Claude Code2.1.158 executable only at236,306,080 bytes/SHA256 `10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74`. Other binaries remain unknown. No model allowlist, per-agent installation or automatic model/reasoning/auth change was added.

User/shared/local and the fixed Windows managed-file source are observed separately. Route environment observations describe Headroom's process, not another Claude process. The UI separates target-file declarations and aggregate declarations in observed files from unknown effective runtime settings, remote/registry policy and other sessions. Observed conflicting routes block a plan. Non-target file/environment drift invalidates Apply; unobserved policy does not automatically prohibit all local edits. Undo preserves unrelated changes and is bound to the original target. Directory identity replacement, deletion and reparse changes fail closed.

## Final local quality gate

After root review and corrections, once on the frozen candidate:

- `pnpm test:app`:90 PASS.
- `pnpm build`: TypeScript and embedded frontend PASS.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`:PASS.
- `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1`:224 PASS/3 ignored.
- `cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol`:PASS. Four known warning causes are unused test-injected `roots` bindings and existing dead-code helper groups; the captured count also includes Cargo's summary warning line.

Toolchain: Node25.2.1, pnpm11.25.0, Rust/Cargo1.92.0. CARGO_HOME/target remain project-local under `work/caches/`. Final binary: `work/caches/cargo-target/debug/headroom-desktop.exe`,16,627,200 bytes, SHA256 `fbce4c822a40a0a23a6995b9189bbc1ce03a1f68885a2b65ba1485de6dbbe3b3`. All70 source/config and3 frontend artifact fingerprints matched after native acceptance. No successful gate was rerun because of a context/credit interruption.

Bounded gate fingerprints/results: `work/proofs/claude-product/{frontend-quality,final-quality}.json`. Focused RED/GREEN and root review findings: [H020 review](../reviews/H020.md). No new dependency or engine pin/allowlist change was needed.

## Actual native UI → config → CLI → engine

Controlled run `4dc29937-a882-4459-9af8-dd1028020a10` used isolated home/config/cache/temp directories and a synthetic project repository named `Project scope İstanbul`. User/shared settings declared Sonnet/medium; local settings declared Opus/high. Auth was a synthetic sentinel in user settings, with empty hooks and tools disabled. The helper did not read or copy a real auth store. CLI flags did not override endpoint, model, effort or auth.

The coordinator operated the real embedded Tauri window through Open Computer Use/UI Automation and guarded owned Win32 dialog controls:

1. Start and exact pinned native CLI discovery worked. User Preview → project picker Cancel retained user scope and the existing preview.
2. User Apply changed the endpoint while model/effort/synthetic auth/hooks and non-target files stayed intact. User Undo restored all three fixture config files byte for byte.
3. Native selection of the Unicode/space project path cleared the old preview. Project Preview → Apply succeeded after the released user record, covering the root's scope-eligibility regression.
4. Normal close exited0 and retained the active project ownership record. Reopen found that record; reselecting the original project restored the context for Undo.
5. The actual Claude CLI loaded project configuration and sent exactly one streaming Messages request through GUI-owned stable ingress `127.0.0.1:51778` → the owned Headroom0.41.0 engine → the loopback Anthropic fixture. Request model `claude-opus-4-6`, effort `high`, synthetic auth/header, tools0 and streaming were preserved. A partial marker was observed before the gated terminal event; final success/marker, exit0, no malformed/error events and no fixture timeout were observed.
6. Project Undo after reopen restored all three config files byte for byte. UI Stop reached Stopped, then normal close exited0. Preferences and owned engine workspace settings were restored on both launches. Final owned process tree/listeners:0/0; the fixture servers were closed.

The native QA upstream used the pinned engine's existing owned workspace `settings.json` support, preserving/restoring the prior file. It did not change the production manifest, environment allowlist or user client configuration. Detailed bounded metadata: `work/proofs/claude-product/native-acceptance.json`, `user-{apply,undo}.json`, `project-{apply,undo}.json`, `closed.json`, and the run's `launch-01/02.json` and `cli-result-02.json`.

## Diagnosis and remaining limits

The first test launcher exited1 before any fixture request because variadic `--tools` consumed the prompt. Adding the argument separator fixed the launcher; application sources and their gate remained unchanged. Historical `cli-result.json` has request_count0; its old unconditional upstream-observed flag is not evidence. The successful second-launch record is authoritative.

The isolated Windows shell picker showed an initial unavailable-location warning. Its owned OK control was dismissed and the fixture folder selected directly. The test helper's ANSI character/selection-clearing issue was corrected with Unicode messages and explicit select-all/clear; the correct final Unicode path was observed in the product UI. These were launcher/dialog-helper corrections, not product source fixes. Screenshots were not used as visual acceptance, and no physical mouse/global input or continuous monitoring is claimed. Picker initial-location handling remains a usability follow-up in H022.

The engine reported a single synthetic input row87→87, saved0/0%; the fixture terminal supplied synthetic usage12/4. Neither is a real user savings or billing measurement. This development chat's token savings are unmeasured.

The user declined Claude sign-in. OAuth/live API-key providers, actual model feature behavior, tools/hooks/plugins/MCP, main/subagent attribution, CCR, Desktop, Win11/macOS/Linux, clean installer and release readiness remain untested here. Registry/remote/session policy is unknown. No live model request, paid CI, push or release was performed. [Compatibility catalog](../../compatibility/clients.json) records the bounded product combination separately from H009's earlier bare-CLI fixture.
