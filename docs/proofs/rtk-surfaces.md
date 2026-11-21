<a name="rtk-oss-110-istemci-hook-yüzeyleri"></a>

# RTK-OSS 1.1.0: Client hook surfaces

The table uses sources pinned in `work/proofs/rtk/toolchain/1.1.0/receipt.json` at commit `753a3b0e939f2e4490fc24e3a1984010f40a103c`. Local copies of hook READMEs, plugins, and Rust files matched their recorded SHA-256 values. The lower section gives the scope of fixtures run on Windows.

| Client / prerequisite | Hook input and scope | Rewrite and permission behavior |
|---|---|---|
| **Codex CLI**. The README does not state a minimum Codex version. The hook can be added to project `.codex/hooks.json` or `$CODEX_HOME/hooks.json` with `rtk init --codex`. | `PreToolUse` event, `Bash`/`bash` tool, and `tool_input.command`. Permission modes are only `default`, `acceptEdits`, `plan`, `dontAsk`, and `bypassPermissions`. Non-Bash tools, unknown modes, and shell forms that cannot be resolved safely stay unchanged. | In `hookSpecificOutput.updatedInput`, the handler keeps existing `tool_input` and changes only `command`. The RTK reply includes `permissionDecision: "allow"`. Codex repeats its normal approval and sandbox checks for the modified command. RTK does not interpret other agents' permission files. |
| **Claude Code**. The fixed shell hook needs Bash, `jq`, and RTK `0.23.0` or later. The separate `rtk hook claude` path handles JSON natively. | Both paths read `tool_input.command`. The shell script calls `rtk rewrite`. The native path also reads `cwd` when it is present in the input. | An allowed rewrite returns `updatedInput` and `permissionDecision: "allow"`. An operation that needs approval omits `permissionDecision`. If no match occurs or a rule rejects it, the original command passes through. The shell script also gives no rewrite when a dependency is missing. |
| **OpenCode**. Version 1.x needs at least `1.18.29`. The README also lists the version 2.0 plugin path. RTK must be on PATH, and `rtk hook opencode --help` must pass the capability check. | The TypeScript plugin handles only the `bash` or `shell` tools and their `command` text. The version 2.x `execute.before` input may contain `event.agent`. Version 1.x `tool.execute.before` has no agent field. Windows PATH search uses `PATHEXT`. | The plugin passes the command with `execFile` argv to `rtk hook opencode [--agent NAME] COMMAND`. The Rust side compares permission results before and after the rewrite under the OpenCode rules it can interpret. It returns an empty response if the result changes. It writes a valid rewrite to version 2 `event.input.command` or version 1 `output.args.command`. An error, timeout, or invalid response preserves the original command. |

<a name="windows-10-fixture-sonuçları"></a>

## Windows 10 fixture results

| Surface | Observed result | Evidence scope |
| --- | --- | --- |
| PowerShell 5.1 | In a separate profile-free shell, `git status` returned 2,279 bytes. Explicit `rtk git status` returned 2,151 bytes. `rtk bypass` preserved stdout, stderr, and exit code exactly. | Explicit command call. The test did not check automatic client capture. |
| Codex | For a synthetic `Bash` input, RTK's native hook returned `rtk git status`. It returned an empty response for `shell_command`, `Read`, `Grep`, and an unknown permission mode. | RTK input handling. The JSON reply was not sent to a client, and the reply did not run a command. |
| Claude Code | The native RTK hook received a `Bash` input with a temporary Claude config directory and returned `rtk git status`. The fixture recorded no `permissionDecision` value. An input without a `command` field for `Read` returned an empty response. | Native RTK hook fixture. The Bash/jq script and a real client session were not tested. |
| OpenCode | The real RTK plugin function gave no rewrite for `read` and `grep`. With `RTK_DISABLED=1`, `bash` and `shell` stayed unchanged. | Early-pass conditions in Node. The real OpenCode loader and execution of rewritten commands were not tested. |

The before/after SHA-256 values for 12 inputs matched. These four fixture runs did not write client settings. Windows 11, macOS, Linux, Codex Desktop, real OpenCode/Claude hook setup, and subagent scope remain `not_tested`. [Machine-readable record](rtk.json)

<a name="kapsam-ve-sınırlar"></a>

## Scope and limits

- These three integrations rewrite shell commands before they run. They are not a proxy surface that observes API requests to model providers. These sources do not prove all model traffic, token savings, or every model/route.
- Codex and Claude records cover only `Bash`/shell commands. These sources do not show that they cover calls to local `Read`, `Grep`, MCP, or other native tools. The OpenCode plugin also accepts only the names `bash`/`shell`.
- The OpenCode plugin README states Windows Node `execFile`, Windows PATH/PATHEXT, and OpenCode Desktop/Electron compatibility. This is not a test of the Windows runtime inside Headroom or real Desktop use.

  The Codex hook record uses a Bash matcher. These sources do not show that it captures PowerShell commands. The Claude hook is a Bash script. Windows/PowerShell execution is not documented. Therefore, these surfaces do not establish Windows PowerShell coverage.
- The Codex README states that the hook does not reuse other agents' permission files. OpenCode 2.x passes `agent` only when it evaluates agent permission rules. Version 1.x has no such field. These details do not prove capture of all main-agent/subagent calls. Subagent coverage for Claude is also not shown here.
- Hook installation or command matching does not prove real integration, routing, account verification, or provider measurement. This source review did not test RTK proxy or coexistence with other tools.

<a name="kaynaklar-ve-fingerprint"></a>

## Sources and fingerprints

Line numbers refer to `work/proofs/rtk/toolchain/1.1.0/source/`.

- Codex: `hooks/codex/README.md:7-19`, `src/hooks/hook_cmd.rs:681-700,983-1025,1027-1066`, CLI hook command `src/lib.rs:1274-1303`.
- Claude: `hooks/claude/README.md:7-10`, `hooks/claude/rtk-rewrite.sh:1-4,16-27,46-60,62-105`, JSON handling/reply `src/hooks/hook_cmd.rs:703-777,824-878`.
- OpenCode: `hooks/opencode/README.md:7-20`, `hooks/opencode/rtk.ts:137-172,177-225`, permission comparison/reply `src/hooks/hook_cmd.rs:1278-1335`, CLI command `src/lib.rs:1294-1303`.
- Commit in the receipt: `753a3b0e939f2e4490fc24e3a1984010f40a103c`. SHA-256 values for the seven inspected files matched the receipt: Codex README `c1f3d218…ff0489ca`, Claude README `424e28ef…ab4e82c`, Claude script `d4cd1e04…893267d`, OpenCode README `a289a50b…fb1c8b`, OpenCode plugin `989e9231…d4898aa4`, Rust hook `ce9965fa…2308ad7`, and CLI definitions `c737ed90…f4e372`.
