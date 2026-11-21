<a name="opencode-v2-alt-ajan-ve-model-rotası"></a>

# OpenCode V2 subagents and model routing

November 15, 2026. Local verification used Windows 10 22H2 x64, OpenCode 2.3.0 Windows x64 baseline, and Headroom 0.41.0. [Acceptance plan](../decisions/009-opencode-provider-routes.md#h010-i--v2-alt-ajan-ve-model-rotası), [test record](opencode-v2-subagents.json), [review](../reviews/H010-i.md)

Headroom Preview and Apply wrote two provider routes and the bundled MCP bridge to a separate V2 profile. The test started a normal OpenCode service. Both `opencode run` turns used it. The provider accepted only the synthetic key supplied to the service. The PID, loopback listener, and service registration matched throughout both turns.

| Turn | Main agent | Actual subagent | Result |
| --- | --- | --- | --- |
| Model inheritance | `lab/slot`, Chat Completions | `lab/slot`, Chat Completions | The subagent inherited the main model without a model argument. |
| Explicit model selection | `openai/slot`, Responses | `lab/child`, Chat Completions | The run kept the subagent model and protocol selected through the `subagent` tool. |

In each turn, the real CLI `subagent` tool record matched a distinct subagent session and its `parent_id` relation in the database. The tool first reported the background start. The main agent received a completion notice with the same identity and the subagent's final text. The test verified six model requests per main agent and four per subagent. It also verified the saved model and agent choices, three MCP calls per role, and final responses. [Pinned tool source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/tool/plugin/subagent.ts), [completion source](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/session/subagent-completion.ts)

In each of the four role and turn samples, the log block decreased from **14,999 bytes to 103 bytes**. Retrieval returned the exact original content. This measures local byte reduction. Actual token and billing savings remain unmeasured.

Apply preserved the model and permission definitions. Normal service stop cleared registration, the port, and service processes. Native Remove restored the original config bytes. The test also restored the original engine settings bytes.

The adapter blocked a profile with `experimental` during Preview in the first run. The final candidate used OpenCode's default single level of subagents. In the second run, the verifier rejected the tool list because it included `skill`. The final verifier checked the tools that the client called. The two final turns completed with the same permissions and no `skill` call. Separate records preserve both earlier runs and their verified cleanup.

The final record is in `work/proofs/opencode-v2-service/native-agent-0cc2093b4d/result.json`. All 22,714 native inputs matched before and after the run. Model changes within a session, nested subagents, subagents in the V2 TUI and Desktop, live accounts, and other operating systems await separate acceptance.

The final catalog candidate passed 222 Node and 462 Rust tests. The gate ignored 19 tests. Rustfmt, the frontend build, and the Tauri embedded build passed. All 344 inputs to the final local gate matched before and after.
