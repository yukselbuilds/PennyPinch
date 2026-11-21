<a name="dev001--araçlar-ve-kabul-düzeni"></a>

# DEV001 — Development tools and task acceptance

26 October 2026. **Accepted.** No product code changed. At that checkpoint, the product task count was 18/51. H022 had yet to start. DEV002, the audit of existing code, came next.

| Check | Result and limit |
| --- | --- |
| Ponytail installation | The team installed the coding, audit, and review skills only under `.agents/skills` at commit `09c434ab73ca43761c5d8b0c5d341926c2fba710`. The three files matched the source Git blobs byte for byte. The installation preserved the MIT license. |
| Source record | `docs/development/ponytail-source.json` records the source pin, hashes, and command arguments. Local receipts contain machine-specific installation details. |
| CodeGraph | The project kept the pinned 1.6.2 runtime, launcher, and MCP registration. The exclusion list included `.agents/` and had 59 entries. After the config change, sync, status, and file list checks passed. They showed 76 source entries and zero `.agents` entries. Rust and TypeScript targets were present. The final indentation fix left the parsed config intact, so the previous results still applied. |
| Live MCP | The actual `codegraph_explore` tool in this chat answered the `undo_owned_endpoint`, `maxFiles: 1` query successfully. It found the symbol and `transaction.rs`. The response had 15,331 characters. Only result metadata was saved. |
| Task acceptance | The workflow linked the five shared acceptance conditions, short worker handoff, existing commands, and evidence reuse. The team specified observable scenarios for H022. It removed missing scripts from H039's acceptance steps. |
| Product tests | All 73 source and config inputs still matched the previous H021 gate. The tool, skill, and document changes relied on that historical test result. |

The lead agent read `ponytail-review` to inspect the changes and related rules. The review made the source manifest portable, preserved the actual arguments for reuse, and removed an unnecessary config indentation change. At that checkpoint, the coding level was `full` and reports used Turkish. The workflow kept the security, rollback, and test rules and the single worker.

The lead agent verified the skill file locations, metadata, and source matches. The lead agent read the review file directly. Automatic discovery by the running host remained unverified. The team expected discovery in the next turn. The live CodeGraph MCP query had its own evidence. The work left global settings untouched. It included no provider or model test, CI run, or push. Token savings were unmeasured.

The `work/proofs/dev-tools/` directory contains local receipts `ponytail-install-v1.json`, `mcp-live-root.json`, and `root-acceptance.json`. The last file records document links, the task count, the protected checkpoint boundary, and final file fingerprints. The run retained no raw query, source, or error dump.

[Shared workflow and completion acceptance](../development/WORKFLOW.md) · [Ponytail](../development/ponytail.md) · [CodeGraph](../development/codegraph.md)
