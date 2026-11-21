<a name="opencode-v2-normal-servis-ve-tam-tui"></a>

# OpenCode V2 normal service and full TUI

November 15, 2026. The test used Windows 10 22H2 x64 (19045), OpenCode 2.3.0 Windows x64 baseline, and Headroom 0.41.0. [Acceptance plan](../decisions/009-opencode-provider-routes.md#h010-h--v2-normal-servis-ve-tam-tui), [test record](opencode-v2-service.json), [review](../reviews/H010-h.md)

Preview and Apply in the real Headroom window saved two provider routes and the bundled MCP bridge in an isolated OpenCode profile. The test then started the normal service with `opencode service start`. The registration file, listener PID, and Windows Job ownership identified the same service. The CLI and TUI commands used normal OpenCode service discovery. [Pinned service resolver](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/server-connection.ts)

| Client surface | Model selection | Local result |
| --- | --- | --- |
| Normal `opencode run` | `lab/slot`, Chat Completions | Three completed local MCP calls, four model requests, an exact final response, and exit code 0 |
| Default full TUI, Windows PTY | Default `openai/slot` from the config file, Responses | Three completed local MCP calls, four model requests, final response in the terminal, and exit code 0 after `/exit` |

Windows showed an open TCP connection from the terminal to the registered service port. Both sessions used the same service database. Each session matched the expected model, original test message, three completed tools, final response, and successful result.

In each turn the sample log decreased from **14,999 bytes to 103 bytes**. Retrieval returned the original bytes. The byte reduction measures this log sample. Actual token use and cost remain unmeasured.

`opencode service stop` completed with exit code 0. The service registration cleared, the port closed, and the service Job held no processes. Remove in the Headroom UI restored the original JSONC bytes and released ownership. The GUI, client, and service closed normally. The run restored the original isolated engine settings.

The result covers local synthetic traffic through a newly started service. Immediate adoption by a running service, OpenCode Desktop, V2 subagents, live accounts, and other operating systems remained open.

The final local run is in `work/proofs/opencode-v2-service/native-fee4adc891/result.json`. The tool loop completed in the first CLI run. The test stopped at acceptance because it searched for the final response in an empty `session_part` table. The updated verifier reads the message's actual `content` field. The first run's record keeps its normal service, port, settings, and ownership cleanup evidence.

The final native run matched 22,705 source, package, and fixture inputs before and after. The catalog recorded the verified version and environment. The final local gate passed 222 Node and 462 Rust tests. It ignored 19 tests. Rustfmt and the frontend and embedded Rust builds passed. Apart from the catalog update, the final product inputs matched the previous candidate.
