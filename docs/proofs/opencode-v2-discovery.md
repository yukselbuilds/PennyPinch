<a name="opencode-v2-salt-okunur-sürüm-tanıma"></a>

# OpenCode V2 read-only version detection

14 November 2026. **H010-f passed locally. Main H010 remained open at this checkpoint. The total remained 42/54.** [Canonical JSON](opencode-v2-discovery.json), [decision](../decisions/009-opencode-provider-routes.md#h010-f--v2-sürüm-tanıma-ve-güvenlik-sınırı), [review](../reviews/H010-f.md).

Product discovery now detects the official Windows x64 baseline 2.3.0 executable verified in H010-e by the size/SHA256 pair. No new download or system install occurred. V1 1.19.3 remains in the same matcher.

The PATH, user-local, and manual file readers accept either of the two pinned sizes. The same checks apply to the hash, bounded reads, initial and final handles, and path identity. Discovery does not run `--version`. The retained file pin verifies the correct version separately. It holds the read handle to protect against normal write or replacement races.

<a name="gerçek-dosya-ve-backend-sonucu"></a>

## Real file and backend result

On Windows 10 22H2 x64 build 19045, the test read only two explicitly selected CLI artifacts inside the project. The V2 opt-in test creates a temporary user-local hard link in its own UUID fixture. Cleanup removes only that root/link. The source binary's bytes remain unchanged. The test injects a fixture selection into the same DiscoveryManager method instead of the normal file picker. It does not count as a physical window/file-picker retest.

| Acceptance | Observed result |
| --- | --- |
| Real V2 PATH/manual/user-local | All three were 2.3.0. Connection was not_verified. |
| Manual rescan | Selected file and version remained. |
| Correct version pin | Acquired. Write-open was rejected. |
| Wrong expected version | Pin was rejected. |
| Real V1 file | PATH discovery preserved 1.19.3. |
| Backend V2 preview | Known version. Blocked, after_headroom=false. |
| Backend Apply | config_conflict. Fixture JSONC unchanged. No ownership root. |
| Account capability | Off for 2.3.0. |

An incorrect size or digest remains unknown. A size from one binary with a hash from the other also remains unknown. The same applies to a matching size with a different digest. The plan test verifies that a V1 writer cannot open JSON in either V1 or V2 format for version 2.3.0. The account parser/login gate is also off.

The UI select_client_file → DiscoveryManager and preview/apply commands → PlanManager path was reviewed. The backend test uses a real file/real transaction blocker. It does not present a mock model or successful connection.

The catalog only updates the old standalone CLI Chat/Responses precheck to local_fixture_partial. V2 config writing/GUI/subagent capabilities remain not_tested. Auth/platform/surface/version matching scope remains unchanged. This record does not show automatic V2 product support or a signed-in account.

<a name="kanıt-ve-tekrar"></a>

## Evidence and reproduction

- Final full gate `work/proofs/opencode-discovery/checks-b3151e09f2/final.json`: **222 Node, 450 Rust PASS. 19 ignored. fmt/frontend/embedded build PASS.**
- Separate real file test `checks-296b2e1367/real.json`: **1 PASS**. This is the real invocation of the ignored test in the full gate. Both records link the same 341 source/test/config/CLI/dependency/tool/OS inputs before and after.
- H010-e's 645 protocol inputs did not change. The successful local model turn was not repeated or expanded. Debug exe and dist hashes are also pinned in the canonical record.
- 59 Rust/5 frontend focused PASS preceded the DOS-path fix in the final ignored test fixture. The final candidate's normal tests are accepted through the full gate. The real file test has its own current record. The old focused record is not presented as the current file fingerprint.
- CodeGraph synced the final four Rust files. A single-result query found the new shared digest matcher. Graph output is not runtime PASS evidence.

The first compile had a test that accessed a private sibling function. The test then used the real public discovery/pin path without expanding production API visibility. In the first real run, V2/plan/pin steps passed. The V1 fixture's canonical `\\?\` PATH input then failed the namespace guard.

Only the test changed to a normal DOS project path. The security guard did not change. Both failed records and prior focused PASS hashes remain in the canonical JSON.

With the pinned project toolchain and earlier local preparation ready, run from the repository root:

```powershell
$env:HEADROOM_TEST_OPENCODE_V2_CLI = 'D:\Projeler\Headroom\work\toolchains\opencode\v2.3.0\runtime\opencode.exe'
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo-home'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml opencode_v2_project_binary_is_discovered_retained_and_pinned_without_execution -- --ignored --nocapture --test-threads=1
```

`verify.py real/final` in the canonical record runs the same commands with only child-process environment variables in a new proof directory. No real user settings/auth, installed client or active chat is changed. No client/model/account is opened, and no Actions, paid API or release is used.

At this checkpoint, V2 writing, receipts, and MCP retrieval remained open. Normal CLI and Desktop service adoption and live auth and subagent acceptance also remained open. Windows 11, macOS, and Linux had separate open acceptance. Real token/billing or development-chat savings were not measured.
