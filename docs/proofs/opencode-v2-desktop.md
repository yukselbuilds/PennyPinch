<a name="opencode-desktop-bağlantısı"></a>

# OpenCode Desktop connection

November 15, 2026. The test used Windows 10 22H2 x64 (19045), OpenCode Desktop 2.3.0, and Headroom 0.41.0. [Acceptance plan](../decisions/009-opencode-provider-routes.md#h010-j--opencode-desktop-paketli-servis), [test record](opencode-v2-desktop.json), [review](../reviews/H010-j.md)

The official Windows NSIS package's release digest, SHA256SUMS, and local SHA-256 matched. The test extracted its Desktop payload inside the project. It applied the installer's AppContainer read and execute permissions to the same payload. The test ran the Electron app. NSIS installation and shortcut handling await separate tests. [Pinned package source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/electron-builder.config.ts)

Preview and Apply in Headroom connected the shared user JSONC through the detected 2.3.0 CLI. Desktop copied its bundled CLI into a separate app profile and started the normal service. The copied binary's hash matched the package. The test verified the service registration, PID, Windows Job, and loopback listener.

The test used separate paths for home, Documents, config, data, and cache within the profile. The state path that the service selected at startup on Windows also belonged to this profile. [Bundled CLI source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/src/main/service/desktop-cli.ts)

The real window showed `openai/slot`, Responses, as the selected model. The test entered the message in the GUI input field and used Send. The fixed local provider accepted only the synthetic key supplied to the packaged service. The run matched four model requests, three completed tool calls, and the visible final response. A separate SQLite check verified the single root session's model, message, tool order, and successful result.

The sample log decreased from **14,999 bytes to 103 bytes**, a **99.31%** reduction for this block. CCR returned the original bytes and hash. Actual token and billing savings remain unmeasured in this synthetic run.

The OS handle for the Electron main process confirmed normal quit with exit code 0. The bundled service stopped independently through its normal stop path. The service registration cleared, the port closed, and the test processes exited. Headroom Remove restored the original JSONC byte for byte. The test restored the original engine settings. It verified that the test executable owned the temporary `opencode:` protocol entry before it removed that entry.

The final run is in `work/proofs/opencode-v2-service/native-desktop-9600eec937/result.json`. The failed preparation runs kept their own records. The test setup changed for the Windows home path and the custom Electron load event.

One run completed the Desktop turn and timed out during test harness shutdown. The final check verifies normal app exit independently of the service that continues in the background. The cause of an earlier Apply failure remains unknown. The same Apply path passed on the final candidate. The review checked undo and cleanup records for all previous runs.

This covers Desktop with the shared config that Headroom connects through its CLI setup. Automatic discovery and connection on a Desktop-only system, Desktop subagents, and immediate settings adoption by an existing service await separate verification. Live accounts and other operating systems also await verification.

The final native run matched 22,818 source, package, and fixture inputs before and after. The single local gate after the catalog update matched 344 inputs and passed 222 Node and 462 Rust tests. It ignored 19 tests. Rustfmt, the frontend build, and the Tauri embedded build passed.
