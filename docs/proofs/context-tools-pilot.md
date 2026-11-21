<a name="h041--context-mode-ve-context-diet-yerel-pilotu"></a>

# H041 — Local Context Mode and Context Diet pilot

November 16, 2026. **The pilot is complete. General automatic filtering remains outside the accepted scope for both tools.** The user excluded Billion Context. Actual client settings, product sources, and defaults stayed the same.

The test ran on Windows 10 x64. It used the pure Context Diet 0.5.0 filter and CLI code and the actual Context Mode 2.2.0 stdio MCP package. Diet used a pinned Git commit. Mode used an npm package with verified SHA-512 integrity. The 49 Diet upstream tests passed. The final run reused those results after matching 23 source and Node fingerprints. MCP installation and Codex hook compatibility remain outside the test.

**14 synthetic command outputs × 3 repeats × 3 arms = 126 comparisons, plus 6 JSON query requests.** Each repeat used separate Mode and Headroom processes. All three Headroom processes used the coding and cache profile and TTL 7200. Local measurement stayed on. Beacon, model routing, Learn, and the output shaper stayed off.

Mode exited with code 1 on Windows in all three EOF cases due to its SIGTERM behavior. The runs left no ownership files. Headroom shut down gracefully three times. All 838 inputs matched before and after.

<a name="ölçüm-tablosu"></a>

## Measurement table

The table uses **local o200k_base counts for the first upstream request JSON** in the same Responses envelope. Provider token usage and billing are outside this measurement. The Mode arm carried its own MCP text. The other arms carried a stdout, stderr, and exit JSON envelope. Differences in the envelope or escaping alone can change the count. Token counts and acceptance fields matched across all three repeats.

| Scenario | Headroom | Diet → Headroom | Mode → Headroom | Acceptance note |
| --- | ---: | ---: | ---: | --- |
| build_fail | 173 | 173 | 1350 | Separate checks verified output scope and retrieval where available. |
| event_count | 156 | 144 | 139 | Diet reduced 200 events to 1 line. The model-facing output lost the repeat count. |
| git_diff | 287 | 287 | 265 | Separate checks verified output scope and retrieval where available. |
| git_status | 158 | 158 | 939 | Separate checks verified output scope and retrieval where available. |
| json_exact | 2220 | 2065 | 1766 | Diet omitted part of the full JSON. That result did not qualify as a saving. |
| long_source | 13406 | 1882 | 1460 | The first response from each candidate omitted part of the source. A full Read requires passthrough. |
| repeated_log | 155 | 155 | 179 | Diet produced no additional reduction. Mode increased the first request size. |
| short_unicode | 160 | 160 | 146 | Separate checks verified output scope and retrieval where available. |
| source_read | 2790 | 1882 | 2615 | Diet omitted part of the full source. That result did not qualify as a saving. |
| stderr_exit | 175 | 177 | 248 | Separate checks verified output scope and retrieval where available. |
| stderr_success | 163 | 163 | 136 | Mode omitted the successful command's stderr warning from the response and the index. |
| stdout_fail | 156 | 173 | 219 | Both candidates displayed the error line. Headroom also supported exact CCR retrieval. |
| test_fail | 184 | 184 | 1086 | Headroom returned the exact error details on retrieval. Both candidates displayed the details directly. |
| test_pass | 157 | 157 | 1051 | Separate checks verified output scope and retrieval where available. |

The pilot sent 132 requests. For every Headroom transformation, the test **recovered the exact input** through CCR or the pinned lossless resolver for runs and blocks. It made 42 CCR GET checks. Content that Diet had discarded and stderr that Mode did not collect remain outside this retrieval result. The test did not measure model decisions to retrieve content or the token cost of later requests.

<a name="faydalı-kullanım-hedefli-veri-sorgusu"></a>

## Targeted data queries

The numeric JSONL file had 1,000 lines and 36,000 local text tokens.

The hand-written local query returned the requested line and line count in 41 tokens. This was a **99.89% reduction for this response**. The exact text values 18446744073709551617 and 0.1234567890123456789012345 matched in all three repeats. The query did not convert JSON numbers to JavaScript Number. The verified task covered the count and target line only. Full-file reading has separate acceptance.

The record includes execution arguments at 104 local tokens and canonical MCP schema JSON at 1732. Startup instructions used 115 local tokens. The index reply used 20 and the search reply used 265. Overall chat and billing savings remain unmeasured. All three FTS searches returned the complete target line.

<a name="hata-ve-geri-alma-sınırları"></a>

## Errors and retrieval limits

- Diet ran with exactOnly and protected lines. A required line that exceeded the budget produced an explicit error. A manifest can report verbatim even when the filter omits information. General filtering requires repeat counts, complete source and JSON, and an original archive. The pilot adapter preserved the exit code and channel envelope.
- On exit code 0, Mode omitted stderr from the combined text. A targeted search for the unique warning key also returned no warning. A stdout-only shell command with exit code 1 returned isError=false even though its text showed exit code 1. Full Read protection and the app's permission and exit contract still need separate checks.
- Mode normalizes line endings in text that it indexes. Retrieval rebuilt the long CRLF source with normalized endings. Byte-exact archival acceptance remained open.
- Of 21 general short-query checks, 12 showed all required lines. The broad test query gave an incomplete test summary. All six focused searches for result and a unique error ID passed. All three searches for the omitted stderr failed again. The test excluded query echoes as evidence. Model task success requires a separate test.

<a name="karar-ve-sonraki-adım"></a>

## Decision and next step

Context Mode's approach of **querying large data locally and returning the relevant part** remains a candidate. This package remains outside automatic general output capture.

Each client needs to preserve its existing permissions and the full stdout, stderr, exit code, and tool identity. Acceptance also requires byte-exact original storage and task success with the same model and effort. Diet remained outside the runtime dependencies. Headroom already provided the reduction in the log example. Diet added no reduction there and lost data when applied directly. Protected source and numeric Read behavior and product defaults stay in place.

H041 remained open. The total stayed 45/54. The next planned slice measures Chat Completions usage. A safe targeted query prototype can be evaluated afterward. Live models, API bills, overall task quality, new operating systems or installers, and Actions remain outside this pilot. Development token savings remain unmeasured.

[Machine-readable acceptance](context-tools-pilot.json) · [Reproduction driver](../../bench/context_tools/README.md) · [Pinned Context Diet source](https://github.com/techdevfun/context-diet/tree/3899b48dbf41ecc9c0e29aaec8634900101c52d2) · [Reviewed Context Mode source](https://github.com/mksglu/context-mode/blob/751f818d0889f7e7da09d050e695c94ec77463f0/src/core/tools/execute.ts#L138-L167)
