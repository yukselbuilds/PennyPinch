<a name="h010-d--windows-10-tam-opencode-tui-kabulü"></a>

# H010-d — Full OpenCode TUI acceptance on Windows 10

November 14, 2026. **The local slice passed. Main H010 remained open and the total remained 42/54.** The test used Windows 10 22H2 x64 build 19045, the real OpenCode 1.19.3 binary, and Headroom 0.41.0. [Acceptance record](opencode-tui.json), [plan](../decisions/009-opencode-provider-routes.md#h010-d--tam-tui-kabulü), [review](../reviews/H010-d.md)

<a name="gerçek-terminal-ve-ürün-yolu"></a>

## Native terminal and product route

The test reused the H010-b3 release and package manifest with matching hashes. It used the native config and metadata helpers from H010-c. In the native UI, Start, OpenCode Preview, and Apply wrote two providers and the product retrieval bridge to the fixture's user JSONC. Each terminal read the normally saved file without inline config or an endpoint override. The run reused the standard product build and package.

The default full TUI command ran in a real Windows PTY. Its stdin, stdout, and stderr were TTYs. The test used the full UI and kept `run --format json`, `--mini`, and direct test calls to the provider outside this path. It sent `/models`, the search text, Enter, the user message, and Enter to the terminal. The real UI displayed the final response. The test exited with `/exit` and Enter.

A separate metadata CLI read only the session list and export of the isolated profile. The report excluded the transcript and raw terminal output.

| Turn | Explicit TUI model selection | Observation |
| --- | --- | --- |
| Same model | `Headroom base fixture` → `lab/slot` | The real subagent inherited the parent model. Tool use, CCR, the witness response, and final responses completed. |
| Change within the same chat | `/models` → `Headroom switch fixture` → `lab/other` | The parent session ID remained the same. A different subagent started and inherited the new model. The connection stayed in place. |
| Two protocols | `Headroom Responses fixture` → `openai/slot`. Subagent `lab/child`. | The Responses parent and Chat subagent used two saved routes with distinct native identities. |

The test observed two parents and three different foreground subagents, with at most one subagent at a time. Task metadata matched the exported subagent parentID, agent, provider, and model fields. Across 27 upstream messages, six log blocks for the two roles decreased from 14,999 bytes to 103 bytes each. Retrieval returned the same original bytes and hash.

Chat used the product MCP retrieval bridge. The Responses parent used the engine's server-side continuation. Call ID, model, and synthetic auth were preserved. Each parent and subagent tool call completed, and the final responses matched.

The three real terminal processes exited with code 0. Source, config, and owner fingerprints matched, and forced=false. Each process belonged to its own Windows Job. The Job prevents orphan processes if the helper exits unexpectedly. Separate measurements of graceful shutdown for every child remain outside this evidence.

The GUI closed normally with confirmed exit code 0. The same exclusive native path ran `--headroom-uninstall keep` to remove the connection. It restored the original JSONC byte for byte, released ownership, closed the listener, and kept state. The run also restored the exact initial record of the small fixture settings with validated content.

Native accessibility actions ran in the background. The test sent no physical mouse or keyboard input to the user's foreground app.

<a name="model-seçimi-ve-önceki-denemeler"></a>

## Model selection and earlier runs

In the negative resume test, the TUI kept the previous model despite the `--model lab/other` hint. The run failed acceptance. Its evidence did not establish correct model selection. The accepted run covers explicit selection from the model menu.

Pinned local state checks the agent and model store before the CLI fallback. The model dialog searches by title and category. The fixture used distinct model names for this reason. The final export verified the actual selection in the running TUI. [Model state source](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/tui/src/context/local.tsx), [model menu source](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/tui/src/component/dialog-model.tsx)

The first startup-only probe recorded CLI exit code 0 and a TTY. Its outer wrapper exited with code 1. The probe ran no model turn and gave only startup evidence. Run `5acf73fd324c` closed normally before model traffic while process Job protection was incomplete.

Run `e1617ea4930e` passed the same-model turn. The resume model caused the switch to fail. The terminal ended at its 180 s limit.

The controller stopped before normal GUI and settings cleanup could be observed. The run remained incomplete. Later checks found no process or listener. Native Undo restored the exact client settings. The previous fixture settings bytes existed only in memory and were lost. The recovery reset only QA state to closed loopback port 9. [JSON record with partial runs, recovery, and hashes](opencode-tui.json)

The final candidate adds a durable copy of its initial test settings. It writes bounded fixture JSON only inside the app-owned proof root. The JSON must contain a single loopback setting. The test creates no backup of real user settings or auth. A negative test confirms that invalid content or a remote URL creates no record.

<a name="doğrulama-ve-tekrar"></a>

## Verification and reproduction

The final native record `work/proofs/opencode-tui/runs/a13dae9bba7a/result.json` reports passed:true. It passed 19 focused checks. These included five TUI export, permission, and fixture checks, ten subagent and fixture backup checks, and four existing OpenCode helper checks. `focused-selection.json` and `focused-selection-extra.json` record matching source and tool inputs before and after. All 363 native inputs matched.

The final record binds source, CLI, runtime, binary, settings, config, and terminal fingerprints. Two manifests pin eight TUI source files.

The same 332 product inputs and tools matched. The review reused 222 Node and 448 Rust test results, with 18 ignored, and the formatting, frontend, and embedded builds. The changed benchmark and new helper had separate coverage. Focused checks and the real TUI run verified them. Reuse a successful check while its source inputs match.

Prepare the H010-b3 QA release and package and the pinned Python and OpenCode tools. The same QA state must have no active connection. Run the first command from the repository root to start the controller.

In the native UI, choose Start, OpenCode Preview, and Apply. Create the `run-all` file in the printed run directory.

In a separate native terminal, run the second command with `same`, then `switch`, then `mixed` in order. For each turn, select the visible model from the table. Enter `HEADROOM_PARENT_<tur>` and press Enter. After the final response, enter `/exit`.

Start the switch turn after the controller writes a valid `same.json`. Close the GUI with normal confirmation after the final turn. The controller performs native Undo.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/tui.py --run
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/tui.py --case same
```

Each terminal has a 180 s limit. Metadata commands have limits of 30 s and 4 MiB. The GUI run has a 900 s limit. The fixture alone disables the title agent. It disables shell, file reads and writes, network, nested and background agents, and auto and yolo. Permissions allow only the fixed MCP, data, and retrieval operations and the two named test subagents. The result covers only the tested accounts, model features, and effective config layers.

**Scope.** The run used the real CLI, subagents, and TUI with a local synthetic model server. The byte reduction measures log payload size. Total tokens, cache, billing, and development-chat savings remain unmeasured here. Live providers and auth, real model quality and TTFT, V2, Desktop, and other operating systems remain open. The test ran locally without account sign-in, Actions, a paid call, or a release. Linux and reporting work remained deferred.
