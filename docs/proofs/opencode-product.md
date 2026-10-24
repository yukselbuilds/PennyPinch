# OpenCode product connection — H021

24 October2026. Accepted local product slice on Windows10 Pro22H2/build19045 x64. This proves the bounded native config/fixture combination below; live-provider and release support remain separate.

## Product behavior

The common Graphite connection dialog inventories every eligible provider alias in the selected user JSON/JSONC file. One Preview/Apply operation uses the existing atomic config/ownership manager and JSONC span writer. Built-in `openai` uses Responses; explicit ordinary `@ai-sdk/openai-compatible` aliases use Chat Completions when their original upstream is exactly standard OpenAI. Custom hosts, other SDKs and ambiguous route overrides stay unchanged and are counted as exclusions. Provider/model/small-model/agent choices, selector versus wire model IDs, auth/options and unrelated bytes are preserved. There is no model-name allowlist or per-agent setup requirement.

Coverage counts, protocol families, configured-default coverage and partial/runtime-unknown status are visible. An excluded default explicitly says that this file edit will not cover that model's traffic. Technical endpoint/declaration details are collapsed under Advanced. Provider keys remain native-only; the renderer sends only a plan ID and config fingerprint. Codex/Claude contracts and presentation are preserved.

Observed process `OPENCODE_CONFIG`, `OPENCODE_CONFIG_CONTENT` or `OPENCODE_CONFIG_DIR` values are consumed into a memory-only digest. A nonempty override blocks Preview; drift between reads or before Apply commit conflicts. Override files/auth stores are not inspected or copied. Other process/project/managed/remote/plugin runtime effects remain unknown. An empty config has no known eligible provider and is blocked instead of inventing a default.

Receipts persist at most16 unique domain-bound provider-key digests, creation masks and validated canonical public URL literals. Already-same routes gain no ownership. Undo verifies every owned route before changing bytes, preserves edits to unowned routes and refuses a missing/edited owned route. Schema1 legacy single-openai receipts remain readable. Full config/auth/custom URL backups are not stored.

## Pins and final local quality

Exact readonly native discovery pin: OpenCode1.19.3,180,982,664 bytes, SHA256 `cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375`. Unknown native binaries/versions remain unknown/blocked; candidates are not executed for discovery. Official source tag resolved to commit `e3c4eb242585dcd77be1a867391b7c25fd47ce52`; reviewed sources/URLs/hashes are in `work/proofs/opencode-product/source-pin.json`. [Decision009](../decisions/009-opencode-provider-routes.md) defines the bounded profiles. H010's inline-config bare-CLI proof is separate.

After root review and focused RED/GREEN regressions, once on the frozen final candidate:

- `pnpm test:app`:92 PASS; `pnpm build`: TypeScript/Vite PASS.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`:PASS.
- `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1`:252 PASS/3 ignored.
- `cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol`:PASS;5 warning lines including Cargo's summary were recorded.

Node25.2.1/pnpm11.25.0/Rust-Cargo1.92.0. Cargo home/target are project-local under `work/caches/cargo-home` and `work/caches/cargo-target`. Binary16,812,032 bytes, SHA256 `54ca0352f68ef37ef2e2905da40b8bcdeafe21eac9b56b012e70397ef63ca8be`. All73 source/config and3 frontend artifact fingerprints, plus the binary, matched after native acceptance. Focused passes were reused when relevant fingerprints matched; compact/credit interruptions did not cause repeated gates. Gate metadata: `work/proofs/opencode-product/{frontend-quality,final-quality}.json`; [source review](../reviews/H021.md).

## Actual native UI → saved config → CLI → engine

Controlled run `d1bd914b-a961-47de-8600-ea4dcc6f7dd2` isolated HOME/USERPROFILE/XDG/cache/temp/ProgramData and a Unicode/space project repository. Its user JSONC had two compatible aliases with selector IDs different from wire IDs, a built-in OpenAI alias, one excluded custom-provider host, comments, Unicode and an escaped canonical URL token. Auth was synthetic via a config environment reference; no real auth store was read/copied by the helper. Explicit schema avoided the CLI's automatic schema insertion.

The coordinator operated the real embedded Tauri window with Open Computer Use/UI Automation:

1. Start reached Ready; PATH discovery found exact native1.19.3. Preview showed3 eligible/1 excluded, Responses1/Chat2, covered default and partial/runtime-unknown notes. Advanced expanded/collapsed through native accessibility.
2. Apply saved all3 eligible endpoints to the GUI-owned stable ingress. Independent config/receipt checks confirmed unchanged selectors/wire IDs/auth/options/provider selection/comments/Unicode and excluded provider; no alias/auth/custom host was retained in ownership.
3. Normal close exited0 with active ownership retained and owned tree/listeners0. Reopen found the active record and restarted the engine.
4. The actual exact CLI loaded the saved user JSONC without inline config or endpoint/auth CLI overrides. First run omitted `--model` and adopted saved `lab-a/slot` → wire `h021-wire-a`; second selected the synthetic `lab-b/other` → `h021-wire-b`; both used Chat Completions. Third selected synthetic `openai/response-slot` → `gpt-4.1` and used Responses.
5. All3 runs exited0, each with exactly one streaming POST through GUI-owned ingress`127.0.0.1:51778` → owned Headroom0.41.0 → loopback fixture. Sentinel auth, tools0, protocol path/wire model, final marker/stop and synthetic usage12/4 were accepted; no malformed/oversized/error events. These are actual CLI protocol checks against synthetic responses, not live model inference or first-token latency proof.
6. Undo after reopen restored original JSONC bytes exactly, including escaped URL/comments. Stop reached Stopped; normal close exited0. Preferences and owned engine settings were restored on both launches. Final owned GUI/descendant/listener counts0/0; fixture servers closed. One automatically generated synthetic CLI log was deleted; helper stdout/stderr/HTTP bodies were never persisted.

The QA upstream used the pinned engine's existing owned workspace settings, preserving/restoring its prior value. Manifest/engine pins/allowlist and global user client/auth config were not changed. No physical pointer/global input, screenshot visual acceptance or continuous monitoring is claimed.

Bounded evidence: `work/proofs/opencode-product/native-acceptance.json`, `config-{apply,undo}.json`, `traffic.json`, `closed-{first,final}.json`; run `launch-01/02.json` and `cli-result-02-01/02/03.json`. Prepared helper hashes are recorded in acceptance metadata. Both transport cases passed on the first native attempt; no application source fix or repeated quality gate was needed.

## Measurement and remaining limits

The fixture reports artificial usage12 input/4 output. Raw engine rows report saved0/%0; the two Chat rows show original2306/optimized12 on different measurement bases. Subtracting those fields would invent savings. They are not real user, billing or development-chat savings. H023 already requires comparable measurement pairs; this exact regression observation is added there. Actor/session/main-subagent identity remains unknown.

Model-independent routing within an eligible profile does not establish every model feature, provider, OpenCode version or agent configuration. Live OAuth/API-key accounts, provider inference/tools/plugins/MCP, actual subagents/attribution/CCR, Desktop/V2, Win11/macOS/Linux, clean installer and release readiness remain untested here. JSON/JSONC resolver/patch fixtures passed locally; the actual native client adoption run used JSONC. No model/account call, paid CI, push or release occurred. [Compatibility catalog](../../compatibility/clients.json) records this bounded product combination separately from H010.
