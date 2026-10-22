# H016-a: read-only client discovery backend

Date: 2026-10-20. Scope: backend discovery DTO and commands only. This is not a
client compatibility, authentication, or connection result.

`get_discovery_snapshot` returns `not_scanned` until an explicit
`scan_discovery` call. Scanning runs on Tauri's blocking pool and reads only
bounded local metadata. It checks PATH entries (up to 64, with `.exe`, `.cmd`,
and `.bat` names), the two known user-local npm roots for exact package names
and matching `bin` entries, direct `.local/bin` candidates, bounded direct
Scoop version folders, and shallow Windows app-package entries for Codex
Desktop. Package manifests are capped at 16 KiB; Windows version resources at
128 KiB. A PATH wrapper remains a separate version-unknown candidate; its
directory alone cannot attach an npm package version to it. It never executes a
discovered program or a version command, and does not inspect client
configuration, credentials, or conversation data.

PATH and fixed candidates are accepted only on local fixed volumes. UNC/device
and removable paths are skipped before candidate metadata is opened;
reparse-point components are rejected. The scan caps candidates at 512 and
reports fixed partial-scope warnings. Versions come only from a matching,
bounded npm package manifest or verified Codex Desktop file-version metadata;
otherwise they remain `null`. Installation IDs are opaque UUIDs, paths stay in
the backend-owned lookup map, and each result is marked `not_verified` for
connection status. A missing installation is not represented as unsupported.

Six focused fixture tests passed. They cover initial/missing state, multiple
PATH installations and case-insensitive deduplication, a Unicode path, exact
package-name/bin matching, malformed and oversized metadata, a same-name PATH
wrapper kept separate from the npm package version, metadata/PATH/candidate
limits, pre-metadata UNC skipping, read-only fixture bytes, and the fixed
validation error DTO. Test fixtures use fresh UUID directories beneath
`work/proofs/discovery/runs`; existing ancestors are checked before creating
each directory. Cleanup checks the original UUID root for symlink/reparse
status, verifies its canonical target is exactly that new UUID directory, and
confirms every descendant remains beneath it before removing only that
fixture.

Commands, from the project root with project-local Cargo directories:

```powershell
$env:CARGO_HOME='work/caches/cargo'
$env:CARGO_TARGET_DIR='work/caches/cargo-target'
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --bin headroom-desktop integrations::discovery_tests
cargo check --manifest-path src-tauri/Cargo.toml
```

All three commands passed; the focused test command reported 6 passed, 0
failed. Environment: Microsoft Windows 10 Pro 22H2 x64, build 19045; rustc
1.92.0, Cargo 1.92.0; `windows-sys` 0.61.2. No client executable was run and
the real-user discovery command was not invoked. Windows 11, a clean machine,
Codex Desktop Appx visibility, and real UI invocation remain untested.

Source/config SHA-256 fingerprints for this candidate:

| File | SHA-256 |
| --- | --- |
| `src-tauri/src/integrations/discovery.rs` | `171d176ecc69b533a310eff4412ad85320ca5a0f9b34ea89366e29c1ad53f03c` |
| `src-tauri/src/integrations/discovery_tests.rs` | `25c8ede29b0ec40d1d3d76b69e51757678230920e4bfddc974b9d028b5c8f1cd` |
| `src-tauri/src/integrations/mod.rs` | `1f5772342aed3ce2b3080ae41b8b8a854ce3bac7d0a010728b49be34798de617` |
| `src-tauri/src/main.rs` | `c3f4cc4045d33c95b38513044c485bbe06199e65fbead305145a47a34bbd9906` |
| `src-tauri/Cargo.toml` | `4f3bd8c9a840103a16de5d69468f6f7f4718fba1e466a7e7c0420680798c324f` |
| `src-tauri/Cargo.lock` | `61f5fd42ecdff941fa80f0ade6a16c0611c403106b2f3448a95e68a93ae90319` |

Installation locations follow the bounded official CLI guidance recorded in
[`client-inventory.md`](client-inventory.md): [Codex CLI](https://developers.openai.com/codex/cli),
[Claude Code](https://code.claude.com/docs/en/getting-started),
and [OpenCode 1](https://docs.opencode.ai/docs/). These links describe known
install surfaces; they do not imply exhaustive discovery coverage.
