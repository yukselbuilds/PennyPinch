<a name="computer-use-tanısı--20-ekim2026"></a>

# Computer Use diagnosis on 20 October 2026

On Windows 10 Pro 22H2, the tool listed Headroom and a new blank Notepad window. It could read accessibility text in AT-only mode. The first screenshot returned FrameArrived timeout. The single recovery after a fresh selection returned window capture timeout. Clicking the current accessibility item by index returned geometry unavailable. Alt+F4 closed the blank Notepad window.

The same failure occurred outside Headroom. The underlying cause in the local native helper remained unknown. Windows settings and the helper file stayed the same. The diagnosis used the existing capture and automation helper.

On 20 October, the review found the Codex issue open. User reports described the same three symptoms in different apps. Window listing and accessibility reads worked. Capture and clicks failed. Maintainer confirmation and the local cause remained open. [Codex #49680](https://github.com/openai/codex/issues/49680)

Related reports describe similar failures on Windows 10 and later releases. Community reports describe a callback or binary patch working with specific helper hashes. The change invalidates the file signature. It gives no guarantee across releases. [#49383](https://github.com/openai/codex/issues/49383) · [#51245](https://github.com/openai/codex/issues/51245)

One report describes a specific helper that waits synchronously for an asynchronous image conversion inside the FrameArrived callback. This diagnosis would need local measurements to apply here. The review read the external patch and launch instructions without applying them. An official signed fix still needed verification. [#35446](https://github.com/openai/codex/issues/35446)

At this checkpoint, H016 still had a separate product issue. The manually selected candidate disappeared after a rescan. A fresh 0-byte cmd file and real bounded environment tests passed locally. Native acceptance remained open. The planned check would trace real selection and scan with fixed metadata in a debug build only. See `docs/proofs/discovery-native-diagnosis.md`.

<a name="aynı-gün-sonraki-durum-ve-ocu-kurulumu"></a>

## Same-day follow-up and Open Computer Use installation

The earlier section records the initial diagnosis. The later local native acceptance for H016 passed. The cause of the original lost selection remained unknown. See `docs/IMPLEMENTATION_STATUS.md` and `docs/proofs/discovery-desktop.md`.

The user explicitly requested installation of Open Computer Use as a separate Codex MCP tool. The source commit was `43eb4b53de7a15b3f99ec2c9922bedeb95602266`. The Windows native Go module uses the standard library. The build used portable Go 1.26.10 and produced version `1.1.2+43eb4b53`. The `go test ./...` command, build, and stdio MCP initialization passed. The MCP server listed 9 tools. The EXE SHA256 was `ced5db2ec4509eb1ff6dbeea68eda575c3e0f04861b7a7f0311ead9a4a6838df`. [Pinned Open Computer Use source](https://github.com/iFurySt/open-computer-use/tree/43eb4b53de7a15b3f99ec2c9922bedeb95602266)

The install root was `%USERPROFILE%\.codex\tools\open-computer-use`. Skill Installer added the skill from the same commit to the user's skills directory. The setup added only the owned `open_computer_use` MCP block to the user's `config.toml`. Other TOML data and existing bytes stayed the same.

The setup enabled real mouse and keyboard input only for this MCP server. Windows policy, the official signed helper, model, reasoning, auth, and agent settings stayed the same. The server was absent from the chat's normal tool list at this checkpoint. The test used a persistent stdio MCP connection in Node REPL. Normal tool discovery required reopening Codex. The skill could be used on the next user turn.

The actual Windows 10 Pro 22H2 test captured a screenshot of a new Notepad window with PID 30272. The `app_post` action left the File menu closed. The explicitly enabled `global` mouse click opened it. The test typed sample text and cleared it with physical Ctrl+A and Backspace.

For the temporary menu target, the foreground guard blocked global Escape. A fresh observation returned to the main window. The test kept the guard and recorded the limit. The Notepad window was clear at the end, and the test confirmed its process exit.

The test captured Headroom at PID 30108. A global mouse click opened Applications. The screenshot and accessibility tree both showed the Applications heading, Codex and Claude candidates, and `Detected installations: 2`. This check covered window and input behavior. Client connection, model traffic, and renewed H016 picker and rescan acceptance had separate evidence. H040 had yet to start.

Global Alt+F4 closed Headroom. The readback returned `appNotFound` and a separate process wait confirmed exit. The test closed its windows and diagnostic MCP server. Monitoring ended with the test. Windows 11 and other tool actions still needed separate checks.

The installation, rollback, and measurement records are `%USERPROFILE%\.codex\tools\open-computer-use\README.md`, `installation.json`, and `runtime-proof.json`. Source and build fingerprints are in `43eb4b53/source-provenance.json` and `build-proof.json`. The record excludes a full config or auth backup and raw error logs.

The default mode of `disconnect.py` is dry-run. The `--apply` option removes the owned MCP addition only if it still matches. It writes atomically and preserves other setting changes.
