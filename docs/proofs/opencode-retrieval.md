<a name="h010-b3--opencode-ürün-retrieval-kabulü"></a>

# H010-b3 — OpenCode product retrieval acceptance

14 November 2026. **Windows 10 22H2 x64, build 19045. OpenCode 1.19.3. Headroom engine 0.41.0.** This local product slice was accepted. Main H010 and total 42/54 remained unchanged. [Programmatic summary](opencode-retrieval.json), [review](../reviews/H010-b3.md).

<a name="gerçek-ürün-yolu"></a>

## Real product path

In the native release window, OpenCode Preview → Apply saved the two eligible provider endpoints and the `mcp.headroom` entry in the user's JSONC. It used the existing atomic writer/ownership flow. The CLI read this file as its normal user config. No `OPENCODE_CONFIG_CONTENT`, custom endpoint override or test retrieval bridge flag was set. Auth/HOME/XDG/workspace were separate for the test. The test did not read the real account or user chats.

The data MCP tool returns only fixed synthetic logs/witness. The two relevant MCP tools were explicitly allowed. Built-in shell/task tools were not enabled in this acceptance.

| Scenario | Observed result |
| --- | --- |
| Preview and close | Config and ownership were identical at byte/hash level. No connection had been written yet. |
| Chat Completions, `@ai-sdk/openai-compatible` | Data → real `headroom_retrieve` MCP call → witness. Three tools completed. Four upstream messages were sent, and streaming remained intact. |
| Responses, `@ai-sdk/openai` | Data and witness completed. For four upstream messages, the engine used server-side retrieval. Upstream remained buffered. |
| Content from both protocols | The original 14,999-byte block became 103 bytes. Retrieved content matched by bytes/SHA. Model/synthetic auth/call_id and final result were correct. CLI exit0/stderr0. |
| Pause/full stop | The existing CCR content was retrieved exactly while the engine was paused. On full stop, the bridge returned a fixed error. |
| Normal close and reopen | New GUI PID with the same endpoint identity/port/config/owner. Unexpired CCR content was retrieved exactly again. Both GUI processes exited 0. No force fallback occurred. |
| Undo cancellation/conflict | Cancellation wrote nothing. Changing the owned MCP timeout caused `config_conflict`. File and owner stayed unchanged. Only this test's own change was restored. |
| Normal Undo | The original commented JSONC returned byte-for-byte. The owner was released, and the owned listener closed. |
| Different-volume boundary | The existing atomic writer returned `unsupported_filesystem`. Config/owner did not change. The successful flow ran on the same volume. |

Model selectors were `lab/slot` in Chat and `openai/slot` in Responses. Wire names were separate fixture inputs. No model-name filter or automatic reasoning/model change was added. Reasoning selection was not tested. A single block's byte reduction is not a total-token/cache/billing saving, model quality or long-term stability measurement. CCR carries the original content again.

<a name="köprü-ve-kaldırma-sınırı"></a>

## Bridge and removal limits

OpenCode starts the packaged, native-hash-verified stdlib Python bridge over stdio. The renderer does not provide a command/path/port. The tool accepts only a hash of 12–24 hex characters. It rejects URL/path/shell/auth input.

Before HTTP application bytes, it validates the bounded endpoint record, fixed endpoint UUID and PID/image/same-user SID ownership of the established socket. The fixed loopback `/v1/retrieve`, socket independent of proxy settings, 64KiB MCP input, 8MiB response and 10-second total deadline are bounded. Missing/truncated content does not count as success. stdout carries no non-MCP logs or secrets.

The release bridge holds the existing `.desktop-use` lock. The real release `--headroom-uninstall keep` entry was also run. While the bridge was open, the exclusive gate was blocked by Windows sharing violation 32. After stdin closed, the bridge exited 0/stderr0 and released the lock.

In a conflict representing the user's MCP change, removal exited 10 with config+owner unchanged. In the normal case, exit0/byte-exact JSONC/released owner and preserved state passed. This narrow entry test does not re-accept full NSIS install/uninstall, updater distribution or production release.

<a name="doğrulama-ve-kaynak-eşleşmesi"></a>

## Verification and source matching

The final candidate passed **222 Node, 448 Rust (18 ignored), fmt, TypeScript/frontend and embedded custom-protocol build**. Seven Python bridge tests cover real Windows socket ownership/gate, EOF, boundaries, errors and Unicode. Seven Rust retrieval tests and ten Node plan tests also passed. The 332 source/test/config inputs, dependency and tool fingerprints for the final gate and native runs matched before and after. Hashes were not copied by hand. The canonical JSON programmatically links source, executable, package manifest and result files.

- Full local gate: `work/proofs/opencode-retrieval/final-74f47538b0.json`.
- Focused verification: `work/proofs/opencode-retrieval/focused-7d784ff844.json`.
- Real GUI/CLI: `work/proofs/opencode-retrieval/native/ce8ea443ed/same-volume/result.json`.
- Release removal entry: `removal/result.json` in the same candidate.
- Preview cancellation and volume rejection: separate hash checks in `rerun/result.json` for the same candidate. This general run did not count as successful.

Staging verified 22,325 payload files/533,082,296 bytes. There are 22,334 resources with the bridge and added sources. The native release executable and immutable staged sources were verified against the same manifest.

The test profile stayed under a separate app-owned state root. Cache/coding profile and CCR TTL 7200. Local measurement was on. Beacon/router/output-shaper/memory/learn were off. The synthetic test had an empty protected-tools list. This run does not prove source/numeric-content protection for all real tools.

Early runs recorded an incomplete tool notice because of the fixture MCP receipt boundary. They also recorded early exit12 because of the release state environment and a different-volume write rejection. These failed records were not deleted or changed to PASS. The earlier seven-argument test connection was cleaned by native Undo in its own verified old binary: `work/proofs/opencode-retrieval/cleanup-old/result.json` passed exit0/MCP removal/canonical endpoint/released owner. This profile is not a user connection.

Do not repeat successful gates unless their inputs change. Existing commands are available if needed:

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/runtime -p test_retrieval_bridge.py
```

Other local gates are in [WORKFLOW.md](../development/WORKFLOW.md). Native evidence requires the packaged release and real OpenCode. A unit test alone or the old test-only MCP bridge cannot reproduce this acceptance.

<a name="açık-kabul"></a>

## Open acceptance

Live provider/auth and real model inference, real main/subagent identity/separate paths, model changes, TUI/V2/Desktop, clean Windows/Windows 11 and other OS remain open separately. The selected user file and the same user's older config layers were checked. Another process's project/remote/managed/plugin layers can change the effective setting. The historical endpoint-only catalog profile was not upgraded to claim this different fixture combination. No Actions, new account/install, release or paid model call occurred. Linux and reporting work remain deferred.
