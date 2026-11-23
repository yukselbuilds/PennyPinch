<a name="h041--optimizasyon-ve-gecikme"></a>

# H041 — Optimization and latency

10 November 2026. Local acceptance passed on Windows 10 x64 build 19045. H041 live API-key and Windows 11 checks remain open. [Plan](../decisions/027-optimization-regression.md), [current local evidence](../proofs/optimization-local-acceptance.json).

Run `bench/optimization/replay.py --run` with the project-pinned CPython 3.13.13. The run needs the payload and prepared tokenizer data named by the development manifest. `--self-check` checks the call IDs and history-append structure of four fixtures without network calls. `--run` sends measurement and health requests to a loopback fixture.

Hugging Face and Transformers downloads are off in the copied test layer. The run records zero model-cache load and checks fingerprints. The test did not capture network packets.

The test used three new controller/helper/engine processes and three conversation turns per instance. It sent the same 36 workloads to the direct loopback control and Headroom. It preserved the model, reasoning and synthetic auth. The source code and exact numeric JSON stayed unchanged in the `Read` tool. The forwarded history prefix and tool schemas matched in new turns. CCR returned the original log.

| Fixture | Headroom requests | Local token reduction |
| --- | --- | --- |
| Source code | 9 | 0%, content preserved |
| Exact numeric JSON | 9 | 0%, content preserved |
| Repeated synthetic log | 9 | 98.15–98.23% |
| Short JSON | 9 | 0% |

Both arms counted tokens from fully ordered JSON with the same `o200k_base` encoding. The count includes added tool schemas. Do not present this sample result as a provider cache discount or billing savings. Actual model TTFT and response quality were not measured. Each normal request record contains the time to first fixture content delta, total time and CPU/RAM.

In the current run, warm additional p95 latency was 72.45 milliseconds. Request CPU p95 was 78.125 milliseconds. The observed private engine memory reached 855.67 mebibytes. The preset limits of 1000 milliseconds, 2000 milliseconds and 1536 mebibytes passed. The process sampled memory every 50 milliseconds and at request boundaries. CPU scope includes the benchmark/fixture and relevant controller/helper/engine processes. We measured Rust ingress separately.

The first run completed 24 normal requests. It then stopped because the error test assumed one upstream call. Fixed ProxyConfig defines three attempts for 429. We corrected the runner to check this behavior and record latency.

In the final run, the direct arm used 1 attempt. The Headroom arm used 3 attempts. Both arms preserved 429 and Retry-After. Headroom error responses took about 2.02–2.07 s. Thresholds did not change. [First Responses measurement](../proofs/optimization-benefit.json).

Three processes closed with normal/graceful exit code 0, and their ports closed. The report did not retain raw requests, auth, control secrets or stderr dumps. It marked stderr content as unknown. External provider calls totaled 0. Other local protocol and relay results follow. Real API-key and checks on other operating systems remain open.

<a name="relay-ve-okumayan-istemci"></a>

## Relay and non-reading client

`bench/optimization/backpressure.py` tested the same 4 MiB data in direct TCP and Rust relay arms. The client receive buffer was 2048 bytes, and it did not read for 30 s. The earlier 180 ms delayed-reader test remained valid within its scope. New counters, enabled only in test builds, recorded actual WouldBlock and bytes written where the relay writes to the client.

In the 10/20/30 s samples, WouldBlock increased across the last two intervals, and transfer had not finished. After the client started reading, both arms passed 4 MiB SHA-256 equality and cleanup. Source, plan and test binary did not change. [Backpressure evidence](../proofs/relay-backpressure.json).

An external process sampled the native test process. In the final repeat, the relay arm used 46.875 milliseconds of CPU over about 30 seconds. That equals 0.156% of one core. External sampling found 0 private-memory growth.

From the start of reading to EOF, the direct arm took 174.97 milliseconds. The relay arm took 143.71 milliseconds. These times come from one pair on the same machine. The preset limits of 10% CPU and 32 mebibytes growth passed. The measurement covers one connection and a test build. [First backpressure measurement](../proofs/relay-backpressure.json).

<a name="diğer-protokoller"></a>

## Other protocols

`bench/optimization/surfaces.py --run` used three new engine processes. It ran the same four inputs and three turns through Chat HTTP/SSE, Anthropic HTTP/SSE and Responses WebSocket. All 216 requests passed across 108 matched workloads. The test preserved source/numeric content, tool IDs, model/reasoning, synthetic auth and forwarded history prefixes. The test restored the original outputs for found CCR markers.

| Surface | Synthetic log token reduction | Warm additional p95 | Warm CPU p95 |
| --- | --- | --- | --- |
| Chat HTTP/SSE | 24.64–49.34% | 60.18 ms | 62.5 ms |
| Anthropic HTTP/SSE | 16.24–24.33% | 48.93 ms | 46.875 ms |
| Responses WebSocket | 98.15–98.23% | 13.60 ms | 31.25 ms |

The log samples in this table used different protocol envelopes. Do not apply these results as a general savings rate for all work. Full JSON token counts include added tool schemas. The test did not measure the real Anthropic tokenizer/bill, provider cache hits, actual model TTFT or response quality. It held the client's explicit cache marker constant in the Anthropic arm.

The WebSocket conversation used three turns on one connection. Request timing excluded the opening handshake. The test recorded each new engine's startup time separately and did not clear the operating-system file cache.

The test retained the cost of added schemas for small or protected content. A negative reduction below means an increase in token count. It does not mean that protected tool output changed.

| Surface | Source code | Exact numeric JSON | Short JSON |
| --- | --- | --- | --- |
| Chat HTTP/SSE | −0.10%…−0.07% | −0.07%…−0.05% | −2.50%…−1.79% |
| Anthropic HTTP/SSE | −1.30%…−0.87% | −0.84%…−0.56% | −27.93%…−20.49% |
| Responses WebSocket | 0% | 0% | 0% |

In the short Anthropic sample, the `headroom_retrieve` tool definition caused the full increase. Input and output fingerprints from the three-turn loopback repeat matched the previous nine records. Removing that definition from the forwarded body restored the original request exactly. Full `o200k_base` JSON counts showed 100 extra tokens per turn. The first turn changed from 358 to 458, and the last changed from 488 to 588. [Field-level measurement](../proofs/anthropic-request-overhead.json).

With fixed engine defaults, the engine adds the tool from the first turn when the request has a tool list and allows optimization. Changing the tool list later can invalidate Anthropic's cached common prefix. The `lazy` setting adds the tool when needed. It may reduce this cost in short conversations. The cache effect in a long conversation was not measured with a live provider.

Three engines closed with normal exit code 0, and their ports closed. The current run matched 648 source and tool inputs before and after. An explicit `legacy=True` setting in the pinned library fixed the first run's direct WebSocket warning. The test ended the connection lifecycle in a controlled way. [First protocol measurement](../proofs/optimization-surfaces.json).

<a name="anthropic-ccr-aracının-eklenme-zamanı"></a>

## When to add the Anthropic CCR tool

On 15 November 2026, `bench/optimization/ccr_policy.py --run` started four new processes with the fixed 0.41.0 engine. It reversed the order in two matched repeats. Each process sent three short turns, a five-turn session with a late log and protected numeric Read, and a separate short request. The numbers below were the same in both repeats. Full canonical JSON counts with `o200k_base` included the CCR definition. [Run record](../proofs/anthropic-ccr-policy.json).

| Session or request | Original | `eager` | `lazy` |
| --- | ---: | ---: | ---: |
| Total for three short turns | 1,386 | 1,686 | 1,386 |
| Total for five long turns | 29,376 | 13,560 | 13,260 |
| Turn when the log first arrived in the long session | 10,321 | 2,263 | 2,263 |

Across three short turns, `lazy` reduced the total by 300 tokens, or 17.79%, compared with `eager`. Both settings reduced the first request with the log by 78.07% compared with the original. After the late log arrived, the `lazy` tool array changed once and stayed fixed in the next turn. The `eager` array stayed fixed from the start.

The test preserved the message prefix, synthetic auth, model/effort, tool calls and protected Read content. A separate session did not inherit the `lazy` tool. Eight local CCR requests that needed no model call returned the original log exactly. Four engines exited with code 0 and released their owned ports.

In the first run, the fixture did not append the previous synthetic answer to the new request. Cache mode therefore forwarded the log without compression. The second run used the correct history and compressed the log. It showed that provider-side `stream: false` applied to CCR.

The final run sent JSON to that arm and SSE to the other arm. The client received completed SSE in both cases. This measurement covers these local protocol checks. Provider cache counters, billing impact and actual task success remain open. The product default stayed `eager`.

<a name="anthropic-ccr-ve-kullanım-kapsamı"></a>

## Anthropic CCR and usage coverage

On 15 November 2026, `bench/optimization/anthropic_usage.py --run` ran 3 separate Anthropic Messages sessions on the fixed 0.41.0 engine. It used nine short requests to build history. In the next three requests, the provider fixture called `headroom_retrieve`. The real engine sent one extra upstream request for the same test request and carried the original log in full.

The test observed 12 client requests and 15 upstream requests. It preserved the model, reasoning and synthetic auth. [Numeric record](../proofs/anthropic-usage-coverage.json).

| Final measured request | Client result | Observed upstream usage total |
| --- | --- | --- |
| Two successful calls | SSE 200, final call 1,343 tokens | 2,429 tokens, 300 uncached input + 2,000 read + 100 write + 29 output |
| Cache-write field missing in primary response | SSE 200, write 0 in final call | Write and total unknown, output 29 |
| Continuation request 429, no usage fields | SSE 502, no usage fields | Total unknown |

The custom `/stats` row in the complete sample showed only the last call's four counters. The row in the failed continuation showed the first call's counters. Neither view covers all calls outside the direct upstream fixture response. The fixture verified history with separate session keys and one unique new row. This mapping covers the isolated serial experiment. Product session ownership, real usage and billing need separate acceptance. [Anthropic usage fields](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#tracking-cache-performance).

<a name="anthropic-kullanımının-motorda-kaydı"></a>

## Engine capture of Anthropic usage

`tests/runtime/anthropic_usage_capture.py` used the client owned by the fixed 0.41.0 engine on Windows 10 x64. It converted 15 client requests into 19 upstream sends. Each observation matched the request ID issued by the engine.

The first response and CCR continuation recorded 2,429 synthetic tokens together. The test retained the explicitly reported 0. A missing cache-write value or a failed continuation left the total unknown. A successful retry after 429 also left the total unknown. Two concurrent SSE requests completed with their own counters. [Validation and source fingerprints](../proofs/desktop-anthropic-usage.json).

Split SSE frames combined the first and final usage data. A missing terminal, malformed frame, or size or call limit made the total invalid. The relay delivered the observed stream bytes unchanged. Requests with a missing or wrong control key returned 404. After restart, the engine record was empty and the earlier key was invalid. Records stay in the engine generation's memory.

The current client-response and `/stats` comparison and three local Anthropic pause scenarios passed. The native Windows runtime test passed 35 tests and skipped 6 opt-in tests. The controller passed 10 tests, and Rust format checks passed. This acceptance covers the private engine record. Desktop display, live provider use, other protocols and Windows 11 validation remain open.

<a name="anthropic-kullanımı-masaüstü-istek-ayrıntısında"></a>

## Anthropic usage in desktop request details

In a local Windows 10 x64 experiment, `tests/runtime/anthropic_usage_capture.py --native-reader` started the real engine. Two upstream responses reached SQLite and request details through the ownership-verified native metrics reader. Both had the same generation and exact request ID. The test verified 2,400 total input, 29 output, 2,000 cache read, 100 cache write and two submission attempts. The input-counting basis and savings measurement stayed separate. [Validation record](../proofs/anthropic-usage-desktop-detail.json).

Local Rust and client tests did not match a wrong generation or another request ID. A failed response and missing counters left the total unknown. A temporary omission did not erase a previously verified field. Conflicting later data made usage fields unknown and preserved the local compression difference.

Old detail records opened without the new fields. 464 Rust tests, 223 frontend tests and a real-engine opt-in test passed. Windows 11 and macOS runs, a visual native walkthrough, other providers and live cost coverage remain open.

<a name="süreli-yük-ve-başlangıçta-kompress-yüklemesi"></a>

## Timed load and startup Kompress load

`bench/optimization/stability.py --run` first warmed up with 96 requests. It used four surfaces × four inputs × three turns × two arms. It then sent matched workloads at one per second for 300 seconds. The 300 pairs/600 requests completed source/numeric/tool/prefix/CCR checks. Combined benchmark/fixture/controller/helper/engine CPU was 6.24% of one core. Additional p95 was 48.67 ms, and request CPU p95 was 31.25 ms.

In the first run, observed private-memory growth after warmup was about 605 MiB. It exceeded the 128 MiB limit. Peak memory was about 1,470 MiB, and ending memory was about 1,207 MiB. Normal exit code 0 and port closure passed. 646 inputs matched. [Failed run](../proofs/optimization-stability-failure.json).

A 120-second diagnosis found that monitored content and record stores stayed near 0.2 mebibytes. The engine process showed the increase. The pinned source showed a background load of Kompress after model start. Headroom Desktop disabled this background load in its engine. It kept the path that uses Kompress when a request needs it.

In the next run with the same 300-second plan, private-memory growth was 56.18 mebibytes. Peak was 1,138.81 mebibytes. Additional p95 latency was 49.88 milliseconds. Both runs wrote separate Kompress files to cache. These observations alone do not separate model-file download cost from model-load cost. [Run after correction with default engine settings](../proofs/optimization-stability.json).

The 0.41.0 run disabled model-file downloads. Under the same plan, it completed 300 pairs, 600 requests and 249 CCR retrievals. Growth was 24.12 mebibytes. Peak private memory was 889.40 mebibytes. Combined CPU was 4.09% of one core. Additional p95 latency was 47.04 milliseconds.

All preset thresholds for the five-minute window passed. 653 inputs matched. The engine exited normally with code 0, and the port was free. The Hugging Face Hub cache held zero bytes of model payload. Provider models, concurrent multi-client load and hours-long operation remain separate scopes.

<a name="headroom-0440-ile-süreli-yük"></a>

## Timed load with Headroom 0.44.0

18 November 2026. The previous 300-second plan and thresholds ran with the current 0.44.0 pin. The Windows 10 run completed 96 warmup requests and 300 measured pairs across Responses HTTP/SSE, Chat HTTP/SSE, Anthropic HTTP/SSE and Responses WebSocket.

Source, exact numeric content, tool ID and prefix checks passed in 600 measured requests. The run verified 249 CCR retrievals exactly. 688 source and tool inputs matched. The engine exited normally with code 0, and the owned port was free.

| Measurement | Observed | Plan limit |
| --- | ---: | ---: |
| Measured pairs | 300 | At least 250 |
| Private-memory growth after warmup | 22.96 MiB | At most 128 MiB |
| Observed private-memory peak | 914.59 MiB | At most 1,536 MiB |
| Additional p95 latency | 46.14 ms | At most 1,000 ms |
| Combined CPU as a percentage of one core | 3.32% | At most 25% |

The following values are full JSON bodies counted with the same `o200k_base` encoding across 75 measured requests per sample. These counts include added tool schemas. The values show the main request size in local synthetic traffic. [Programmatic evidence](../proofs/optimization-local-acceptance.json).

| Synthetic sample | Original tokens | Forwarded tokens | Difference |
| --- | ---: | ---: | ---: |
| Source read | 724,026 | 725,970 | 0.27% increase |
| Exact numeric read | 1,114,776 | 1,116,720 | 0.17% increase |
| Repeated log | 3,110,226 | 1,079,854 | 65.28% reduction |
| Short JSON | 37,476 | 39,420 | 5.19% increase |

<a name="h041-açık-kabulleri"></a>

## Open H041 acceptance criteria

On 19 November 2026, **6 verification criteria and 3 optional R&D criteria** were open. The main task count was **45/54**.

| Criterion | Evidence needed |
| --- | --- |
| Restricted native Windows file profile for Codex | Check an allowed read in a suitable native sandbox. Check an explicit access denial. The legacy sandbox on Windows 10 did not start the profile. WSL2 verified the same file boundary. [File and archive evidence](../proofs/targeted-query-comparison.json) |
| Windows 11 x64 | Test the packaged 0.44.0 engine, shutdown, and traffic on actual Windows 11 x64. The standard GitHub Windows 11 image was ARM64. CircleCI offered Windows Server images. [Standard GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-private-repositories) · [CircleCI Windows images](https://circleci.com/docs/guides/execute/using-windows/) |
| macOS ARM64 | **Verified on 24 November.** The 0.45.0 package passed the controlled Codex Desktop turn, local usage, and normal exit checks. The earlier 0.44.0 package passed the start, stop, and restart checks. [Current package evidence](../proofs/headroom-045-managed-packages.json) · [Earlier lifecycle](../proofs/macos-044-package-lifecycle.json) |
| Live Anthropic Messages | Use an authorized account. Match the usage counters of an actual Messages response to the corresponding Headroom request. [Local usage evidence](../proofs/desktop-anthropic-usage.json) |
| Live OpenAI API-key route | Confirm authorized access to the API-key route. Collect usage evidence for the scope that H008 carried forward. [H041-d plan](../decisions/027-optimization-regression.md#h041-d-responses-httpsse-kullanım-ölçümü) |
| Actual billing difference | Collect comparable billing data for the paired runs. The token comparison is on record. [Live comparison](../proofs/targeted-query-comparison.json) |
| Broader acceptance of Learn benefits | Pair tasks with a known repeat pattern on the same model and effort. The first live pair showed no benefit. [Learn measurement](../proofs/learn-merit-pilot.json) |
| Experimental rules e3 | The work is frozen. A restart requires a new candidate and predefined criteria for quality and net benefit. [R&D decision](../research/optimization-roi-review-2026-11-17.md) |
| Experimental rules e4 | After e3 passes, verify enablement, disabling, and rollback within the selected scope. [Experimental rules plan](../decisions/027-optimization-regression.md#h041-e-deneysel-kurallar) |

<a name="19-kasım--kalan-kabullerin-başlatma-koşulları"></a>

### 19 November — Launch conditions for remaining acceptance

- **GitHub macOS.** On candidate 6531cc4, `macos-package.yml` selects the manual `macos-15` job. The local precheck read the workflow, two Bash steps, three Bash scripts, and six Python sources. All 230 input digests matched before and after. The precheck verified the 0.44.0 pin. Confirm the free quota and zero-spend limit before starting the job with `verify_desktop_traffic=true`. Local precheck (local receipt `work/proofs/macos/h041-package-preflight-3f48e53e19.json`) · [Workflow](../../.github/workflows/macos-package.yml).
- **Quota and duration.** The team verified GitHub CLI access on 19 November and read the private repository's past runs. The user confirmed USD 0.10 of quota. The CLI credential lacks the `user` scope for the account usage summary.

  USD 0.10 covers one full Mac minute. The prior actual Headroom 0.41.0 package and Desktop traffic job took 5 minutes and 39 seconds. At the rate recorded that day, 6 rounded minutes used USD 0.372 of included quota.

  The 15-minute limit for the current 0.44.0 job represents USD 0.93 of included quota. The new job awaits confirmation of sufficient free capacity. [Prior job](https://github.com/Protocol272/headroom-desktop/actions/runs/43284385567) · Duration evidence (local receipt `work/proofs/macos/h041-remote-history-0136ac2a66.json`) · [GitHub rates](https://docs.github.com/en/billing/reference/actions-runner-pricing) · [Quota renewal](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
- **CircleCI macOS option.** At this checkpoint, Free plan capacity for private repositories was 30,000 credits per month. Apple Silicon `m4pro.medium` used 200 credits per minute. At that rate, 15 minutes used 3,000 credits.

  This option requires authorization to connect the private repository to CircleCI. Confirm the account quota. Run the native GUI precheck. [Plan](https://circleci.com/pricing/) · [Rates](https://circleci.com/pricing/price-list/) · [Apple Silicon images](https://circleci.com/docs/guides/execute/using-macos/).
- **Native Codex profile.** The running Codex process used CLI 0.169.0-alpha.11.1. The test computer ran Windows 10 x64 build 19045. MXC requires a suitable current Windows 11 environment. The earlier legacy elevated test stopped before the command because it required `:root` read access. [OpenAI sandbox compatibility](https://learn.chatgpt.com/docs/windows/windows-sandbox#mxc-compatibility) · [Earlier run](../proofs/targeted-query-comparison.json).

The local precheck report has SHA-256 `38ce4294dd93fadb20762023699e96f197c362f61b73e3eb5908770d1dff4b63`. At that checkpoint, native macOS acceptance awaited an actual run. The table above shows the current package result.

<a name="18-kasım-erişim-kontrolü"></a>

### 18 November access check

- The test computer ran Windows 10 x64 build 19045 and Codex CLI 0.169.0-alpha.11.1. The earlier unelevated profile did not start the command. MXC was unavailable. The separate ACL check verifies an allowed read and an access denial within its scope. The named profile's two file boundaries require MXC or an elevated sandbox that an administrator configured. [Earlier run](../proofs/targeted-query-comparison.json) · [OpenAI Windows sandbox documentation](https://learn.chatgpt.com/docs/windows/windows-sandbox#mxc-compatibility).
- GitHub CLI reported an invalid session. The browser billing page could not show the account quota. The macOS ARM64 run still requires confirmation of included quota and the zero-charge limit. The standard Windows 11 runner label was ARM64. Windows 11 x64 acceptance awaits the matching desktop environment. [GitHub runner table](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-private-repositories) · [GitHub Actions usage quota](https://docs.github.com/en/billing/concepts/product-billing/github-actions#included-compute-usage-and-storage).
- OpenCode provides its free chat models through its own client. H041 requires an authorized API route for direct observation of remote Chat Completions in this project. [OpenCode Console endpoints](https://opencode.ai/docs/console/models/#endpoints).

<a name="19-kasım--uzak-chat-completions-sayaç-kaydı"></a>

### 19 November — Remote Chat Completions usage records

The test used one submission per transport through the documented anonymous AI Horde API. The isolated project Headroom 0.44.0 engine sent synthetic arithmetic questions to one fixed HTTPS endpoint. Both HTTP 200 responses matched the request ID, completion state, and provider usage values in their engine submission records. Each request used 49 input tokens and 2 output tokens.

The provider did not mark the counters as estimates. Cache reads and writes remain unknown. The SSE response included the final usage frame and `[DONE]`. [AI Horde usage contract](https://haidra-org.github.io/AI-Horde/reference/chat_front/#reasoning-and-usage) · [Measurement](../proofs/chat-completions-usage.json).

The SSE output matched the expected single digit exactly. The JSON response ended normally and kept its counters. Its model output failed the single-digit format check. The record keeps the earlier schema-constrained 404 and runs that stopped at completion or format checks. This check covers live counter delivery.

Billing difference and savings percentage remain unmeasured.

<a name="19-kasım--kısıtlı-codex-dosya-profili"></a>

### 19 November — Restricted Codex file profile

The test tried the `elevated` backend in a prepared Windows profile with only earlier local fixture traffic. CLI 0.169.0-alpha.11.1 stopped before the command at a check that required `:root` read permission. The run provides no file-denial evidence. [Pinned Codex source](https://github.com/openai/codex/blob/226fa88979b1a9d76dc36f339ee9c5fbdb2c8e7f/codex-rs/windows-sandbox-rs/src/resolved_permissions.rs#L114).

The WSL2 test used the official Linux binary of the same CLI version and Bubblewrap 0.9.0 from the Ubuntu package. The team extracted Bubblewrap inside the project. The `headroom-file-scope` profile denied reads at the root and allowed only the test scope and required runtime paths.

The first and last checks opened both synthetic files. The intervening check read the allowed file exactly and returned an access denial for the target file.

Archive queries and full readback on the allowed file verified 48 records and 5,926 original bytes. Seventeen inputs matched.

The run covers the Codex CLI file boundary inside WSL2. It made no model calls. [Codex sandbox documentation](https://learn.chatgpt.com/docs/sandboxing) · [Programmatic evidence](../proofs/targeted-query-comparison.json).

<a name="19-kasım--anthropic-messages-gerçek-yerel-inference"></a>

### 19 November — Anthropic Messages with real local inference

On Windows 10, the pinned Headroom 0.44.0 engine and llama.cpp b12359 completed two synthetic requests with local SmolLM2-135M. JSON and SSE both returned correct schema-constrained answers and ended normally. Each used one provider submission that matched the engine request ID. The test read the final SSE usage values as cumulative counters. [llama.cpp Messages endpoint](https://github.com/ggml-org/llama.cpp/blob/b12359/tools/server/README.md#anthropic-compatible-api-endpoints) · [Anthropic stream contract](https://platform.claude.com/docs/en/build-with-claude/streaming).

| Response | Input outside cache | Cache reads | Cache writes | Output |
| --- | ---: | ---: | --- | ---: |
| JSON | 44 | 0 | Unknown | 15 |
| SSE | 11 | 33 | Unknown | 15 |

The two requests ran in order on the same local server. The local provider reported 33 cache-read tokens. The missing cache-write field left total input and full usage unknown.

Three parser checks and seven request-boundary checks passed. The local engine and model server shut down. Live Anthropic account and billing comparisons remain open. [Programmatic evidence](../proofs/desktop-anthropic-usage.json).

<a name="19-kasım--gerçek-yerel-modelde-eşlenmiş-chat-completions-kıyası"></a>

### 19 November — Paired Chat Completions comparison with a real local model

On Windows 10, the test compared a direct request with Headroom 0.44.0 using the default coding and cache profile. Both arms used llama.cpp b12359 and SmolLM2-135M. They read the same answer from four synthetic logs of 24 lines each. Prompt caching was off. Both responses were correct and complete. The numbers come from the model server's SSE usage counters.

| Route | Input | Output | Total |
| --- | ---: | ---: | ---: |
| Direct | 6,647 | 10 | 6,657 |
| Through Headroom | 6,647 | 10 | 6,657 |

Both arms forwarded 12,013 bytes of tool output. Input savings were 0%. The Headroom usage record matched the single model request. All 67 input and tool files matched.

The engine, local model server, and owned ports closed. The cloud model call count was 0. The result covers one pair on this workload. [Runner](../../bench/optimization/chat_local_savings.py) · Local evidence (local receipt `work/proofs/optimization/chat-real-inference/pairs/cdbbc80e8eba4368b4db81e640194218/result.json`)

<a name="20-kasım--headroom-benchmark-tekrarları"></a>

## 20 November — Headroom benchmark reruns

Windows 10 22H2 ran 33 scenarios from the pinned upstream SDK script with packaged Headroom 0.44.0 and CPython 3.13.13. The SDK measured six headline examples in three warmup and ten measured rounds. The count fell from 126,289 to 55,209 tokens (56.28%).

The upstream page reported 56% with Headroom 0.37.0 and an Apple M-series system. Local compression p50 for the six examples ranged from 0.48 to 5.32 ms. [Upstream benchmark page](https://github.com/headroomlabs-ai/headroom/blob/b21479e0fa84db5f3e46bb79f96c962de4d5fdc6/docs/content/docs/benchmarks.mdx) · [Runner](https://github.com/headroomlabs-ai/headroom/blob/b21479e0fa84db5f3e46bb79f96c962de4d5fdc6/benchmarks/bench_latency.py).

The test paired the full Chat Completions SSE requests for those synthetic examples and counted them with `o200k_base`. On the six headline examples, direct used 122,922 tokens, SDK used 69,280 (43.64% lower), and the managed desktop proxy used 69,328 (43.60% lower).

The counts cover the full request JSON. The test preserved source code exactly. At index 67, a reconstructed JSON error case kept the code, solution, and value 731 on the same compressed line. The SDK and proxy messages matched in that case. The proxy added `stream_options.include_usage`.

Both local runs closed the owned engine and port without model calls. [Programmatic evidence](../proofs/upstream-benchmarks-044.json).

The HTML test used the pinned Scrapinghub dataset and HTML dependencies in the test environment. On 181 pages, F1 was 0.9168 and average character reduction was 94.98%. This exceeded the upstream full-dataset F1 threshold of 0.90. Paid SQuAD model evaluation, actual provider billing, and end-to-end response time remain unmeasured. [HTML test source](https://github.com/headroomlabs-ai/headroom/blob/b21479e0fa84db5f3e46bb79f96c962de4d5fdc6/tests/test_evals/test_html_oss_benchmarks.py) · [Programmatic evidence](../proofs/upstream-benchmarks-044.json).

## 24 November — Matched Headroom 0.45.0 comparison

Headroom 0.45.0 adds a clean-worktree requirement to the code-mode benchmark and removes the duplicate index table for protect_recent=0. The SDK runner, fixture sources, benchmark page, and HTML evaluator match the 0.44.0 files. These changes do not update the measurements on the upstream benchmark page. [Release notes](https://github.com/headroomlabs-ai/headroom/releases/tag/v0.45.0) · [Pinned benchmark page](https://github.com/headroomlabs-ai/headroom/blob/ce0460cfa8598dc939243dbc7aec4b2d35f4e44a/docs/content/docs/benchmarks.mdx).

Windows 10 22H2 ran both versions with the same fixtures. The runner fixed generated IDs before each run to keep the inputs identical. Input hashes and token counts matched in all 33 SDK cases and all eight proxy cases. The six headline cases use two counting methods.

| Method | Before | After | Change |
| --- | ---: | ---: | ---: |
| Upstream SDK message counter | 126,289 | 55,209 | 56.28% lower |
| Full Chat Completions JSON through the managed proxy | 122,944 | 69,350 | 43.59% lower |

The proxy sent 53,594 fewer input tokens in the six headline cases. The count uses `o200k_base` and includes the full request JSON. The SDK produced 69,302 tokens when measured on the same full JSON. Both engines preserved the Python source exactly. They also preserved the error code, resolution, and value 731 in the same synthetic error record.

The HTML extraction test returned F1 of 0.9168 on 181 pages. The mean character reduction was 94.98%. The test passed the upstream F1 threshold of 0.90 and matched the previous result.

These runs used local test inputs and zero model calls. Provider billing, live response quality, and end-to-end response times remain unmeasured. [Matched measurements and source hashes](../proofs/upstream-benchmarks-045.json) · [Earlier 0.44.0 run](../proofs/upstream-benchmarks-044.json).
