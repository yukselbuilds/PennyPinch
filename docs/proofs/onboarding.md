<a name="h022--ilk-açılış-ve-hatırlanan-bağlantılar"></a>

# H022 — First launch and remembered connections

27 October 2026. **Accepted.** [Task](../../TASKS.md), [lead review](../reviews/H022.md). Scope is the embedded debug product on Windows 10 22H2 x64. It is not installer, live-provider or other operating system acceptance.

| Scenario | Observed result | Evidence |
| --- | --- | --- |
| Launch → read-only scan → Connect access from Overview | Engine Stopped, Claude found. No choice file, project `.claude` or user config was created. Applications opened with details collapsed. | Native `initial-pass.json` |
| First picker and cancellation | Started in isolated home. Cancellation did not change scope/preferences. A Windows notice caused by a missing Desktop folder in the isolated profile was fixed in the test fixture. | `picker-cancel-pass.json` |
| Unicode project selection | Showed the correct `settings.local.json` target and saved only project metadata. Single-candidate preselection did not write an installation preference. No `.claude` was created. | `project-selected-pass.json` |
| One explicit Connect | The engine changed from Stopped to Ready. Fresh Preview/Apply created project `.claude/settings.local.json` and a scoped active ownership record. The app remembered the installation after the explicit action. No user config was created. | `connect-pass.json`. UI → manager → runtime/transaction source path |
| Normal close/reopen | A new real process restored the same Unicode project scope and Claude installation. Config bytes matched. The engine was Stopped. No paid verification or automatic Start occurred. | `reopen-pass.json`, `closed-1.json` |
| Remembered picker and cancellation | Started in the same Unicode project. Cancellation preserved the selection bytes. | `remembered-picker-cancel-pass.json` |
| Undo in the wrong project | The UI showed a config conflict. Original config and ownership bytes did not change. No `.claude` was created in the wrong project. | `wrong-project-undo-pass.json` |
| Undo in the correct project | Undo removed only the owned `ANTHROPIC_BASE_URL`. It preserved the synthetic user field and the note file. Ownership became Released. | `undo-pass.json` |
| New client → rescan | The unrun empty `opencode.cmd` manual candidate remained after scanning. The new client did not connect automatically. OpenCode selection and client config were not written. | `new-client-rescan-pass.json`. This does not claim restart persistence for manual discovery. |
| Final close/cleanup | Three GUIs exited normally with exit0. The preference hash was restored. The team removed selection/settings fixtures that were absent at start. All ownership became Released. Owned processes/listeners were 0. The two processes of a different installed Headroom were not managed. | `closed-{1,2,3}.json`, `cleanup.json`, `metadata-restore-audit.json`, `process-cleanup-audit.json` |
| Status and simple UI | Discovery, config record and traffic verification were separate. Connection remained Not verified beside a Ready engine. The normal screen showed a short explanation and primary actions. Technical details stayed in collapsed Advanced sections. | Real OCU image/AX observation. C1/C2 source reviews. |
| Final full local gate | 97 Node PASS. 273 Rust PASS/3 ignored. fmt, frontend and custom-protocol build PASS. The same 77 sources, 3 dist files and binary hashes matched after QA. | `frontend-quality.json`, `final-quality.json`, `acceptance.json` |

Native files are under `work/proofs/onboarding/ui-runs/c5c8cfa4-3ad7-409d-8bc0-725a1d0cec2e/`. Combined acceptance and hashes are in `work/proofs/onboarding/acceptance.json`. No raw error/provider content was retained.

Tools: Node 25.2.1, pnpm 11.25.0, Cargo/rustc 1.92.0. The run used an offline Cargo cache and one Rust test thread. Build warnings: 6. Errors: 0. The product gate ran once on the final candidate. It was not repeated for document closure.

A1/A3/B1/B2/C1/C2 intermediate records remain under the same proof root with separate fingerprints. A1 covers passive Preview with a missing parent. A2 is a historical primitive before removal of the unused directory-delete API. A3 covers real TxF Apply/abort/Drop/Undo. B1 covers boundary/reparse fixes. B2 connects discovery/context/IPC/UI for choices.

The final gate includes the final sources from these intermediate results. CodeGraph sync/narrow query is in `codegraph-final.json`. It is not runtime or savings evidence.

Undo preserves the ownership-of-fields contract. It removes Headroom routing and preserves user additions. The empty settings file and `.claude` may remain. A transaction rolls back a new path on abort or a pre-commit failure. Native SetFolder, boundary/reparse/identity, choice/store errors and conflict paths were reviewed in source and focused tests.

Runtime error text explains the base package, disk/write permission, Refresh status and Start path. Real download/resume/repair belong to H037/H042. The workspace/settings.json used to trigger an error was not input to this recovery verification. The engine became Ready, and the file was restored. **This probe did not count as recovery PASS** (`recovery-probe-not-counted.json`). Disk-full and interrupted download were not tested.

This flow did not start account sign-in, a CLI/model request, paid verification or a model/effort/agent change. The local sink observed 0 POST requests. Network isolation and provider traffic require separate evidence. No workspace base-url override was used.

Live auth/model/subagent and Win11/macOS/Linux verification were not performed. The scope text describes supported models and main agents and subagents on the same route. Adoption by an existing session remains unverified. Development token savings were not measured. Additional H019 CLI errors remain open in H008.
