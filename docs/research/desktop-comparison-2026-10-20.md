<a name="headroom-desktop-karşılaştırması"></a>

# Headroom Desktop comparison

20 October 2026. The reviewed source config lists app version 0.9.45 and engine version 0.41.0. These values describe the pinned source. [Reviewed repository](https://github.com/gglucass/headroom-desktop) · [`49a6161ebec57e50eb27e2cf04cf7b4990755bd0`](https://github.com/gglucass/headroom-desktop/commit/49a6161ebec57e50eb27e2cf04cf7b4990755bd0)

The review copied 34 selected source and documentation files. It statically reviewed relevant feature, proxy, client, metric, update, and Windows sections. SHA-256 records for the 5,303,471 downloaded bytes are in the project file `work/research/gglucass-headroom-desktop/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/source-manifest.json`. The reviewer did not run instructions from the external repository. The reviewer installed no dependencies or installer and did not start the application or change user client settings. This work is not a line-by-line review of every file, a full security audit, or evidence of operation on both Windows versions.

<a name="genel-sonuç"></a>

## Overall result

At the time of the review, the other project was further along with the same product idea. Our engine management and preference storage worked. Client connections, the real savings store and screen, tray support, and the installer were still in progress. Therefore, a feature list alone does not show that our equivalent features work. This comparison separates **implemented**, **already planned**, and **new requirement** items.

Both architectures use Tauri and Rust with a React interface and a managed Python Headroom engine. Both reviewed source trees use engine version 0.41.0. The source is a useful research input, especially for the Codex ChatGPT route and Windows recovery cases. It does not replace our version, auth, or protocol evidence.

<a name="özellik-karşılığı"></a>

## Feature mapping

| Area | Equivalent behavior in the reviewed source | Our actual status / decision |
| --- | --- | --- |
| Codex, Claude Code, and OpenCode connections | `client_adapters.rs` and Codex ChatGPT routing in `proxy_intercept.rs` | H008–H010 have partial technical evidence. Product connections remain open in H016–H022. They are already planned but not implemented. |
| Setup and connection health | Onboarding, an installation-stuck screen, and diagnostics for an unrouted client | H022/H034 clarify scope. Show found, config written, and real traffic observed as separate states. |
| Stable client endpoint | Fixed Rust ingress port plus a changeable Python backend port | Existing H014 manages only the owned engine. The persistent client-endpoint contract is missing. New H040 comes before config writes in H017. |
| Daily/monthly and client breakdown | `dashboardHelpers.ts`, `state.rs`, `activity_facts.rs` | H023–H027 are already planned. Our real screen is still empty. Preserve scope and measurement basis. |
| Cache awareness | New input/cache cost and separate output estimates | Our metric contract has the basic approach. New H041 covers cache, quality, and latency regressions. |
| System tray, shutdown, and recovery | Tray, watchdog, health, and bypass routes | H030/H034 are already planned. H040 will define the connection lifecycle in detail. |
| Updates | Tauri updater public key, release channels, and signed-artifact checks | H036 compatibility/update behavior alone is not a real updater. New H042 covers it. |
| RTK | Optional CLI output compression and client-specific hooks/wrappers | It was not in our plan. The team added H043 technical evidence and H044 controlled product integration. RTK starts off. |
| Other add-on tools | Document/MCP/learning tools and an Addons view | We will not install all tools as a group. Each tool needs separate license, process/data, protocol, and benefit evidence. RTK is the first candidate. |
| Local recommendations | Daily analysis/learning and health signals | H045 covers local recommendations derived from statistics without LLM calls. Automatic instruction writing and learning stay off. |
| Windows, macOS, Linux, and Grok | Extra platform/client code and release flows | Our targets are Windows 10/11 and the three current clients. This comparison does not add other platforms or Grok to the first release. |
| Accounts and subscriptions | The README says users need an account and an active plan | We will not add an account/subscription requirement to our local core. A cloud account remains a separate product decision. |

<a name="alınacak-güçlü-yaklaşımlar"></a>

## Strong approaches to adopt

1. **The client endpoint should not depend on the engine generation.** Prevent a client config from pointing at an old temporary port during engine restart. A fixed ingress with a changing internal engine port is a strong candidate. Two proxies are not a required decision. H040-a will compare alternatives and select one using ownership, latency, crash, and shutdown evidence. We will not copy a fixed port such as 6767 without review.
2. **Installation and traffic flow are separate signals.** Show version, effective settings, the last real request, and wrong or missing scope. Give the user a targeted next step. Diagnostics do not need prompt or auth content.
3. **Show measurement scope.** Separating source, cache, and output estimates is useful. Our main card will keep amount and percentage on the same measurement basis. It will show partial coverage, negative results, and unknown values.
4. **Use verified artifacts for updates.** Track app and engine versions separately. Do not start installation after a signature/hash error. Do not force a change during an active stream. Preserve state and the last working version after a failed update.
5. **Make extra compression layers optional.** RTK can reduce some terminal output before a model request. Prove its behavior separately for each client and tool. Claude hook success does not prove Codex support. Do not add layer percentages or token counts directly.
6. **Keep the core light and recommendations local.** Avoid making heavy ML/MCP tools mandatory in the base install. Evaluate daily state with deterministic rules. This limits the program's own usage cost.

<a name="doğrudan-taşınmayacak-davranışlar-ve-ölçüm-sınırlamaları"></a>

## Behaviors and measurement limits not to transfer directly

<a name="mevcut-codex-sohbetlerini-topluca-değiştirmek"></a>

### Bulk changes to existing Codex chats

[`client_adapters.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/client_adapters.rs#L5163) finds existing state databases and updates `model_provider` for every matching thread. The SQL operation is at L5321. This has a real scope impact. It is not, by itself, a claim of a security vulnerability. Our default connection will not change an existing chat database. Do not apply such a migration without a separate, explicit, scoped, reversible user flow and evidence. The team strengthened acceptance for H008/H017–H019.

<a name="tasarruf-oranı-ile-faturaabonelik-kazancını-eşitlemek"></a>

### Do not equate savings rates with billing or subscription savings

Some helpers in [`dashboardHelpers.ts`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src/lib/dashboardHelpers.ts#L92) clamp negative input values to zero. The old cache-cost path also has a `/9` fallback. The code comment says this does not match every model's discount rate. We will not transfer these rules to our main token formula. Show increases as negative savings. Show an unknown amount when the cache price is unknown. These limits do not mean every counter in that project is wrong.

[`output_savings.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/output_savings.rs) applies sample-count, coverage, and confidence-interval filters against old/upstream estimate problems. We will not present historical problems described in comments as unfixed bugs in today's application. The lesson is to avoid labeling an estimate as a real measurement. Our output shaper stays off.

The README's approximately 50% or 2x claims do not prove our users' savings. This project has not measured real user token savings. Access to the original content through CCR does not guarantee quality for every answer. H041 will preserve this distinction in tests and product claims.

<a name="pause-ve-doğrudan-iletim-her-protokolde-güvenli-varsayılmamalı"></a>

### Do not assume pause and direct forwarding are safe for every protocol

Removing routes and reverting the thread provider during pause or quit is the other project's lifecycle choice. We will not use the same method until we test active streams, prior compressed history, prefix/cache, and CCR access together. Do not send a second POST automatically after the engine sent an upstream request. Do not restart in the middle of a stream.

The source uses different fallback targets for Codex with ChatGPT and for the OpenAI API-key route. [`proxy_intercept.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/proxy_intercept.rs#L1128) We will not enable an unverified auth/gateway route as a general fail-open path. An unknown route must not send user traffic to a guessed target.

<a name="kapsam-ve-sürüm-bilgisi-belgeyle-sınırlı-kalmamalı"></a>

### Verify scope and versions beyond the documentation

`docs/architecture.md` and the old tool matrix describe RTK as required and macOS as the only polished target. The current README and Windows checklist describe RTK as opt-in. The Responses timeout document also describes an exact-pin fix for 0.37.0. We cannot treat these documents as a Windows 0.41.0 guarantee or current patch instructions. H036 will compare documents, catalog entries, and real versions. A test or CI job in that source repository does not mean that test passed in our environment.

<a name="hesap-öğrenme-ve-büyük-bağımlılık-paketini-taşımamak"></a>

### Do not transfer accounts, learning, or a large dependency package

This comparison does not enable accounts/subscriptions, remote analytics, automatic AGENTS/instruction writing, memory/learning, or the output shaper. Local statistics stay enabled. The anonymous beacon stays off. The README's approximately 3GB runtime and our approximately 523MB development payload are different components and scopes. They do not support a release-size comparison or speed guarantee. H022/H037 will show the base engine and optional heavy components separately.

<a name="yol-haritasına-etkisi"></a>

## Roadmap impact

The team added new main task IDs at the end to preserve old evidence links. **Number order is not implementation order.**

| Task | Priority / order | Goal |
| --- | --- | --- |
| H040 | First release, after H016 and before H017 | Stable client endpoint, ownership, and verified failure path |
| H041 | First release, after H023 and before release acceptance | Cache, content integrity, and latency/benefit regressions |
| H042 | First release, after installer/uninstall and before H039 | Signed updates, channels, and recovery |
| H043 | After the first usable release, off by default | Real technical, client, and protection evidence for RTK |
| H044 | After H043 | Optional RTK install/enable/remove flow |
| H045 | After the first usable release | Local health/savings recommendations from metadata |

Acceptance conditions for H008, H017–H019, H022–H023, H025, H027–H028, H030, H034, and H036–H039 were also strengthened. The team did not open a second main task for the same feature. The new scope was not marked complete. At this checkpoint, 9 of 45 main tasks were complete. H016 had not started.

<a name="kaynak-aktarımı-sınırı"></a>

## Source-transfer limits

The pinned repository includes an MIT license. The work changed only research and planning documents. [License at the reviewed commit](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/LICENSE)

For a later code transfer, record the pinned commit and file in the H037 component and NOTICE record. Include required copyright and license notices, the changes, and our own tests. The local app remains independent of the other project's private account backend.

Stop after this comparison task. H016 or any new added task starts only after the user's next continuation message.
