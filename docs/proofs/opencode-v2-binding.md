<a name="opencode-v2-kullanıcı-bağlantısı"></a>

# OpenCode V2 user connection

November 15, 2026. Headroom connected existing provider endpoints for the pinned OpenCode 2.3.0 CLI on Windows 10 22H2 x64 through Preview and Apply. The plan preserved the current model, effort, authentication, and permission settings. The test used the official Windows x64 baseline package. [Decision](../decisions/009-opencode-provider-routes.md#h010-g--v2-kullanıcı-bağlantısı-ve-şemalı-geri-alma)

| Scenario | Observation |
| --- | --- |
| Preview and Apply in the real Headroom window | The app wrote two provider endpoints and the MCP bridge to the same JSONC file. The shared ownership record stored the V2 schema. |
| Chat Completions and Responses | Each protocol made four local model requests and completed three tool calls. The model, synthetic API key, and final response matched. The CLI exited with code 0. |
| Retrieve a compressed log | The sample block decreased from 14,999 bytes to 103 bytes in both protocols. `headroom_retrieve` verified the original bytes and hash. |
| Remove after reopening Headroom | An edit to the MCP entry caused a conflict. The app preserved the settings file and ownership record. After the test removed the edit, Remove restored the original JSONC bytes and released ownership. |
| Shutdown | Both Headroom windows closed normally. The owned processes exited and their ports closed. |

The real OpenCode binary started its own `--standalone` service and read the V2 settings file that Headroom saved. The local upstream returned fixed synthetic data. The byte comparison measures the sample log block. Live token savings, billing savings, and development-chat savings remain unmeasured.

Window and model evidence is in `work/proofs/opencode-v2-binding/native-d2a0ee959b/result.json`. The test window used a separate application identity and profile. The test accessed the controls only through that window's WebView. Full OpenCode TUI, Desktop, shared service, subagents, live accounts, and other operating systems remained open in H010 at this checkpoint.

The final product gate passed 222 Node and 462 Rust tests. The default run skipped 19 opt-in Rust tests. Format checks, frontend build, and embedded app build passed. A separate V2 test used a real file and verified that the app refused to write when the bridge was absent. The native run matched 22,685 inputs, and the final gate matched 344.

After the native run, the compatibility catalog added Windows 10 CLI evidence. The connection source, tests, and WebView files retained matching hashes. The final local build included the new catalog record. [JSON evidence](opencode-v2-binding.json)

The first window did not open with a restricted GUI environment. Later runs opened the window with the Windows environment inputs and an isolated settings file on the same disk. The test helpers checked the identity and hash of the file at the package path. The Responses verifier used the observed V2 output to correct its assumptions about upstream streaming and tool order.

Separate SQLite records in the test profile verified `headroom_retrieve` completion for both protocols. The JSON evidence references the failed runs and diagnosis.
