<a name="opencode-v2-windows-teknik-ön-kontrolü"></a>

# OpenCode V2 technical preflight on Windows

14 November 2026. **H010-e passed locally. Main H010 remained open at this checkpoint. The total remained 42/54.** [Programmatic acceptance](opencode-v2.json), [plan](../decisions/009-opencode-provider-routes.md#h010-e--v2-windows-ön-kontrolü), [review](../reviews/H010-e.md).

The official [v2.3.0 release](https://github.com/anomalyco/opencode/releases/tag/v2.3.0) Windows x64 baseline ZIP was placed only in `work/toolchains/opencode/v2.3.0`. The release API digest, SHA256SUMS and local archive hash matched. The executable was also pinned. `receipt.json` records the download/extraction time and says `executed:false` at that time.

Later real execution has separate runtime evidence. V1 installation and global PATH did not change. The sources are 34 focused files pinned to commit `a6b15d33daf6ab7d35569f6d711fdb630b9f04e7`. Their URLs/hashes are in canonical JSON.

<a name="gerçek-yol-ve-sonuç"></a>

## Native CLI route and result

On Windows 10 22H2 x64 build 19045, each protocol used new HOME, XDG, AppData, ProgramData, and temp paths. The working directory was its own global config root. Pinned ConfigDiscovery skips ancestor/project search in this case. The CLI received only an explicit allowlist environment. The caller's auth/config/server-password were not passed through.

V2 uses `providers/settings`, permission/policy and agent definitions. Update, share, warming, snapshot, compaction, tools and title agent were disabled.

For each profile, the real binary ran version/help/debug paths and `run --standalone --format json`. Help showed the private server flag. Debug paths showed that all ten paths were under the fixture. `--standalone` opens a private server with an stdin lease instead of discovering the normal background service. This path does not test whether the normal CLI/Desktop service adopts the settings. [Connection selection source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/server-connection.ts), [private server source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/standalone.ts).

| Check | Chat | Responses |
| --- | --- | --- |
| Selector → upstream model | lab/slot → headroom-opencode-tool-fixture | openai/slot → gpt-4.1 |
| Model requests | 1 | 1 |
| Model and synthetic Bearer | Preserved | Preserved |
| SSE final + terminal step | Exact final / 1 step | Exact final / 1 step |
| Four CLI commands | exit0 / stderr0 | exit0 / stderr0 |
| Owned job after each command | process0 | process0 |
| Config and environment | Same | Same |
| Engine shutdown | graceful / exit0 / port closed / force=false | graceful / exit0 / port closed / force=false |

The first request from the real CLI in run `f89faa722bae` returned `provider.no-route`. The run remains a failure in the record. The engine closed normally. Diagnosis used a private profile. V2 policy requires `provider.use` and a usable integration record for the provider. The final config denies all, then allows only the selected provider's `provider.use` and `integration.use` permissions.

Other integrations and tool permissions remain off. This test config is not written to user settings. [Policy source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/config/plugin/policy.ts).

<a name="doğrulama-ve-tekrar"></a>

## Verification and reproduction

The final record `work/proofs/opencode-v2/runs/1dbf43fb31b2/result.json` reports passed:true. Six tests cover split text and final events, missing or duplicate terminal steps, and error events. They also check incorrect routes, auth, models, streams, and prompts. They cover V2 deny-by-default config, paths outside the profile, and isolation from the caller's environment. They verify cleanup of the owned CLI after a Job assignment error.

`focused-72dd3c285b.json` passed with the same final benchmark source. The runtime's 645 inputs, case config/environment and 34 pinned sources matched. The earlier 332 product inputs, tool versions, OS, and Python matched. The team reused 222 Node and 448 Rust results, with 18 ignored. It reused the format, frontend, and embedded build results. The new test has separate evidence. CodeGraph sync exited 0/stderr0.

With the verified V2 toolchain and existing pinned engine payload ready at the project root:

```powershell
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_v2.py
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -B .\bench\opencode\v2.py
```

Each repeat uses a new UUID profile/engine. It does not overwrite the previous record. The CLI command has limits of 120 s and 4 MiB. HTTP has a 4 MiB limit. Engine startup has a 100 s limit. The owned Job supplies the required shutdown fallback. A pass requires 0 processes before the Job closes.

Token/usage values are fixed synthetic fixture counts. Real usage, savings, cost and time to first token were not measured. The closed loopback proxy and offline engine profile are config checks. They are not firewall/packet-capture evidence.

The first positive runtime record `42d4a3f93ce4` remains. Review added a kill/wait fallback and a negative test for the CLI handle that had not yet been assigned to a job when assignment failed. The source fingerprint changed, so the old PASS was not reused for the final candidate. The six tests above and two protocols were verified again on the new candidate. Original bytes of the earlier canonical acceptance JSON were also preserved in the project proof root.

<a name="desktop-ve-açık-kapsam"></a>

## Desktop and remaining verification

V2 Desktop is a separate `opencode-desktop-win-x64.exe` artifact (267,406,912 bytes). The pinned Electron builder includes the bundled `opencode-cli` resource. The main entry starts a sidecar probe. This installer was not downloaded or run. CLI precheck does not count as Desktop service/process/config adoption, auth or real GUI acceptance. [Package source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/electron-builder.config.ts).

At this checkpoint, the V2 product blocker and `not_tested` catalog status remained. The next slice covered discovery and trust, atomic Plan, Apply and Undo, and V2 `mcp.servers` retrieval. V2 MCP, CCR, subagents, and normal service and GUI use remained open. Live providers and auth, Windows 11, and other operating systems also remained open.

No account, paid call, Actions or release was used. Development-chat savings were not measured.
