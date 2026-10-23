# H019-d5 — Root-operated CLI agent probe

23 October2026, Windows10 Pro22H2 x64/build19045. **One live probe executed; result PARTIAL, not agent/route acceptance.** The user's separate manual test already proves two real children completed and returned to their parent; see [that evidence](codex-user-manual-test.md).

## Probe and safeguards

The user explicitly invited root to test the CLI. The sole development worker prepared ignored `work/proofs/codex-agent-check/{agent_check,test_agent_check}.py`; root reviewed the boundaries and ran the live probe. The final files have SHA256 `babaf8c6dcfb9cde372ad696498d31258568312a27df8fede4b3979628c2a1e2` and `ec217ae352b23b624551520141f3f957dcd4e8d46e18a8638302e860e31aaea5`. Worker Python3.14.0 Windows evidence:16 focused tests PASS and `py_compile` PASS. Checks include opaque byte/half-close preservation, UUID/header redaction, duplicate/partial lifecycle rejection, native peer direction using a separate Python connector process and a single-run lock. No model call was used for those checks.

Root corrected an initial peer tuple direction/PID assignment, stale ten-row envelope assumption, marker-prefix match, native response-item parsing and non-finite stats handling before the probe. The final preflight reused the guarded launcher CheckOnly and passed against the exact installed CLI `.11.2`,336,134,960B/SHA `c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a` and owned ingress/ready0.41.0 engine.

The CLI used its normal cached login, `--no-daemon exec --ephemeral --json`, a read-only fresh project UUID workspace and a process-only endpoint override. The root model/reasoning/provider were not overridden. The prompt requested exactly one test child with the user's selected `gpt-6-luna`, fork-none, inherited effort and no child tools/files, with a unique marker and parent wait. This is a test combination, not a model restriction. No auth/config store was manually read or copied; no new login/key, global setting, extra plaintext TUI log or client connection Apply was used.

A temporary exclusive loopback observation relay forwarded bytes to the existing owned ingress. It inspected only the first bounded HTTP header per connection, exporting validated thread/turn UUID comparisons rather than auth, other headers or bodies. CLI JSONL was reduced in memory to fixed booleans/counts; raw content/errors were discarded. This extra observation relay is a development instrument, not the production connection path or Verify implementation.

## Observations

- Live run `f98d79fa-e5ec-4f7c-ab3e-28db47256894` ended with CLI **exit0** and `turn_completed`. There was no second live probe or automatic rerun.
- The actual exporter used `thread_started` and `collab_tool_call`, not all of the runtime/dotted event shapes modeled by the collector. Nine events included two error items, two agent-message items and one collaboration item. Their raw text was not retained. The collector therefore reports lifecycle **partial** with0 decoded spawns; this is missing collector coverage, not evidence that no child ran. The two error-item causes remain unclassified.
- All8 observed CLI peers and all8 relay-to-native ingress peers matched the expected process ownership. No usable thread UUID was obtained from the first headers; child-thread correlation remained0/unknown. The relay recorded2 incomplete/error closures; their causes were not classified or promoted to success.
- Root's independent55.01s/1318-sample TCP watch observed the identified GUI PID5012/port51778 forwarding to pinned engine PID21296/port56485, with both owned listeners present. This is a transport-leg witness, not a request-to-child attribution.
- Six new Codex-tagged stat rows included `gpt-6.1-sol` and `gpt-6-luna`. The output-bearing rows measured **0 tokens/0% saved**. Separate0-output rows measured464/3.3545% and465/3.4206%; they are excluded from any response-turn saving. No session total, agent-attributed percentage or cache-based saving is claimed.

Evidence: ignored `work/proofs/codex-agent-check/runs/<run>/summary.json`, `work/proofs/codex-agent-observation/root-probe-route-watch.json`. A read-only SQLite query filtered only to the new UUID workspace found0 persisted threads, as expected for this ephemeral probe; no unrelated thread content or auth was read. The missing live event details cannot be reconstructed from that query and were not invented.

## Cleanup and remaining work

CLI process exited; observer listener and worker threads closed. The owned native Headroom window/engine remained Ready for the user, with the same process creation identities. Preferences SHA remained `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. The finite TCP watcher exited; no continuous monitoring was enabled.

All65 existing application source/config/artifact hashes and the native binary hash matched the accepted H019-d3 candidate. No product source changed, so the existing185 Rust/65 Node/build evidence was not rerun; the new ignored harness had its own focused checks. No CI was started. `application-fingerprint-match.json` records the match.

**H019-d5 stays open.** Next implementation must cover the installed CLI exporter schema and establish trusted child/request identity without raw content capture; first-header-only observation was insufficient here. Tests need actual bounded schema evidence rather than assuming native rollout and exec JSONL are identical. Product Verify, Desktop, general model/agent compatibility and per-agent savings remain unaccepted. Main H019 is open,15/49 complete.
