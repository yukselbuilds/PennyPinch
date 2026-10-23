# User-operated Codex CLI test

23 October2026. The user ran the H019-d4 launcher, shared its exit/statistics output and explicitly authorized root to run a follow-up CLI agent test. This accepts the **interactive launcher/normal exit path**, not the product verifier or general model/agent compatibility.

The supplied launcher run `dece1327-a5c0-4825-a3ca-317d8e59ad20` ended with CLI exit0. Its allowlisted report is under ignored `work/proofs/manual-codex/<run>/launcher-report.json`. It contains new Codex request records for `gpt-6.1-sol` and `gpt-6-luna`, token metadata and measured per-request percentages. The report correctly leaves actor identity and user-turn completion unknown and does not aggregate rows.

Root inspected **only the explicitly supplied test thread** `01a168b3-181c-75f3-9b03-555a7b3d14b3`, reducing its existing CLI rollout in memory to tool/lifecycle/model metadata and fixed marker booleans. Auth/config and unrelated conversations were not read; raw prompts, answers and tool errors were not exported.

- Parent turn metadata records both models. Two real `spawn_agent` calls requested `gpt-6-luna`, `xhigh`, `fork_turns=none`; tool results returned separate canonical task names.
- Runtime `SubAgentActivity` items record started/completed lifecycle for children `01a168b7-3d62-7f71-916f-c13cb04fb282` and `01a168b7-f124-7740-ad1a-df6c1a35ed05`, with matching task paths. `CollabAgentToolCall` records completed wait operations.
- First-class `response_item/agent_message` records carry the matching child author path, recipient `/root` and expected `HEADROOM_CHILD_OK` marker. This establishes real child creation, completion and return to the parent, beyond the parent's natural-language claim.

Sanitized evidence: `work/proofs/codex-agent-observation/user-thread-metadata-v3.json`. Earlier extraction versions are preserved; empty UUID/notification fields reflected missing parser coverage of the new first-class event shape. No model request was made to inspect this completed run.

The displayed request with134 output tokens records43,678→43,224 input tokens,454 saved,1.0394%. Two3.4206% rows have0 output tokens and are not added to a claimed response-turn saving. Other rows have real0% measurements. CLI provider cached-token usage is not Headroom compression savings. The report contains only10 recent rows and is not a complete session ledger; no session-wide savings percentage is claimed.

The pinned engine's `RequestLog` deliberately has no session/thread field; a source comment says it was removed rather than populated. `/stats` therefore cannot bind a row to either child from model name alone. Root subsequently ran [one bounded live CLI probe](codex-agent-probe.md); it completed but exporter/header identity gaps prevented agent/route acceptance. Global config, auth and product source were unchanged. Product Verify, Desktop, separately correlated per-agent savings and main H019 remain open;15/49 main tasks complete.
