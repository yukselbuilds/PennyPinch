<a name="optimizasyon-hedefleri-ve-kanıt-sınırları"></a>

# Optimization targets and evidence limits

4 November 2026. This note maps sources to existing targets for H012/H025/H034/H041/H043/H044. The lead agent read the RTK and comparison sources on the same day. The evidence covers source inspection only.

<a name="karar"></a>

## Decision

- The main connection architecture uses an endpoint supported by the client and Headroom's local proxy/engine path.
- RTK-OSS Apache-2.0 terminal filters are an optional complementary candidate. They are not part of the default install. The RTK engine and separate proxy use BUSL-1.1 and need a separate license/distribution decision. The team made no decision about automatic import or distribution.
- Preserve the model, reasoning, auth method, and agent count. Do not switch to a cheaper model, route models or reasoning automatically, enable memory/learning, or install global shells/hooks.
- Do not use a blind bypass for unknown auth, route, or protocol. Refresh surface and traffic evidence after a client update when needed. A `not_tested` result leaves compatibility and behavior unknown.

<a name="kaynak--ürün-hedefi"></a>

## Source → product target

| What the source shows | Planned target | Evidence limit |
| --- | --- | --- |
| [RTK v1.1.0 README](https://github.com/rtk-ai/rtk/blob/v1.1.0/README.md) shows a 25 October 2026 publication date. The 4 November GitHub API review showed it as the latest stable version at the time. Its release API `target_commitish` value is [`753a3b0e939f2e4490fc24e3a1984010f40a103c`](https://github.com/rtk-ai/rtk/commit/753a3b0e939f2e4490fc24e3a1984010f40a103c). A hook/wrapper can replace selected terminal commands with RTK equivalents. | H043 tests which tool, command, shell, and client routes are actually intercepted. H044 may offer only a filter that works and shows a benefit. The user must opt in. | Hook success does not transfer to another client. Bash hook success does not prove Windows PowerShell, Codex Desktop, or the application's native Read/Grep tool calls. |
| The RTK counter's gain value is an estimate of about bytes/4. | H041 records command-byte changes as a separate byte metric. H043 shows this estimate for the same input in the RTK arm with a separate label. | Label it as a byte-based estimate. Token differences and billing require their own evidence. |
| The [RTK proxy README](https://github.com/rtk-ai/rtk/blob/develop/proxy/README.md) gives an rtk claude example. | This is a source/license comparison outside H043/H044 scope. | The develop branch can change and must be pinned before real implementation. This note does not plan proxy installation/implementation or Codex/OpenCode support. |
| Pinned desktop comparison: [20 October 2026 research](desktop-comparison-2026-10-20.md), upstream gglucass/headroom-desktop commit 49a6161ebec57e50eb27e2cf04cf7b4990755bd0. | H025 shows token amount and percentage measured on the same scope, the source, and the last update. H034 simplifies errors and next steps. | Source features are not implemented or measured in our product. The research keeps its original data and version limits. |
| Additional pinned source reviewed on 4 November: [gglucass/headroom-desktop commit 67fb0e164df942b71166aeb878791489abf41a9f](https://github.com/gglucass/headroom-desktop/commit/67fb0e164df942b71166aeb878791489abf41a9f), selected README, Codex adapter, and proxy files. | H025 targets simple use and measurement display. H034 targets errors by surface and route. | Source review compares features. It is not evidence for a Headroom implementation, live traffic, or savings. |

<a name="h041-tabanı-ve-h043-genişletmesi"></a>

## H041 baseline and H043 expansion

The H041 first-release gate requires only the **control** arm without Headroom and the **Headroom** arm. Use the same fixed fixture/load, client surface, model, auth, reasoning, agent setting, versions, input, and tool result. Match cold-cache and warm-cache runs separately. Record repeat count, load/duration, and success/latency thresholds before measurement.

The optional RTK evaluation in H043 adds **RTK filter** and **RTK → Headroom combined**. These arms complete the four-arm comparison. Reuse the H041 baseline result only when the relevant fixture, settings, machine/OS, version, and load fingerprints match. Otherwise, record the difference separately. A live-provider test needs separate authorization before it can start.

For each arm, record source, numeric value, identity, and JSON integrity. Also record errors, exit code, stderr result, layer order, command bytes, comparable token basis, and provider cache/output counters. Record first-response and total latency, plus CPU/RAM, separately. Do not include raw conversations or secrets in the report. If the team did not measure quality, byte-exact replay is not a quality guarantee.

- **Command bytes.** Record the byte count before and after. Label RTK's bytes/4 value separately as an estimate.
- **Total input tokens.** Calculate the difference from before and after counts with the same measurement basis. Keep the result unknown when the bases do not match.
- **Billing and cost.** Use an official billable usage measure for any figure labeled measured. Use a versioned price source for any figure labeled estimated. Do not derive an amount from subscription quota or layer percentages.

Do not add layer percentages. Report the combined arm as its own result only when it uses the same work/request binding and an explicit before/after measurement. Do not include a tool in the supported scope if it misses the predefined benefit threshold, breaks data/output integrity, or causes significant delays or errors. Real-user savings and quality guarantees require their own evidence.

The team did not verify a one-to-one mapping between the RTK label and commit in this turn. The automated check rejected the command that queried the label. The team did not retry it. Independently pin the actual version/artifact and license before H043 implementation.
