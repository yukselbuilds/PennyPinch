# H039 — Local release candidate validation

10 November 2026. The user chose to continue without a paid Windows 11 environment. H039 started while the external H041 and H042 criteria remained open. The Windows 11 x64 result is `not_tested`. The scope of this slice was local validation. Public release, the production key and CI were tracked separately.

## Scope and ownership

The lead agent owns the source, package, process and measurement checks, the real native flow and acceptance. The existing single Luna xhigh worker reviews guide and README claims in scope and drafts `USER_GUIDE.md`. Keep model and provider settings intact. Review the final change and related paths within this scope.

The final candidate is a real current-user NSIS build from current source and the pinned payload, using an isolated QA identifier. Discovery debug tracing is disabled. It uses default release settings and NSIS compression without a production source or key. Installation files, fixtures and evidence live in `work/rc39/`. Manage only QA-owned state, registry entries and the installer. Preserve the settings of other apps and accounts.

## Acceptance scenarios

1. **Normal.** Install the final package into a separate directory. In **Applications → OpenCode → Add from file**, select the empty, unexecuted `opencode.cmd` fixture. **Selected file** remains after three consecutive scans and after leaving and returning to the screen. This covers discovery during the current app session. Restart persistence requires an explicit save.
2. **Failure or cancellation.** Canceling the picker preserves the previous row. Review the picker and scan races, stale-result protection, settings and measurement behavior, and user-facing claims. Fix any concrete critical defect in this task with a focused regression check. Keep the acceptance conditions.
3. **Boundary and delivery.** Exercise the manual scenario in the real native package with debug tracing disabled. Bind source, test, configuration, tool, package and installed-file hashes in the same acceptance step. Complete the actual local quality gate on the final candidate. Reuse valid evidence only when the corresponding fingerprints and tools match. Clean up QA-owned installation and processes afterward.

## Verification paths

- **Local gate.** Use `test:app`, the frontend build, formatting, offline Rust tests and the custom-protocol build from `docs/development/WORKFLOW.md`. Run the gate on the single final candidate after review and fixes. Reuse the 179 frontend test and build results if frontend source and tools match. Adding the Rust verifier test changed the Rust inputs and required a fresh gate.
- **Package.** Use the pinned Python with `scripts/package-windows.py`, Tauri CLI 2.12.3 and the existing NSIS cache. Pass the new QA config without modifying source. Verify engine files against the full inventory.
- **Native UI.** Use Computer Use for the real window, file picker, repeat scans and navigation. Record native acceptance from the actual app. Mock, CDP fixture and debug-trace results cover their own scenarios.
- **Support matrix.** Record Windows 10 local acceptance, Windows 11 `not_tested`, open macOS and Linux port work, and open live API-key and production release checks independently. Report development token savings only when measured.

## Local acceptance passed

10 November, Windows 10 x64 build 19045. [Combined evidence](../proofs/release-candidate.json) · [Review](../reviews/H039.md) · [User guide](../USER_GUIDE.md)

- A real PE check found the console subsystem in the release executable. The regression first failed. After setting the Windows GUI attribute, subsystem=2 passed. The packaging command uses the same guard. The interrupted first candidate and test-driver failure were excluded from acceptance.
- The NSIS/LZMA package used default LTO enabled, 1 codegen unit, stripping and panic abort, with an isolated QA identifier. Verification checked 22,333 resource files. After accounting for Tauri's single NSIS bundle-type marker, the executable matched all expected bytes. The verifier accepts that exact marker difference.
- The user confirmed the prefilled path by clicking **Open**. The lead agent completed three scans, visited **Activity** and returned, and read the retained manual source from the native accessibility tree. No error or stale banner appeared. Canceling exit preserved the row. Confirming exit then returned process code 0. Discovery tracing was off. Native file-picker cancellation was outside the repeat run, with cancellation and race coverage in the source tests.
- The final gate passed 18 Python tests, 413 Rust tests, formatting and the custom-protocol build. It skipped 15 opt-in Rust tests. The 179 frontend tests and build were reused with 112 matching inputs and tool versions. The check recorded 232 matching source inputs and ran the product gate once.
- The uninstaller returned 0. The QA registry entry, executable, state and sidecar lock were removed. The interrupted first installation was retained in the project-local failed-artifact area. The run applied zero client routes, made zero provider requests and started no CI or public release.

| Environment or scope | Result |
| --- | --- |
| Release, NSIS and native flow on this Windows 10 x64 host | Passed. Clean OS acceptance is separate. |
| Windows 11 x64 | `not_tested`. The user deferred a paid environment. |
| macOS and Linux | Port and native acceptance were open at this checkpoint. |
| Live Codex API-key route | Open in H041. |
| Production update feed, key and HTTPS delivery | Open in H042. |
| Clean Windows and a host without WebView2 | Open in H037. |

Some background captures showed the foreground Codex window and were excluded. The native results use the target Headroom accessibility tree, real installation and the exit record of the owned process. This is local H039 acceptance. Public release, clean installation and other platforms have separate evidence.
