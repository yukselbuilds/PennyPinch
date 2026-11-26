<a name="h041--optimizasyon-ve-gecikme-karşılaştırması"></a>

# H041 — Optimization and latency comparison

<a name="başarılı-görev-başına-net-tasarruf"></a>

## Net savings per successfully completed task

On 15 November 2026, the user's suggestions were compared with the existing H041/H043 evidence and the pinned engine. The goal is to reduce total usage per correctly completed task under the same model, reasoning effort, and tool permissions. Retrieval, extra model turns, and retries count toward usage. Local tokenizer counts, provider usage counters, and API billing figures retain their own measurement bases. Usage unobserved at the provider remains `not_tested`.

1. **First slice, H041-b.** Measure the `eager` and `lazy` settings on the pinned 0.41.0 engine. In a short session, `lazy` may omit an unneeded CCR tool definition. In a long session, adding the definition at the first compression can change Anthropic's shared cache prefix. A session carrying an earlier CCR marker retains the retrieval tool. The threshold must avoid adding and removing the schema on every request. [Tool and cache contract](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-use-with-prompt-caching).
2. **Measurement contract.** Alongside the existing `cache_hit` observation, identify the scope of the provider's verified `cached_tokens` or `cache_read_input_tokens` and any cache-write and output counters. Include CCR calls and their extra turns in the net-benefit calculation when they can be tied to the same session. Keep unknown values and negative results. Establish reliable measurement before choosing a setting or profile.
3. **Exact repeats in new results.** Test the pinned engine's deduplication path first. A new result qualifies only after its identity and content have been verified within the same session. Preserve the prefix of previously forwarded messages, status codes, and access to existing CCR entries. Even when the original bytes can be retrieved, verify that the model finds the necessary information in time to complete the task correctly.
4. **Log and test output.** H044 keeps its RTK installation prerequisite. A separate H041 experiment may shorten repetitive success lines while showing the full error and necessary context immediately. Preserve the original streams from the first run, the exit code, and tool identity. Measure gains against the Headroom baseline. Retrieval requires access to the original output from the first run.
5. **Code and client file transfers.** Initially forward protected Read output, exact numeric JSON, and diff source material intact. AST processing and logs written to files are later candidates, subject to an explicit content and route contract, original-data retrieval, task correctness, and a memory limit. Kompress background warm-up is disabled while on-demand loading remains available. A Caveman-style concise-answer preference is a separate experiment that users can see. Preserve the model, reasoning effort, history, and default response scope.

**25 November, Codex shell output.** On Responses HTTP/SSE and WebSocket, preserve the process status for a completed `exec_command` with a unique call ID. If compression omits the success status, copy the original status line before forwarding. Forward a failed result in full. Apply the correction before prefix replay. Keep the original content available through existing CCR. [Verification](../proofs/responses-shell-status-045.json)

<a name="birebir-tekrar-eden-yeni-araç-çıktısı-deneyi"></a>

### Experiment on exact repeats in new tool output

The lead agent verifies the cache prefix, tool identity, and retrieval of original content. The first engine startup showed that deduplication is enabled in the 0.44.0 `coding` profile. Compare that profile with the same engine running `HEADROOM_DEDUPE=0` for the test process only. Check the active setting in each arm's startup record. Fix input limits and time limits in `bench/optimization/dedup.py` before running. Responses HTTP, Chat Completions, and Anthropic Messages carry the same synthetic scenarios. Reverse the order of the arms in the two paired runs. The requests reach a loopback provider without a model call.

1. A new call ID with the same tool and arguments returns the same output. A request that only resends history contains no new result. In a new session, a reference can point only to accessible earlier content in that request. Keep the messages and tool definitions already sent.
2. Preserve nonzero exits, separate stderr, exact numbers, and full source reads. Shared lines alone cannot qualify different results as exact repeats. A reference in the output must resolve to a single original sequence using only the earlier text sent to the model. An existing CCR marker must resolve to the exact original through the real engine retrieval path.
3. The difference between the arms measures the current filter's contribution. A new rule needs correctness checks and evidence of incremental savings over the current profile. Count the full request JSON and the new tool output separately with `o200k_base`. Assign shrinkage in replayed history to that history. Timing is observed locally. Provider caching, billing, and task quality each require their own evidence.

The 0.44.0 run on Windows 10 completed four engine runs, 168 paired submissions, and 32 exact CCR reads. In both repeats of the short Responses duplicate example, the full request dropped from 405 to 306 local tokens, and the new tool result dropped from 112 to 19 tokens. The third request, with no new call, showed the same 99-token difference in the history it carried. The setting made zero difference in the Chat Completions and Anthropic examples. Full source, numeric reads, session separation, and error lines were preserved. [Evidence](../proofs/duplicate-tool-results.json).

Responses also shortened a block shared by different results. In one case the previous result held two different spans with the same abbreviated prefix tag, and the reference confused those spans. Even though both spans were present in the actual first submission, the reference could not distinguish them. Both repeats showed the same limit. The next Responses slice will require verified exact new output and a reference that resolves uniquely in the final text previously sent. Keep original output on ambiguity. The pilot left product behavior as it was.

In Anthropic, 14 earlier message prefixes changed in both arms. Focused diagnosis of two short flows identified `cache_control` as the changed field. The failed tool result flag was preserved. Moving automatic cache markers needs separate investigation from deduplication. Acceptance of an actual provider cache effect requires that provider's usage measurements.

<a name="responses-tekrar-referansının-doğruluğu"></a>

### Correctness of repeat references in Responses

The current 0.45.0 runtime disables cross-turn repeat references in Responses. The later model test found a wrong answer after replacing the latest result with a reference. Keep normal per-result compression and CCR retrieval. The 0.44.0 pilot below recorded the wire behavior of the retired rule. [Model comparison](../proofs/responses-repeat-local-model-045.json) · [Correction evidence](../proofs/responses-repeat-output-045.json)

The lead agent owns the engine's final wire format and call scope. For Responses HTTP and WebSocket, replace the previous span reference with a full-output reference for a new `function_call_output` under the same explicit session ID, tool, and arguments. The earlier call and its complete result must be visible in the same request sequence. The earlier result must equal the new raw result, and the call ID must resolve exactly once. References apply only to full results. The rule uses the current deduplication setting and messages already sent, with no need for a response archive or a new tool.

1. Use two distinct call IDs with the same tool, arguments, and full output. The second result points to the single preceding call ID and shrinks under the same local tokenizer. The next request carries the same history without reporting an additional repeat. Pausing preserves access to the existing reference.
2. Cover the earlier ambiguous-span example, different arguments or error/exit envelopes, full source, and exact numbers. These inputs keep the original data and existing CCR retrieval behavior without emitting a new reference.
3. Cover a missing session ID, a new scope, a colliding call ID, or only a reference to a previous response. The rule requires verification of original output in the same request. If a later request reuses the source or target ID, it restores the original result in place of the carried reference.

Run focused protocol tests, HTTP and WebSocket inputs on the real engine, and CCR regression tests. The final candidate goes through the local quality gate in `docs/development/WORKFLOW.md`. The usage evidence consists of local counts for the request JSON and output. Live task quality and provider caching have separate acceptance criteria.

Local acceptance passed. The same fixture dropped from 405 to 307 o200k_base input tokens (24.20%) over both transports. The checks covered 52 submissions, original output after two call ID collisions, three exact CCR retrievals, and pausing an active stream. [Verification record](../proofs/responses-repeat-safety.json).

<a name="anthropic-otomatik-cache-işareti"></a>

### Anthropic automatic cache markers

The lead agent owns the protocol and counters. Observe the block marker added by the pinned 0.44.0 engine, the client's block markers, and Anthropic's top-level `cache_control` independently. Pair plain-text block cases with short additions, the 19- and 20-position limits, and longer additions. Measure the previously sent content, marker position, and documented reach to an earlier cache write separately. [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#explicit-cache-breakpoints).

- Preserve explicit earlier and new markers, model, auth, content, TTL, and cache carriers across matching requests. Inspect the actual upstream body to see whether the engine adds a marker alongside a client's top-level marker or when the four-marker budget is full. Include the preceding automatic marker in the same session and the pause flow.
- Tie JSON and SSE fixture usage to the engine's private usage record. Read and write counters come from the fixture response. Position distance measures reach along the blocks. Provider counters and billing require their own data.
- If a conflict is verified, correct ownership of the extra marker generated by Headroom. A custom long-session cache policy requires live read and write measurements. Run regressions for existing CCR, pausing, and explicit client markers. The final product candidate goes through the local quality gate.

Local acceptance passed. The 24 paired requests reproduced two block-budget conflicts and one top-level 1h conflict. The final candidate resolved all three. Prior unmarked content, explicit markers, and usage from the JSON/SSE responses were preserved. On the plain-text sequence the earlier write point was reachable at 19 positions. With additions of 20 and 22 positions, an explicit client marker kept the earlier point reachable. Live cache hits and billing measurements are pending. [Local verification](../proofs/anthropic-cache-markers.json).

<a name="context-mode--context-diet-pilotu"></a>

### Context Mode / Context Diet pilot

At the user's request, on 16 November the two candidates ran without model calls in separate processes and state roots. Billion Context was outside the pilot. The experiment ran 14 stdout/stderr/exit outputs three times through Headroom, Diet → Headroom, and Mode → Headroom. Three additional JSON query examples compared text and exact numbers. [Plan](../../bench/context_tools/README.md), [acceptance and limits](../proofs/context-tools-pilot.md).

**Querying large data locally and retrieving the needed portion is a candidate. Neither package was selected as a general automatic filter.** In a handwritten query, calculating a count and returning one record from 1,000 JSONL lines reduced local text from 36,000 to 41 tokens. The schema, instructions, call arguments, and extra lookup were counted separately. The result covers local text, with total task and billing savings requiring their own measurements. Mode FTS also retrieved the exact target line. A broad test query and a query for one error or line produced different results. An echo of the query did not meet the evidence requirement.

Even Diet exactOnly can lose repetition counts and full source or JSON too large for its budget, and it has no original archive. When a process succeeds, Mode omits stderr from the combined text. A stdout-only shell exit 1 yields isError=false. The index normalizes CRLF and final lines. These behaviors require explicit handling before use on user output. The pilot found no incremental benefit over Headroom's existing log reduction to justify Diet as a separate runtime dependency.

The next candidate is a bounded local query after usage measurement is complete. Preserve permissions in the same client, model and effort, stdout/stderr/exit, and tool identity. Full source and numeric Read must pass, and the app-owned store must retrieve the original bytes from the first execution. Changing the default requires equivalent task success with a real agent and proof of additional turns, retrieval, and total usage. General code execution beyond the existing permissions and resummarizing entire chats are outside this acceptance. H041 remains open.

<a name="güvenli-hedefli-sorgu-prototipi"></a>

### Safe targeted-query prototype

The lead agent owns implementation because it involves archive access and measurement. The developer-only prototype returns the exact identity and record count for a specific JSONL report already completed through an authorized path. It has no independent file, command, or network access. The temporary archive is bound to trusted session authorization supplied by the caller. Query arguments cannot determine that authorization. Test each of these separately: a different scope, archive expiry, an unknown record, and exceeded capacity. The original stdout/stderr bytes, tool identity, and call identity are retrievable.

- Normal, error, and boundary acceptance. Preserve original line endings, large integers, and exact decimals. Keep the full-output path for missing or duplicate records, malformed JSONL, an exceeded budget, stderr, and nonzero exits. Verify exact full-source and numeric Read on the local engine. A query over incomplete data cannot report success.
- Three isolated paired runs. Compare full Read on the existing Headroom engine with targeted query through the same engine. Use the same model, effort, and auth in both arms. Count the initial request, tool schema, short archive reference, arguments, and subsequent request. Count the third request that also asks for full output separately. The provider fixture establishes transport behavior only. Real-agent quality, caching, output tokens, and billing have separate acceptance criteria.
- Limits are 1 MiB per input, 4 MiB and 16 records per archive, 64 KiB per JSONL line, 10,000 lines, and one synchronous local query. Verify the relevant driver and source fingerprints without changing product behavior. Product integration also requires acceptance of archive lifetime and client authorization through the real client path.

The local run finished. Across three paired repeats, 27 submissions and three normal engine shutdowns passed. For the 1,000-line example, returning one record and the total count reduced the total for full request JSON from 60,295 to 693–709. Including a full read in the third request increased the total to 61,200–61,228. The checks passed for exact archive retrieval, tool identity, source and numeric Read, stdout/stderr/exit, and the authorization boundary. Each prototype run held its archive in memory. Live client permissions, task outcome on the same model, provider tokens, and billing are still open. [Evidence](../proofs/targeted-query-pilot.json).

Run the command from the project root.

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/context_tools/targeted_query_replay.py --run
```

<a name="hedefli-sorgunun-gerçek-cli-oturum-sınırı"></a>

#### Actual CLI session boundary for targeted queries

Next slice, 17 November. Bind the existing archive to the stdio MCP server owned by this Codex CLI process only. The lead agent owns authorization and ownership decisions. The source is the existing child process that generates the fixed synthetic report. The server rejects user files, shell commands, URLs, and client-supplied owner IDs. Each server receives a new in-memory archive and authorization. Only the real MCP request ID is attached to the source metadata. It does not identify a model function call or a subagent.

- Authorized flow through the real CLI is report reference → exact ID query → original output. Preserve the record, count, line endings, stdout/stderr/exit, and the source MCP request ID. Set `matched_count=1` and `truncated=false` only after successfully scanning the full JSONL. On error, an unknown count stays unknown.
- A new stdio server rejects lookups and original reads using the first CLI's reference. Model arguments cannot choose authorization, files, or commands. Restarting the server also invalidates old references.
- With the same real CLI's `disabled_tools` setting, the original-read tool is omitted from the catalog. A forced call from the provider fixture returns no original content. This verifies the separate MCP tool permission. Native Read and shell file sandbox behavior need their own evidence.

Verify with the new focused tests in `tests/runtime/test_targeted_query_client.py` and `bench/context_tools/targeted_query_client_replay.py --run --codex <pinli CLI>` on the pinned CLI and 0.44.0 engine. Run these commands once the files have been added. The provider is a loopback fixture with synthetic auth and scripted model results and usage counters. Real model quality and savings, general file permissions, subagents, other clients, and other operating systems are still open. The same H041 main task continues. e3/e4 stay closed, and product defaults and real client settings stay in place.

In the precheck under the default CLI budget, the text sent toward the model for a full original read was cut off at 11,984 characters. The local tool result was 128,099 bytes, with a truncation marker and no CCR marker. Keep this failed acceptance in the record. The final transport and permissions candidate sets `tool_output_token_limit=65536` only in the isolated fixture profile. The data and exact-readback requirements still apply. Full readback under the default CLI budget and production readiness require separate acceptance. General client integration still needs an authorized chunked read or another way to meet the budget. The setting's scope was compared with the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

In the first high-budget attempt, the original tool output was complete in the CLI but was compressed again on the model path. A generic `read_original` tool name did not protect it. Register the MCP original-read action as `Read`, which the pinned engine recognizes for source and byte-exact protection. The existing internal archive method and general client and engine settings still apply. Final acceptance requires an exact original envelope at both the CLI and engine output.

The local precheck passed six focused tests and 11 loopback submissions with 11 usage records across three real CLI scenarios on the final candidate. The target record, the 1,000-record count, and the original stdout/stderr/exit/source identity matched exactly. The new CLI's query and original read were denied. A disabled `Read` was absent from the catalog, its forced call did not run, and the next allowed query worked. Direct MCP returned `isError=true`. The pinned CLI reported `status=failed` without carrying that field into its JSON record. A missing flag remains unknown, and the failure is verified separately. Before, after, and current fingerprints and normal engine shutdown passed. Counting diagnostic runs, the total was 65 local submissions and zero real provider calls. Prior failure records were retained. [Evidence](../proofs/targeted-query-client.json).

The following sections record local acceptance after this precheck. Actual file and agent permissions, live model quality, and total usage and cost including all extra turns remain open.

<a name="varsayılan-cli-bütçesinde-parçalı-geri-okuma"></a>

#### Chunked readback under the default CLI budget

18 November. The same owned `Read` tool returns UTF-8 text and a byte range for stdout or stderr. The raw chunk is at most 6 KiB and the encoded response at most 8 KiB. Reads preserve UTF-8 character boundaries. Join ranges in order. Missing, duplicate, or out-of-order chunks cannot produce a successful full read. Verify the source MCP request ID and exit code on each chunk.

Four local scenarios passed on Windows 10, Codex CLI 0.169.0-alpha.11.1, and the current 0.44.0 engine. The default output budget was used. All 107,498 bytes came back exactly across 18 stdout chunks and an empty stderr record. A separate failed report retained 16 stdout bytes, 23 stderr bytes, and exit code 9. Another CLI could neither query nor read the report. A disabled `Read` produced no result, and the allowed query worked. In the first 22-request run, the 10-row limit in the raw diagnostic view prevented complete usage matching. The final candidate uses the panel's `recent_requests` view. All 34 test requests matched an engine usage row. Ten focused tests and normal engine shutdown passed. [Evidence](../proofs/targeted-query-default-budget.json)

In this large example, the initial reference, query, and full chunked read sent 22 requests in total to the local fixture. The run's responses and usage counters came from the fixture. For the live comparison, define the full read, targeted query, and any necessary readback in advance on the same model and effort. Include every extra request, including chunked readback, in total usage. Actual file and subagent permissions and provider cost require separate acceptance.

<a name="eşlenmiş-tam-read-ve-hedefli-sorgu-provası"></a>

#### Paired rehearsal for full Read and targeted query

18 November. The 48 JSONL records fixed in advance occupy 5,926 bytes and fit in a single default Read response. Both arms use the same question, asking for the exact record, line count, and all digits of the numeric values. The candidate may call `Read` if needed. The full-output path, tool schemas, and readback transfers count toward usage. The first provider request in each arm must start with zero cache hits. Verify the exact answer and each request's input and output counters. The run is limited to one pair, with a budget of four provider submissions per arm and eight per pair.

The local fixture rehearsal on the real CLI and 0.44.0 engine produced 3 + 4 requests, seven usage rows, and two normal engine shutdowns. The candidate rehearsal deliberately requested the full read and counted its extra request. The 12 focused checks passed. On the live ChatGPT Free attempt, the control arm's first four requests returned HTTP 200 and the CLI received the report reference. The limit stopped the next request before it reached the provider. A full Read and final answer were unobserved, and the candidate arm never started. Live savings percentage and cost are unknown. Actual file and subagent permissions are still open. [Evidence](../proofs/targeted-query-comparison.json)

<a name="canlı-tam-okuma-ve-codex-kod-kipi"></a>

#### Live full read and Codex Code Mode

18 November. The comparison tool gained the explicit name `read_archived_report` and shared instructions identifying the archive arguments. In the isolated ChatGPT Free run, two successful MCP calls delivered the original 5,926-byte report to the CLI exactly in the full-read arm. Four provider requests returned HTTP 200. The per-arm limit stopped the fifth before it reached the provider. The run did not reach a final answer and the candidate arm never started. Savings percentage and cost are unknown.

The initial diagnostic expected the top-level tool array and direct MCP names only. A local request using the same real model catalog verified that Responses Lite uses `input.additional_tools` to carry `functions.exec`. The diagnostic now distinguishes both forms and retains only fixed tool labels and event counts. The exact tool order, archive, and output are still checked against CLI events. The 14 focused tests, seven local engine requests, and one catalog rehearsal passed. [Evidence](../proofs/targeted-query-comparison.json)

<a name="kod-kipinde-istek-bütçesi-ve-canlı-kıyas"></a>

#### Request budget and live comparison in Code Mode

18 November. In Code Mode, the full read reached the CLI exactly. The engine turned the 5,926-byte text into a 33-byte CCR marker in the request to the provider. Both isolated engines in this experiment preserved `exec` and `functions.exec` output. The result reaching the provider was verified byte for byte on the real Codex CLI and pinned 0.44.0. Full read and targeted query each took three local requests including the final answer. Query plus a full read took four. The run passed 10 local Code Mode requests, 7 direct-tool regression requests, and 16 focused tests.

The local result supported keeping the live limit at four per arm and eight overall. On the same model and effort in the isolated ChatGPT Free profile, the 48-line task produced four requests and a correct final answer in each arm. Both first requests had zero cache hits. The control used 44,787 input and 282 output tokens. The query used 41,481 input and 229 output tokens. The total difference was 3,359 tokens (7.45%). The live candidate completed after the query without a full read. The local readback rehearsal does not establish live savings on tasks that require readback. Billing and general file and subagent permissions remain open. [Measurement](../proofs/targeted-query-comparison.json)

<a name="codex-dosya-ve-alt-ajan-yetkisi"></a>

#### Codex file and subagent permissions

18 November. The pinned Codex CLI and Responses Lite ran one main agent and one subagent with actual `spawn_agent` and `wait_agent` calls. At startup, the archive server bound to the fixed synthetic JSONL file in the permitted workspace. A path outside the workspace supplied in a tool call did not change the selected source. With sharing disabled, each agent retrieved exactly 66 bytes from its own archive. Cross-reads were denied both ways. The subagent was matched using its separate client thread and the local agent message in the main agent. The local provider completed 6 main-agent and 4 subagent requests.

With `mcp_connection_sharing` enabled on the same CLI and catalog, the subagent retrieved the main agent's archive exactly. The prototype's connection-level owner can therefore differ from the agent identity. Product acceptance requires a trusted agent identity or truly separate MCP connections. The run made zero model calls and covered MCP with one synthetic source. General native file permissions, protected Code Mode through the Headroom engine, and other supported clients require separate evidence. [Permission evidence](../proofs/targeted-query-comparison.json)

<a name="headroom-yolunda-ajan-sahipliği"></a>

#### Agent ownership through Headroom

18 November. The verifiable thread UUID in the pinned Codex CLI's stdio MCP calls matched the two identities at the 0.44.0 engine inlet. The `x-codex-turn-metadata` header was removed at the outlet. The isolated test matched inlet and outlet using body and thread digests. The prototype creates a separate archive per valid thread in MCP context. A missing, invalid, or duplicate thread field grants no archive authorization. Model arguments cannot name the archive owner.

The sharing-disabled control completed in 10 requests. The sharing-enabled control with an archive bound to one connection reproduced cross-agent data in six requests. With sharing still enabled, the thread-scoped prototype rejected both cross-reads over one MCP connection and 10 local requests. Six MCP calls matched the correct two inbound threads. Both permitted 66-byte outputs arrived intact. The 17 focused tests and seven-request normal query and readback regression passed. The run made zero model calls. Selective protection on large Code Mode results and general native file scope require their own acceptance. [Engine evidence](../proofs/targeted-query-comparison.json)

<a name="uzun-kod-kipi-çıktısında-seçici-koruma"></a>

#### Selective protection for long Code Mode output

18 November. In the local 0.44.0 trial, digests of calls and outputs from an authorized archive read were matched using the verified fixture thread and a unique call ID. Protection used the engine's exact Read path for that match only. The text of model-generated code by itself grants no authorization. Repeated versions of the same code in different call forms and multiple outputs invalidate the match. The local test verified that the same request carried the permitted 5,926 bytes exactly and that the other output was retrievable via CCR. A different thread and a changed call took normal compression.

Three synthetic engine requests, 18 focused tests, and the 10-request real Codex ownership regression passed. The run made zero model calls. The engine trial's call and output record was generated by the test driver using the fixed archive read. General file scope, a selective-protection record from an actual MCP read, and live cost acceptance remain open. [Evidence](../proofs/targeted-query-comparison.json)

<a name="gerçek-codex-mcp-okumasından-seçici-koruma"></a>

#### Selective protection from an actual Codex MCP read

18 November. The permitted call sent by the local fixture was matched with the output digest recorded after a successful archive `Read`. A match requires the same thread, a unique call ID, the archive range, and the original MCP envelope. The path supports real Codex's two-part Code Mode envelope. An unverified field added to the envelope receives no protection. The retrieval test restored the other result in the same request to its original text through normal CCR.

One MCP connection recorded two successful 5,926-byte reads and two denied cross-reads. Both authorized outputs stayed byte-exact at the engine outlet. The 10 local usage rows, 19 focused tests, three selective-protection engine regression requests, and seven query and readback requests passed. The run made zero model calls. The evidence covers one authorized synthetic file and an isolated local provider. General native file permissions and live cost including readback remain open. [Evidence](../proofs/targeted-query-comparison.json)

<a name="native-dosya-izinleri-ve-arşiv-geri-alımı"></a>

#### Native file permissions and archive retrieval

18 November. The pinned Codex CLI opened the permitted synthetic file in an isolated read-only sandbox. The archive received the 5,926 bytes actually returned by the sandbox. The target record, line count of 48, and original bytes were preserved. In a separate test, a Windows ACL denied reads of a newly created file. In the same run, Codex read the permitted file and got access denied on the ACL-protected file. The ACL was restored and the file bytes matched. The 20 focused tests passed.

The restricted Codex read profile stopped twice before the command started. The unelevated backend required an elevated sandbox for this profile. MXC was unavailable on this runner. The runs did not qualify as file denials. The general Codex profile boundary requires a suitable backend. Live cost including readback has separate acceptance. [Engine and permission evidence](../proofs/targeted-query-comparison.json) · [OpenAI Windows sandbox documentation](https://learn.chatgpt.com/docs/windows/windows-sandbox)

<a name="zorunlu-geri-okumanın-canlı-kullanım-kıyası"></a>

#### Live usage comparison with mandatory readback

Both arms use the same 48-record synthetic report, question, and exact JSON answer schema. The control reads the entire archive. The candidate queries it and then reads the same full archive. In Codex Code Mode the calls after receiving the reference run in order within one code execution. Acceptance requires a successful full read in the CLI, delivery of the same original 5,926 bytes to the provider, and two exact final answers.

Test the local pair first with the pinned CLI and engine. The live run uses one pair on the existing isolated ChatGPT Free profile. The model and effort match in both arms, the first request starts with zero cache hits, and the limits are four provider requests per arm and eight overall. Input, output, and cache-read counters must match every successful request. The result gives the provider token difference for the one matched task. Dollar amounts require per-request billing evidence.

Four requests in each arm matched in the live pair. Both JSON answers were correct, and both CLI readbacks were byte-exact. The original report was verified in the control's provider request. Exact byte equality could not be established in the candidate's provider-bound output. Both engines shut down normally and their CCR archives were empty. The recorded counters and tool events were insufficient to determine how the candidate was transmitted. The run produced no valid comparison or billing amount.

The local test showed that the preceding verifier missed verbatim plain text and JSON string forms. The verifier now checks both against the full report. Truncated content, tool errors, and calls that do not match are still invalid. The final local candidate passed 22 focused tests and six matched engine requests. The live result was recorded before the verifier change. [Evidence](../proofs/targeted-query-comparison.json)

The second live pair with the updated verifier also produced two correct answers and eight matched counter records. The control's original report was verified in the provider request. The candidate's full report was again verified at the CLI only. The second run's token difference and billing acceptance remained open.

Local reproduction showed that the old verifier could not recognize an original report if the query result and full read appeared one after the other in one code output. The current verifier matches both results to the same archive and originating call. It rejects a different archive and truncated content. The engine's JSON reading and the provider submission record byte matches separately for the same request. The full report was verified at both points in the last request of both arms of the local pair. The 23 focused tests, six local requests, and normal engine shutdown passed. The live candidate's output format remains uncertain. [Evidence](../proofs/targeted-query-comparison.json)

In the third live pair, the same 48-record report yielded the correct result in both arms. The original 5,926 bytes were verified in the same request at the engine inlet and in the output sent to the provider. Both first requests started with zero cache reads. All three control requests and four candidate requests matched provider usage records. Total input and output usage was 34,750 and 45,579 tokens, respectively for control and candidate. Query followed by mandatory full read used 10,829 more tokens (31.16%) on this task. Input read from cache was 10,112 and 30,464 tokens. Billing data was unavailable, so the result covers token usage for this one task. [Evidence](../proofs/targeted-query-comparison.json)

<a name="h041-e-deneysel-kurallar"></a>

### H041-e: Experimental Rules

User decision on 16 November 2026: the ModHarness-RL-inspired rule experiment is **optional and off by default**. The lead agent owns scope, measurement, and permission contracts. This record is only a plan. The goal is to select needed information from large tool outputs and show verified error loops as suggestions in Learn. [Paper v1](https://arxiv.org/abs/2611.18533v1) is the source for the method. The acceptance conditions here are specific to this product. Model training, generating and running code autonomously, and re-summarizing all chats are out of scope.

**User flow:** Settings → Learn → Experimental rules. A separate preference starts off. Opening Learn, starting the engine, installation, or an update does not enable it. Enabling the feature only makes this area available. The user selects one file or permitted scope → reviews a suggestion → Try locally → sees the previously accepted rule and connection/project scope → enables it. Preview and local trials do not change live traffic. No new top-level page, mandatory per-model/subagent choice, or general installation step is added.

**Work order and ownership:** First complete the normal H041-d local Chat Completions slice. Each rule's real-task and enablement acceptance depends on a usage record verified for its own protocol/route under H041-d. Acceptance for other inaccessible providers remains separately open and does not unnecessarily delay evidenced scope. Reuse Learn's existing file selection and local analysis path. Share data/evidence with the recommendation-benefit pilot only when fingerprints match. A targeted data-query rule also depends on acceptance of the existing safe targeted-query candidate. H041-e1 → e2 → e3 → e4 are sequential. No open substep counts as done. This document does not activate a package installation or product behavior.

1. **H041-e1 — Off by default and preview.** Verify that the separate preference is off on first open, update, or restart, or correctly retains its prior enabled value, and is independent of Learn and engine preferences. Analysis-only processing runs on the selected file or a clearly bounded app-owned record. An invalid or cancelled file does not change settings or traffic. Raw content, secrets, and private reasoning are not copied into persistent reports. Local samples are managed with clear retention, size, and deletion limits. There is no hidden background scan, telemetry, or automatic provider transmission.
2. **H041-e2 — Bounded local rule and replay.** Pre-reviewed local functions process only permitted inputs. Log/model text is not executable code. No shell, network, or arbitrary file access is granted. Separate normal, error, and boundary samples preserve stderr/stdout/exit, exact counts, complete source/diff reads, repeat count, and tool/call_id. Original bytes can be recovered from app-owned storage with bounded access and lifetime. Tests required after a current-source change, normal polling, legitimate retries, and parallel agents do not produce false loop warnings. Re-sending old history in a later request does not count as a new tool call. Unclear call/session ownership produces an insufficient-evidence result. Local replay does not stand in for real model-task success.
3. **H041-e3 — Independent real-task acceptance.** Run a control/rule A/B with the same model, reasoning, permissions, tools, and environment. Fix tasks not used to design the rule, expected results, at least three matched repetitions, cold/warm cache state, savings and latency/CPU/RAM thresholds before the experiment. All specified correctness tests must pass. Lost output, false success, or expanded permissions rejects the rule. Include primary use, CCR/retrieval, retries, extra model turns, and any recommendation-generation use. Show cache reads/writes, output, and non-cache use separately from the local tokenizer. Do not start a new paid resource, account, GPU training, or autonomous model call. If real-provider quality or total usage cannot be measured, the relevant acceptance remains open/not_tested. Do not invent net savings. Do not generalize measured task quality to all tasks.
4. **H041-e4 — Scoped enablement and rollback.** Apply only an accepted rule version to a selected, actually supported connection/project scope, preserving client permissions. Separately verify the UI → manager → real engine/client path and its states. Do not change model/auth/reasoning or agent count. Test different main/subagent models and model changes. A model name is not agent identity or a support allowlist. For an unknown combination, keep the rule inactive and continue the existing base-engine path. Acceptance is bound to the rule/client/protocol/tool/source fingerprint. A relevant change or conflicting evidence requires reevaluation. Enablement pins a new session/context version and does not rewrite already-sent history/prefix. Disabling stops new transformations and preserves ongoing streams and valid access to old originals/CCR records. Preferences and rollback persist with ownership, atomic writes, and conflict protection. A corrupt/stale rule record stays inactive with a warning. It does not claim the base engine configuration or other projects. Project instructions/AGENTS or client settings are never written autonomously.

**Three acceptance flows:** (a) start with the feature off → the user previews/trials a local file → live route and base preferences remain the same, with only a bounded local result; (b) an incorrect/over-budget rule or one that loses error/numeric data → reject it while preserving original output/failure and the base path; (c) accepted scope → explicit enablement/new session → real rule path, then disable/restart or change a fingerprint → correct inactive state while preserving ongoing streams and access to old rollback data.

**Completion gate:** e1–e4 close with concrete evidence and before/after fingerprints for the relevant source/test/config and tool/environment. Verify the real current interface/command in each slice. Run the [WORKFLOW](../development/WORKFLOW.md) quality gate and required native flow once on the final product candidate. A mock/local-filter PASS is not live-provider or other-OS acceptance. A plan-only update requires no product test or new benchmark. Main H041 remains open. The current slice is e3 applicability and real-task acceptance.

<a name="h041-e1-uygulama-kabulü"></a>

#### H041-e1 implementation acceptance

Persistence and file access are lead-owned. A separate toggle under Settings → Learn only opens file selection for local rule candidates. Existing Learn file selection remains independent. Engine startup and memory preferences do not use this setting. `experimental_rules` remains in the preference response. A missing record reads as off. Saving language, theme, operating preferences, or local recommendations preserves the current choice. [Shared preference format for returning](#geri-dönüşte-ortak-tercih-biçimi).

- With the preference enabled, one selected Codex/Claude Code JSONL file uses the existing bounded analysis path. The result is held on screen only. For experimental requests, the native command checks the preference again before opening the picker and before returning the result. A disabled or corrupt preference does not open file selection.
- Cancelling file selection preserves the previous result. An invalid file returns a fixed error. Closing the view or changing pages does not show the previous trial as the visible result. Test opening/closing and reopening together with the actual engine state in a real Windows window.
- Verify the legacy record, reopen, concurrent old revision, and independent preferences with focused tests. On the final candidate, require the local quality gate, the real Tauri IPC → native picker → pinned Python → visible suggestion path, and an accessibility check. Enablement and replay belong to e2–e4 acceptance.

H041-e1 passed local Windows 10 acceptance. The off-by-default persistent preference and 24 native checks passed. If another save request disabled the preference while file selection was open, the result was rejected. Codex/Claude preview, cancellation, malformed/empty files, reopening with the preference on/off, and independence from the engine/other settings passed. The record reports 224 frontend tests, 468 Rust tests, two builds, and two accessibility checks with zero violations. [Local acceptance](../proofs/experimental-rules-preview.json).

<a name="h041-e2-uygulama-kabulü"></a>

#### H041-e2 implementation acceptance

The lead agent owns original data and native call scope. The first rule is `repeated_log_lines_v1`, using the fixed engine's line-repeat transform and inverse. Up to 16 MiB of original bytes from the same selected file is held as one app-memory record, accessible for 15 minutes. The record ID is tied to the main window and application lifetime. A new successful preview or clearing the result in the UI releases the previous record. Cancelling file selection preserves the previous result. The UI releases the record on expiry, and native access also checks its lifetime and cleans up an old record. No raw archive is written to disk.

- Try locally after preview operates on the same snapshot. The native path checks the preference, owning window, record ID, and lifetime. Only the snapshot is sent to Python. Execution is limited to 15 seconds and 64,000 input lines. No code embedded in model/log text is executed. The result contains only counts and source fingerprint verification.
- Preserve complete Read/source/diff and exact numeric data, stderr, and output with failed or ambiguous exit/channel information verbatim. Only pure log lines on the explicit stdout channel of a verified successful command are candidates. A candidate fails if the inverse transform is not equal to the original text. Preserve existing compression markers and budget overruns in the text. Test repeat counts and call_id mapping. Unchanged historical repeats do not count. Conflicting calls or ambiguous scope are reported as insufficient evidence.
- Test normal polling, legitimate repeats, intervening edits, and parallel-agent cases in Learn regression tests. Native archive ownership, capacity, lifetime, and cleanup get focused tests. Verify the final candidate through the local quality gate and the real Tauri → same in-memory snapshot → local process → visible trial result path. Measurement is limited to tool-output bytes. Task quality and provider tokens belong to e3 acceptance.

H041-e2 passed local Windows 10 acceptance. Six Learn tests covered normal/error/boundary input, and archive tests covered scope/capacity/lifetime/cleanup. In 17 real-window checks, the original snapshot, byte-exact rollback, new ID, and disabled-preference boundary were preserved despite file changes. One eligible stdout sample went from 18,500 → 99 bytes. This measures output bytes. Live token savings and task quality require separate evidence. The record reports 224 frontend and 469 Rust tests, two builds, and two accessibility scans with zero violations. Source/Python/toolchain fingerprints matched before and after. [Local acceptance](../proofs/experimental-rules-replay.json).

<a name="h041-e3-istemci-uygulanabilirlik-kapısı"></a>

#### H041-e3 client applicability gate

Before using provider quota, the lead agent tests whether the rule applies to real client output. `bench/optimization/rules-client-plan.json` fixes normal stdout, a result mixed with an error, and a client-truncated result across three repetitions. `rules_client.py` runs the installed CLI with a new auth-free `CODEX_HOME` and read-only Windows sandbox. Only the owned loopback protocol fixture returns responses. No real model runs. Compare arguments with the tool schema from the first real request. Check exact values and before/after fingerprint of the file the CLI actually reads. A temporary record linked by tool/call_id and CLI thread ID feeds the existing e2 trial. Fixture usage counters do not count as provider usage. Raw wire content is not exported. Only this run's CLI private records are cleaned up when the process exits.

The 16 November Windows 10 / CLI `0.169.0-alpha.11.1` run passed **9/9 and 18 local sends**. Six complete outputs preserved 100 log lines each and exact integers. Three truncated outputs contained 9 log lines each. All results were strings on the wire, and error information was the same text. e2 preserved every result, leaving zero eligible outputs. This shows the e2 safety contract was passive on this route. It is not support acceptance for Desktop, another client, or every CLI version. Client truncation found during preparation is a separate boundary sample in the final run. It is not Headroom savings. [Evidence](../proofs/rules-client-applicability.json).

<a name="h041-e3-kontrollü-üretici-ve-yerel-karşılaştırma"></a>

#### H041-e3 controlled producer and local comparison

`bench/optimization/rules_producer.py` uses the existing test producer. The fixed command's separate stdout, stderr, and exit code are held in memory with the tool and call ID. Another owner cannot read the record. It uses e2's real eligibility check and the engine's fixed inverse transform. Both arms start from the same output and use the same 0.44.0 `coding` profile. The rule applies only to the eligible stdout field in the experiment arm. Count the first request, tool result, and follow-up request in the same session for each arm. [Conditions](../../bench/optimization/rules-producer-plan.json).

The first preparation run showed the base engine placing a CCR reference in stdout. Based on this observation, local retrieval was validated separately and a scenario with one real engine continuation request was added. The final run passed with three new engines, five scenarios, 15 matched comparisons, 90 client requests, and 93 loopback sends. 36 local CCR retrievals and the original stdout in three continuation requests were verified byte-for-byte. Engines closed normally and released their owned ports.

- The log sample did not need retrieval and contained **586 → 752** local input tokens. The experimental rule increased input by 28.3%. In the experiment arm, stdout fell from 4,000 → 261 bytes, and the original line counts and exact values were recovered.
- A sample needing a CCR continuation request contained **2,808 → 1,061** local input tokens with the same retrieval-tool description in both arms. A 62.2% reduction was observed. The base arm's actual continuation body is included in the total. The client used SSE. For this sample the engine used upstream JSON.
- In successful-command stderr warning, nonzero exit, and exact numeric `Read` samples, the experimental rule made no changes. Matched send counts and local token measurements were identical in every repetition. Measurement used `o200k_base` over all observed upstream input JSON. [Evidence](../proofs/rules-producer-comparison.json).

The rule did not meet the overall additional-benefit threshold. Keep the local-preview scope. For a live pilot, evaluate tasks that do and do not actually need retrieval separately. Bind the comparison to a limited route that can verify the client's original channels and call scope. Fix matched tasks, exact expected results, cache conditions, resource limits, and usage limit before the first real request. Standard CLI combined outputs remain out of scope as in the earlier finding. Real task quality and measurement of total provider usage remain open in H041-e3.

<a name="h041-e3-izole-mcp-yolu-ve-responses-kullanım-düzeltmesi"></a>

#### H041-e3 isolated MCP route and Responses usage correction

`bench/optimization/rules_pilot.py` connects the installed Codex CLI, in a new auth-free profile, to the owned 0.44.0 engine. The read-only MCP tool does not accept a command or path from the model. A fixed subprocess produces stdout, stderr, and exit code separately. Matched arms use the same tool, model label, and effort. Expected replies for the exact-count and failed-command cases are fixed in test code. The far end of the stream produces a local reply. Real task quality and provider usage remain open.

In the first run, eight requests across four CLI sessions appeared in `/stats`. The private usage record was empty for all eight. Comparison with delayed `[DONE]` showed that a record was created when the client read to the end of the stream, but was missing when the client closed after `response.completed`. The engine's real request ID was matched before cleanup. For Responses HTTP/SSE, covered counters are retained after cleanup cancellation using the upstream response ID and verified completion data. Incomplete completion, conflicting IDs, and records truncated by the budget are excluded from this special path. The contract remains that totals are unknown for failed replay or incomplete counters.

The final candidate matched all **10/10 requests** to usage records across two closure types and four CLI sessions. The JSON reply and call ID, unknown status for a missing cache-write counter, and three CCR retrievals were verified. Eight existing Responses scenarios and the native reader → SQLite → request-details path also passed. [Evidence](../proofs/responses-terminal-usage.json).

For the first matched live e3 pair, use the same read-only tool and engine settings with the model/effort selected by the user and the existing authorized account. Verify normal exit for both arms, fixed-answer checks, and every real request ID. If this pre-acceptance passes, measure independent tasks with at least three pairs each in cold and warm conditions. Document cache state with provider counters. Include primary and continuation requests plus retries. Do not produce a summary with missing counters, and do not treat the first run as general enablement acceptance. Preserve the overall-benefit threshold and e4 order.

For the first pair, `bench/optimization/rules_live.py` opens two temporary CLI turns and two new engine processes in the same empty Git workspace. The fixed counting task tests raw and rule-applied stdout while preserving error and exit fields. If a Headroom reference appears in the control arm, the read-only test tool retrieves verified task content only from that engine. Match request scope using distinct engine IDs, usage records, and outgoing HTTP-send counters.

The default command uses local replies only. A live command requires `--live`, selected `--model` and `--effort`, and the separate Headroom test profile's `--codex-home` path. Verify model and effort against that profile's Codex catalog. The pair has a total limit of eight Responses HTTP sends. The control arm reserves at least two sends for the compressed arm. Each CLI turn is limited to 120 seconds. On retry, `--prior-attempts` counts previous sends toward the total limit. Stop before starting the engine when fewer than two sends remain for each arm. Request bodies are limited to 256 KiB. The runner sends only to the specified endpoint with the same model/effort. Redirects are not followed. Rejection records retain the HTTP status and fixed reason tags. The live pilot records total input and output, cache reads and optional writes, and extra sends. If both arms pass, calculate the percentage from the difference in total input and output. The first pair runs control → rule, and provider counters are used to observe cache state. Independent repetitions are required for the production-benefit threshold.

In the 17 November live Luna pair, both turns completed normally. The control used four provider HTTP sends and the compressed arm used three. The control completed retrieval. Neither arm passed the exact-output check, and both usage totals remained unknown. The compressed arm's final message parsed as JSON. Sensitive-boundary and coefficient checks matched, but two count fields did not. If the normally completed control arm fails quality or usage acceptance, still collect evidence from the other arm. Both-arm conditions remain required for acceptance and a savings percentage. [Live pair](../proofs/rules-live-luna-pair.json).

`ResponseCompletedEvent` carries the completed response. The pinned OpenAI schema defines the inner `status` field as optional. The Responses HTTP/SSE reader accepts a missing inner status field when same-ID `response.created` and `response.completed` events exist, including when the status is `null`. An explicitly conflicting status, ID mismatch, or stream without usage remains unknown. The local terminal-event and early-close test passed. Live validation remains open because the remaining live records did not contain the terminal field. [Local validation](../proofs/responses-optional-terminal-status.json).

One live Luna diagnostic request had a completion event with `status=completed` and counters of 9,386 input and 8 output. The usage reader skipped the response because its content type was not classified as standard SSE. Also inspect events from a successful Responses response sent with `stream=True`. Determine completion and identity from stream content. Truncated, conflicting, or counter-incomplete streams remain unknown. No raw stream was saved. Local reconstruction uses live counters and event metadata. [Content-type validation](../proofs/responses-stream-content-type.json).

The pilot output contract requires integers for `passed` and `held`, and strings for `exact_limit` and `signed_factor`. The earlier generic string instruction conflicted with this. Both arms now use the same explicit instruction and same `--output-schema` file. The schema contains no expected numeric answers. Evaluate the final assistant message exactly. Intermediate progress messages count toward usage totals. A type mismatch is not converted into the correct value for acceptance. The previous live record retained only field matches, so it cannot show whether the two counts were numerically wrong. [Response-contract validation](../proofs/rules-answer-contract.json).

In the new live Luna pair, counters were observed for all five control and three compression sends. Total input and output was 49,826 → 29,592. The compressed final answer was exactly correct. The four values from the raw arm did not match. This pair's quality acceptance was negative. Equality of CCR-record stdout with the original and successful local retrieval were verified. The record did not prove the integrity of the retrieval body sent to the model. Independent repetitions, quality, and overall-benefit acceptance remain open. [Live pair with measured counters](../proofs/rules-live-counted-pair.json).

The pilot's retrieval diagnostic tracks separate match flags for the result returned by the fixed producer to the CLI and the result sent by the engine to the provider. Control is limited to the pilot's tool names and same call ID. Content, the model-selected hash, and error text are not recorded. A completed call with an explicit MCP error is not treated as a successful tool result. In a synthetic ChatGPT-route test with the real CLI and local engine, both ends of retrieval matched. This local observation does not establish the cause of the previous live response error. [Retrieval delivery](../proofs/rules-recovery-delivery.json).

The catalog for the next diagnostic live run used Luna `code_mode_only`. The pinned engine's `unwrap_tool_call_name` policy, when called inside `exec` or `functions.exec`, preserves recovered content from `headroom_retrieve`. The pilot's `recover_result` name did not take this path. In a local Responses `custom_tool_call` scenario, stdout using the old name was offloaded again, while output using the canonical name was preserved exactly. The pilot was changed to `headroom_retrieve`. The diagnostic uses the same engine name resolution and compares fixed content in code-mode outputs, both plain and JSON-wrapped. The live final-delivery body was not retained, so the relation between the local finding and the previous response error is not fully proven. [Code-mode validation](../proofs/rules-code-mode-recovery.json).

The next live pair using the canonical name completed 7/8 sends. Three producer results matched in the CLI and in the full retrieved result sent to the provider. Retrieval was not offloaded again. All 7/7 provider usage records were complete. The control used 40,716 total tokens and the compressed arm used 29,548. Only the control's `passed` count failed to match. The compressed answer was fully correct. The pair's quality acceptance was negative, so there is no accepted savings percentage. Observed cache counters are not a controlled cold/warm experiment. The shared instructions for both arms were updated to explicitly count `check=pass` and `check=held` records programmatically and aggregate reserved repeat groups. Exact answers and tolerances remain unchanged, and expected values are not given in the instruction. Nine focused tests and a local CLI/ChatGPT route check with five sends passed. Live correctness of the new instruction, independent repetitions, and the overall-benefit gate remain open. [Canonical-name live pair](../proofs/rules-live-canonical-pair.json).

In the live pair using programmatic counting instructions, both final answers were exactly correct and all 8/8 usage records matched. The control used two retrievals and 52,563 total tokens. The compressed arm used 29,611. The observed difference in this single sequential pair was 43.6657%. It is not overall savings acceptance because cache conditions were uncontrolled and there were no independent repetitions. The old runner rejected the control because the tool sequence was limited to one retrieval. The plan requires totals for primary, retrieval, retries, and extra turns. It does not require exactly one retrieval. The auditor accepts read-only repetitions after one fixed producer and preserves server ownership, completion, and explicit-error checks separately. Ten focused tests and a local CLI/ChatGPT route rehearsal with five sends passed. The previous live record did not separately retain tool-state/ownership checks, so it was not retroactively made a PASS. Live acceptance for the corrected control remains open. Model/effort/permissions, exact answer, and source boundaries are preserved. [Programmatic counts and run acceptance](../proofs/rules-live-programmatic-counts.json).

<a name="h041-e3-canlı-ön-kabul-ve-bağımsız-görev-planı"></a>

#### H041-e3 live pre-acceptance and independent-task plan

With the corrected auditor, both answers in the new live pair were correct, tool ordering/ownership/completion/error checks were positive, and all 8/8 usage records were complete. Including one retrieval, total tokens were 40,720 → 40,452, an observed 0.6582% reduction in one pair. Cache reads were 19,072 / 28,800, and writes were zero in both arms. Both engines closed normally, and before/after fingerprints matched. This pre-acceptance does not close independent quality, overall savings, or CPU/RAM gates. [Pilot acceptance](../proofs/rules-live-pilot-acceptance.json).

[Independent data and plan](../../bench/optimization/rules-heldout-plan.json) defines three controlled tasks not used to design the fixed rule: build-log summary, failed process, and sensitive-source read. Original stdout/stderr/exit and Read, exact answers, type schemas, and source fingerprints are fixed. During local preparation, oracles were checked against original inputs. Log inversion was exact, and error and source were preserved. These are fixed tool outputs, not real-project build or live-task acceptance. `source_arguments` contains only captured tool metadata. No command strings or source text are executed. The existing fixed stdio producer is used.

Three matched repetitions are needed for each task and cache starting state, with 18 pairs planned. Order alternates by repetition as control/rule → rule/control → control/rule. For the first completed upstream request in both arms, zero cache reads counts as observed cold and a positive value as observed warm. All later read/write and output counters remain in the totals. Warm means only partial prefix use was observed. Mixed, missing, or unestablished conditions are recorded as open. There is no silent PASS, dropping samples, or automatic extra run. The plan grants no new live-call authority. The batch limit is eight sends, with no automatic next batch.

Acceptance requires all exact answers and tool scopes to pass, and at least 5% input+output token reduction across all required tasks/repetitions in each cache group. Include all CCR/retrieval, retries, and extra turns. The p95 of matched completed-task duration differences must be at most 1,000 ms. The rule arm's completed-task local CPU p95 must be at most 2,000 ms. Total private memory observed for engine processes at 50 ms intervals must be at most 1,536 MiB. CPU includes the existing owned harness/engine processes and transformation. It excludes CLI/provider CPU. Duration includes the full CLI turn and provider wait and is not presented as engine-only added latency. Apply `replay.percentile` using nearest-rank separately to each cache group. These numeric thresholds are fixed before results. The new scope is completed tasks rather than individual requests. The independent runner has not yet been implemented. Live quality, usage, and resource measurement remains open.

<a name="h041-e3-bağımsız-koşucu-ve-çoklu-tekrar-özeti"></a>

#### H041-e3 independent runner and multi-repeat summary

`rules_live.py` runs the plan's three independent tasks using `--case`, zero-based `--repeat-index`, and `--cache-start`. The task-specific schema is the same in both arms, with no change to input or numeric acceptance. The second repetition runs rule → control. The first arm's name does not change budget allocation. Each command runs only one pair and sends at most eight requests. If the requested starting cache condition cannot be verified, stop before opening the remaining arm and retain usage and resource data up to that point. Record timeouts as failed/partial turns. `first_upstream_cache_read_input_tokens` comes from the first upstream attempt in the engine request sequence. Do not replace the initial-condition value with a later retry/cache value.

Resource measurement uses the existing Win32 sampler for the main harness and owned engine processes. Rule preparation CPU in the MCP producer is added in a separate bounded numeric record. CPU for other MCP starts/tools, the CLI, and the remote provider is not measured. No total-system CPU claim is made. Missing or invalid counters mean the resource data is incomplete. The `passed` value for a single pair that passed the schema/output/protocol rehearsal is not general performance acceptance for 18 pairs. The local failed-process example exceeded CPU and additional-time numeric thresholds. This did not change the acceptance thresholds.

`rules_summary.py` scans all independent records for the same provider/model/effort and has no option to select only successful files. It checks before/after and current source/tool fingerprints, coverage and closure of both arms, exact answers, all provider counters, and cache and resource details. Missing, duplicate, failed, or changed input cannot produce acceptance. If the full matrix is present, calculate total input+output, 5% net savings, nearest-rank p95 duration/CPU, and peak private memory in each cache group. Do not report an overall percentage with missing data. The summary command starts no client or model. A local fixture is not live acceptance.

Local acceptance passed with 22 focused tests, four real CLI/local-engine pairs, and 22/22 usage records. A forced cache mismatch stopped before opening the second arm after three sends. The independent fixture reports a missing cache-write counter as explicit zero. The previous missing-field attempt and the product's unknown behavior were preserved. Plan data and thresholds did not change. [Runner and summary evidence](../proofs/rules-heldout-runner.json). No independent real-model request was made. The 18 live pairs and overall-benefit and resource acceptance remain open.

Local single-pair and read-only live-summary commands run from the project root using the pinned Python. They use two loopbacks unless `--live` and explicit current test-profile parameters are supplied. With incomplete data, the summary is expected to return exit code 1.

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/optimization/rules_live.py --codex '<sabit-codex.exe-yolu>' --case build_manifest_v1 --repeat-index 0 --cache-start cold
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/optimization/rules_summary.py --provider chatgpt --model gpt-6-luna --effort medium
```

<a name="h041-e3-ilk-bağımsız-canlı-kalite-reddi"></a>

#### H041-e3 first independent live-quality rejection

The matched `build_manifest_v1` / repetition 0 / cold run for the same selected `gpt-6-luna` / `medium` pair completed in eight sends. Both first upstream requests confirmed cold status with zero cache reads. The control returned the correct answer. The rule arm failed the build and warning counts. Artifact/digest and types were correct. All 8/8 usage records, tool scopes, normal closure, and before/after fingerprints were complete. Total tokens of 40,245 → 40,154 were observed. No savings acceptance was produced because the answer was wrong. The summary rejected the run because it contained one failed record and 17 missing pairs. Under the plan's requirement to pass all correctness checks, this rule candidate is not eligible for live enablement. The remaining pairs are not run automatically. [Evidence](../proofs/rules-heldout-first-live.json).

In pinned local `collapse_runs` / `expand_runs` replay, 1,940 bytes of original output were preserved exactly. Because the model-written counting code was not retained, the exact failure mechanism is unknown. Byte-level invertibility of the transformation does not mean the model counted the task correctly. This rejection is not an incompatibility claim for the entire Headroom engine or other models. It applies to the measured optional rule/route/model/effort/task scope.

The second demonstration candidate in `rules_runs_json.py` is only a local prototype. Under the `headroom.runs.json.v1` frame, the protocol carries the original line and the integer `occurrences` total as separate fields. It does not execute code or instructions. The pure encoder/decoder is limited to 16 MiB and 64,000 lines. This is not a product rule record or engine hook. Three tests passed for local calculations covering CRLF/LF/Unicode, empty text, an original line that resembles a marker, invalid types/fields, expansion limit, and explicit counts. The fixed example used 392 bytes and round-tripped exactly. Real model quality or token savings were not measured. The old rule, data, and thresholds were unchanged.

This prototype was designed after observing the existing independent sample fail. That same sample does not count as independent acceptance data for the new candidate. The next local scope is to bind the existing eligible stdout, stderr/exit, and Read protections to the new demonstration. Fix independent tasks and source/cache conditions before real quality/enablement acceptance. Preserve the failed old record. Do not relax acceptance tolerances or retroactively PASS the earlier matrix.

<a name="h041-e3-json-adayı-için-yerel-kapsam-ve-yeni-görevler"></a>

#### H041-e3 local scope and new tasks for the JSON candidate

`trial_capture` requires an explicit expected owner and call_id for one locally captured call. Reuse the existing `local_replay`, `read_transcript`, and `replay_items` validation. Do not add a separate general transcript parser or loosen eligibility policy. Reject multiple/mixed historical calls, conflicts, and ambiguous ownership. Do not modify failed results or results with ambiguous stdout/stderr separation, Read/numeric source, or an explicit error flag. Only eligible successful stdout can become a JSON candidate, with exact equality of the complete output/metadata inverse transform. Preserve original stdout instead of growing output. Do not execute command metadata or source text. This remains a benchmark prototype, not a product/native/UI path.

Local data acceptance passed on three new tasks prepared after seven focused checks and implementation were fixed. [Evidence](../proofs/rules-json-guard.json). The queue/receipt example showed 1,527 → 358 stdout bytes, and output was preserved exactly in error and sensitive-source examples. These are not provider tokens or real model responses. [Separate plan](../../bench/optimization/rules-json-heldout-plan.json) with the `explicit_runs_json_v1` candidate excludes all previous development and failed tasks. Expected answers were verified against original inputs and are not included in schemas/instructions. Source/test/tool fingerprints are fixed. The old failed marker candidate and plan are preserved.

The new plan carries over the old plan's 18 pairs, alternating arm order, cache classification, and acceptance thresholds exactly: 5% savings / 1,000 ms added p95 task duration / 2,000 ms local p95 task CPU / 1,536 MiB observed private engine memory. There is no automatic call, warm-up, or new batch. The existing test account/model/effort/permissions are not changed. Do not run a live trial until the new candidate's CLI adapter, raw-original retrieval path, cache conditions, resource limits, and request budgets, and separate rejection records are first verified locally. Live quality, usage, and resource acceptance for the new candidate is `not_tested`. The e3/e4 order and quality rejection are preserved.

<a name="h041-e3-json-cli-adaptörünün-yerel-kabulü"></a>

#### H041-e3 local acceptance for the JSON CLI adapter

The new candidate's three tasks were connected to the same isolated CLI/owned-engine runner. `case_details` and `case_plan` distinguish JSON data/schema from the old marker plan. The JSON instruction is identical in both arms. The existing model/effort and eight-send limit are preserved. The report carries `candidate_id`. The summary's `--candidate` selection prevents an old rejection record from being assigned to the new candidate. The old default pilot path was also locally revalidated.

If a JSON candidate produces a CCR reference, the test tool first verifies that the same owned engine returns the frame exactly, then returns the raw stdout/stderr/exit envelope from before JSON transformation. No content assumption is made about another tool/call. Producer and upstream diagnostics have separate original envelopes for their expected retrieval. No content/hash value is added to diagnostic lines. In the controlled required scenario, a fixed JSON frame was placed in the test engine's own SQLite database. The real CLI → MCP → engine retrieval → raw result → upstream path passed. This scenario does not prove the default retrieval-trigger threshold.

Local validation found a measurement error in the first arm. The fake provider imported `_openai_responses_to_sse` on the first upstream request. Thread CPU/stack metadata showed approximately 3,438 ms CPU and the import path on the first request thread. This setup now uses `prepare_provider` before timing/CPU sampling for both arms. Setup CPU/time is reported separately under `fixture_setup`. The record accounts for the setup cost. The test sends no request to warm the model cache. Fixture/engine/rule work during the request remains within the prior scope. Completed-task and setup phases remain separate, and numeric thresholds are unchanged. In the same error example, the local rule-arm CPU measurement of 4,062.5 ms was 281.25 ms in the final candidate, with separate fixture setup of 3,328.125 ms. This is not a speedup claim for the product engine.

The final candidate passed 35 focused tests and 31 local upstream Responses sends: four CLI pairs with 22 sends, six raw-retrieval sends, and three cache-stop sends. A separate evidence record preserves 31 sends before the fix and five diagnostic sends. Normal closure, source/environment fingerprints, correct schema/answers, and all usage records were verified. Short local observations were within fixed resource limits, but full live-matrix acceptance was not granted. [Evidence](../proofs/rules-json-cli-adapter.json). Real model calls: zero. The JSON candidate has zero records and 18 missing pairs in the real-provider summary. The old marker candidate's preserved rejection record was verified. The next step is a limited first JSON live pair with the same selected account/model/effort. No automatic additional batch.

The new candidate's read-only live-summary command is:

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/optimization/rules_summary.py --candidate explicit_runs_json_v1 --provider chatgpt --model gpt-6-luna --effort medium
```

<a name="h041-e3-sınırlı-canlı-json-denemesi-ve-roi-durması"></a>

#### H041-e3 limited live JSON trial and ROI stop

On 17 November, the user left the Gemini evaluation to the lead agent's decision. Before the first real pair, an additional work decision was written to `registration.json` without changing code/data/thresholds or general acceptance. A quality failure means rejection. A missing turn/counter/cache condition means stop. A valid first total-usage signal below 5% is a reason to freeze optional rule R&D. A signal above 5% does not replace the full matrix. There is no automatic retry, batch, or third-candidate development. [Review](../research/optimization-roi-review-2026-11-17.md).

Five control and three candidate upstream sends were made using the same selected test account/model/effort and task. The control completed correctly. The candidate reached the eight-send limit before finishing its final turn. The CLI's visible record showed two completed `inspect_result` calls for the candidate, with no final count/answer. The candidate total and the pair's cold condition are unknown. They are not assigned incorrect arithmetic or a negative overall-savings label. Tool IDs and raw counting code were not retained, so no more detailed cause is inferred from these event counts. This optional arm was frozen under the previously registered stop condition for missing measurement. e3/e4 are not complete. [Evidence](../proofs/rules-json-live-pilot.json).

The observation layer recorded only real integers in `sent`/`retry` fields from 0–64,000 for this fixed task, along with sequence/status/error flags for known normal tools. Boolean/overflow values and arbitrary strings were discarded. Private reasoning, prompts, and raw model code were not retained. Two synthetic projection checks and one real normal-tool record were inspected. Model request/instruction/budget were not changed. Input/output/cache counters matched exactly for the control's five completed requests. Usage remained unknown for the candidate's rejected extra ingress row. Both engines closed normally, and source/tool fingerprints matched. The existing 35 tests were not rerun because product/test source did not change.

The live-provider slice under H041-d was verified for Windows 10 / pinned 0.44.0 / CLI 0.169.0-alpha.11.1 / selected ChatGPT account-model-effort / Responses HTTP-SSE. This is not JSON-benefit acceptance, API-key, WebSocket, Chat, another account/model, or another OS acceptance. Other boxes and main H041 remain open. The independent safe-targeted-query client/permission pilot under the same main task is next priority. Do not proceed to dependent e4 of frozen e3. This grants no authority to close the main task or publish.

<a name="h041-b-yerel-deney-koşulları"></a>

### H041-b local experiment conditions

The lead agent owns the experiment and acceptance because they involve measurement, runtime, and caching contracts. `bench/optimization/ccr-policy-plan.json` defines two paired repeats in advance. Each repeat uses two separate engines. One session consists of three short requests. The second has three short requests followed by a compressible long log and a protected exact numeric Read result. A short request in a separate clean session checks that the sticky tool does not carry over to another session. All requests go to an isolated Anthropic SSE fixture.

The short `lazy` arm must match the original body and reduce the existing tool overhead in each repeat. The long arm requires the same message prefix, model, effort, auth, and tool identity, exact log retrieval, and protected Read bytes. The tool definition is added once at first compression and stays in place for subsequent turns. The engines exit normally and close their own ports. Startup and request wait limits match the previous fixture. The measurement counts local full request JSON. Live provider caching and model answer quality have separate acceptance. Changing the product default requires evidence of multi-turn usage and task success.

The 15 November local run passed. In a short session, `lazy` sent 300 fewer tokens across three turns. In a long session the first log compression changed the tool array under `lazy`, while `eager` kept it stable. Anthropic identifies a tool change as a cache-invalidation event. The default stays `eager`. Revisit the decision after measuring cache reads and writes in live use and net tokens per completed task. [Local evidence](../proofs/anthropic-ccr-policy.json) · [Claude caching rules](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#what-invalidates-the-cache).

<a name="h041-c-kullanım-ölçümünün-kaynağı"></a>

### H041-c source of usage measurement

The lead agent verifies the measurement source and request ownership. The first slice covers Anthropic Messages on the pinned 0.41.0 engine. The client requests SSE. Once the long log is compressed, the local provider fixture calls `headroom_retrieve`. Match the engine's primary request and the follow-up carrying the original log using the synthetic request ID controlled in this experiment only. Use the same model across three separate sessions to test a normal result, a missing usage field, and a failed follow-up. The preceding short turns establish the same history. The run measures usage coverage, with model quality and live provider billing requiring their own evidence.

Sum `input_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`, and `output_tokens` in separate fields across upstream responses. Total input is the sum of non-cache input and both cache fields. Count a zero only when observed. A counter stays unknown if the upstream response omits it, is invalid, or fails. Compare the SSE client response and private `/stats` row with usage sent by the fixture for every upstream call. Verify the model, reasoning, synthetic auth, follow-up correlation, the original log, and owned shutdown. The record retains only the tested counts and states. [Claude usage fields](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#tracking-cache-performance).

If coverage or session ownership is incomplete, the usage source for the product's net savings display remains unknown. In the 15 November run, the client SSE and the engine's private `/stats` record reported only the last upstream response after successful CCR. When the follow-up failed, `/stats` included the primary call and the client received 502. These surfaces provide incomplete usage coverage. [Local evidence](../proofs/anthropic-usage-coverage.json).

For H041-d, capture usage where every upstream response can be attached to the same internal request while its usage fields are still available. Store only numbers and call state. Until per-record coverage is established, matching uses no model or timestamp similarity. Keep missing data, conflicts, and incomplete calls explicit. Verify OpenAI and other protocol usage fields on their own surfaces.

<a name="h041-d-motorun-anthropic-kullanım-kaydı"></a>

### H041-d Anthropic usage records in the engine

The lead agent owns request scope, version pins, and usage calculation. In the first product slice on Windows 10, the engine-owned HTTP client observes the primary submission, CCR follow-up, and retries within the same Anthropic Messages request. It ties the engine-issued request ID to the completed ASGI scope. The four explicitly reported JSON and SSE counters are recorded. SSE measurement does not consume the stream ahead of delivery. If the final stop marker or a verified field is missing, the total stays unknown. Processing uses bounded memory. The usage record excludes request content, responses, and authorization data.

Read recent records through the local endpoint protected by the current engine control key. The successful example must reach 2,429. A missing cache write or a 429 and retry must never produce a persistent zero. Verify call counts and engine request IDs on normal SSE and two concurrent sessions. Check isolation on unauthorized reads, shutdown, and restart. Run backward-compatibility checks for the current client response and `/stats` contract. Desktop ingestion of local engine records and other provider paths are later acceptance slices.

This engine slice passed. Records cover HTTP Anthropic Messages submissions from the same ASGI scope and the engine's verified request ID. The latest 64 records are held in memory. Record at most 16 submissions per request. Overflow, an incomplete SSE termination, or an upstream response without usage leaves the total unknown. A fresh engine generation starts with empty records and rejects the earlier control key. Desktop ingestion will match the generation and complete request ID. [Local evidence](../proofs/desktop-anthropic-usage.json).

<a name="h041-d-masaüstünde-kullanım-eşlemesi"></a>

### H041-d desktop usage matching

The lead agent owns this slice because it involves measurement semantics, connection ownership, and compatibility with earlier records. The existing `/stats` read supplies traffic. Apply a usage record fetched from the same owned engine with the current control key only when both engine generation and complete request ID match. Calculate input, cache reads, cache writes, and output from the raw call counters. A failed or incomplete call leaves the total unknown. Show observed zeroes. The primary measurement keeps its tokenizer basis and savings percentage when these records arrive.

Keep the existing traffic record if the optional usage endpoint is temporarily unavailable. Repeated observations may fill in usage fields. A conflict for the same ID makes usage unknown. Earlier records are readable without the new fields. Request details show input, output, cache reads and writes, and the observed upstream call count. Acceptance covers the known 2,429 example, missing fields, the wrong generation, unauthorized matching, repeated reads, and conflicts. Live providers and other protocols require their own acceptance.

Local Windows 10 acceptance passed through the real owned engine, Rust metrics reader, SQLite, and request details. Cache fields and the attempt count leave the local savings percentage on its existing basis. API account and other protocol coverage will be tested separately. [Usage record evidence](../proofs/anthropic-usage-desktop-detail.json).

<a name="h041-d-responses-httpsse-kullanım-ölçümü"></a>

### H041-d Responses HTTP/SSE usage measurement

16 November 2026. The lead agent owns implementation and acceptance because they involve measurement semantics and process ownership. The first slice covers POST Responses HTTP/SSE traffic. WebSocket, Chat Completions, live providers, and other operating systems require their own acceptance. Retain the model, reasoning, auth, compression profile, and current basis for savings percentages.

Observe usage at the actual upstream submissions by both the normal client and the separate HTTP/1 client within the same ASGI request. Bind the primary call, CCR follow-up, and retries to the same engine request ID. Store only counters and state. Missing or failed responses, truncated SSE, overflow, and identity conflicts leave the total unknown. Preserve observed zeroes. Pass SSE bytes through intact.

Responses `input_tokens` is total input. Cache reads and `input_tokens_details.cache_write_tokens`, when present, are components of that total. Count them once. Missing details remain unknown. Accept usage only from a completed response. [Usage fields](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/response_usage.py) · [SSE completion](https://developers.openai.com/api/reference/resources/responses/streaming-events).

Version 2 of the existing secret-protected usage endpoint identifies each row's surface. Anthropic and Responses share a record queue. The native reader also accepts the version 1 Anthropic format. Match only by generation and complete request ID. Keep the existing persisted measurement and preference schemas and use the provider fields already present.

An actual failure test showed that the pinned engine could wrap the JSON body of an HTTP 400 from a Responses CCR follow-up in HTTP 200. Raising an exception lower down is insufficient because the CCR handler catches it and returns the previous tool call. The app-owned guard flags failure only on this follow-up path after the existing retry policy is exhausted. Before response headers are sent, the owned ASGI boundary returns HTTP 502 without forwarding the invalid body. It generates no additional submissions or retries. Native request details indicate the failure from the client status in the same private record. Missing usage stays unknown.

Acceptance covers (1) known input/output/cache values, zeroes, and missing fields in JSON and fragmented SSE, (2) separate counters for the primary request, CCR, retries, and concurrent requests, original CCR content, and route preservation on the real owned engine, and (3) missing or failed follow-ups, unauthorized reads, and isolation after restart. The native reader → SQLite → request details must show the same total. Record source and tool fingerprints before and after. Run focused parser and normalization tests and real engine checks, then run the WORKFLOW local gate once for the final candidate. The slice uses local verification with no Actions run or real model call.

<a name="h041-d-responses-websocket-kullanım-ölçümü"></a>

### H041-d Responses WebSocket usage measurement

16 November 2026. The lead agent connects raw usage extraction and the outcome boundary of the pinned 0.44.0 engine. Attach the numeric usage for each completed upstream response to the next engine outcome ID in the same WebSocket context. Select it before the record-write await point. Match by context and identity without relying on model, time, or chat name. Verify the recorded source hashes and preserve provider and client bytes.

Separate active responses using explicit `response.id` values. The limit is 16. Intermediate content is discarded. Incomplete, ambiguous, duplicate, or overflowed scopes leave the total unknown. The engine can defer usage from an earlier failed response to the next outcome. The total stays unknown in that combination. Each completed response gets its own record. Attributing a new `response.create` sent after client CCR/MCP retrieval to the same user task requires additional evidence. [WebSocket protocol](https://developers.openai.com/api/docs/guides/websocket-mode).

The private version 2 record carries the `openai_responses_ws` surface and the observed response `completed` state in place of an HTTP status code. Input includes cache reads, and missing details remain unknown. Connection setup retries are excluded from the model call count. WS→HTTP fallback is measured through the existing HTTP observer without separately counting WS events. If fallback occurs on a connection with prior WS observations, incomplete earlier coverage leaves the total unknown. Retain compression, model, effort, auth, memory/learn, and pause defaults.

Acceptance covers (1) sequential and interleaved responses with distinct IDs on one connection, model and effort changes, measured zeroes, and missing cache fields, (2) unknown totals for missing, failed, or incorrect identities, overflow and disconnection, with isolation after reconnection, and (3) WebSocket and HTTP fallback on the real owned engine, original CCR retrieval, and the native reader → SQLite → request details. Run focused tests and HTTP and Anthropic regressions on the final candidate, then the WORKFLOW local gate once with before and after source and tool fingerprints. Live provider access, other operating systems, Actions, and a new installer have separate acceptance.

<a name="izole-chatgpt-profiliyle-canlı-websocket-doğrulaması"></a>

#### Live WebSocket verification on an isolated ChatGPT profile

18 November. In a separate Windows 10 run, test the pinned Codex CLI and 0.44.0 engine on the existing isolated free profile. Allow one optional warm-up message and one generated response in a single connection. Block other WebSocket endpoints, redirects, and upstream HTTP. Acceptance requires a correct answer and matching engine record ID and CLI counters for input, cached input, and output of the single completed generation. Record warm-up and ambiguous usage as observed. [Pinned Codex provider](https://github.com/openai/codex/blob/rust-v0.169.0-alpha.11.1/codex-rs/model-provider-info/src/lib.rs) · [OpenAI WebSocket protocol](https://developers.openai.com/api/docs/guides/websocket-mode)

The local rehearsal passed nine focused tests, one warm-up, and one generation. In the live run the CLI completed the correct answer over WebSocket. The generation matched by engine record ID had 9,784 input, zero cache reads, and 8 output, matching the CLI counters. The warm-up remained separate with 2 input and zero output. Both engines exited normally. Acceptance covers the selected ChatGPT Free profile, Windows 10, and the pinned versions. Other accounts and provider protocols require their own acceptance. [Evidence](../proofs/websocket-usage-desktop.json)

<a name="güncel-headroom-ve-serena-adayları"></a>

### Headroom and Serena release candidates

On 15 November 2026, the stable Headroom `0.44.0` and Serena `2.1.0` releases were reviewed. The active engine stays on `0.41.0` during evaluation. Compare the candidates in isolated packages with locked dependencies. [Headroom release notes](https://github.com/headroomlabs-ai/headroom/releases/tag/v0.44.0) · [Serena release](https://github.com/oraios/serena/releases/tag/v2.1.0).

1. **Headroom engine candidate.** First determine how the `0.44.0` fixes for disconnection, pending streams, and caching affect our routes and pins. Compare the previous candidate on packaged startup, owned shutdown, SSE/WS, pausing, and original CCR retrieval before deciding whether to upgrade. Verify the source of the new `/stats` cost field. Provider-reported usage and billing require their own data. [Release changes](https://github.com/headroomlabs-ai/headroom/releases/tag/v0.44.0).
2. **Lossless compression and repeats.** Add the `0.44.0` lossless `/v1/compress` surface and its protections for tables, search, and numeric JSON to the controlled comparison. Compare against existing exact-repeat merging in the pinned engine. Acceptance covers original failure text, line order, quotations, source, and numbers. Measure equivalent task results, caching, extra model and CCR calls, and completion time in the live task experiment. Disabling response buffering qualifies for comparison only once retrieval on the client side is proven. [Compression and CCR changes](https://github.com/headroomlabs-ai/headroom/releases/tag/v0.44.0).
3. **Learn preview.** The pinned 0.41.0 pattern detector powers the [local preview](#learn-önizlemesi) when the user selects it. Compare suggestions against real project rules. If provider-backed analysis is considered, the pilot measures its data scope, task quality, and token cost. [Learn review](../research/headroom-serena-2026-11-15.md#learn-ne-yapar).
4. **Serena project experiment.** On the same Rust and TypeScript tasks, compare the free language server backend's read-only symbol lookup, file summaries, and reference queries with existing CodeGraph and focused file reads. Record source correctness, total context, startup time, and memory use. The initial pilot stays within project scope and uses the three classic symbol tools. [Serena review](../research/headroom-serena-2026-11-15.md#serena).

<a name="typescript-referans-sorgusunda-serena-ve-codegraph"></a>

#### Serena and CodeGraph for TypeScript references

18 November. The lead agent owned source correctness and measurement conditions. A shared 61-file copy came from production code. The five predetermined source callers of `connectionActionClient` were compared using Serena 2.1.0 with the TypeScript 7.0.2 language server and the existing one-tool profile of CodeGraph 1.6.2. Each tool ran in three separate processes with one repeat in the same session after the first query. The index and language server were prepared in advance. Tool schema and context were measured in UTF-8 JSON bytes. Memory is the sum of private memory in the process tree at the first result.

Serena showed 5/5 caller lines on each of its first three results, and CodeGraph's single query showed 1/5. First result sizes were 5,597 and 17,588 bytes, respectively. The median time from process start to first result was 3.75 and 2.05 seconds, and private memory was 248.42 and 384.28 MiB. Every CodeGraph query started from the same index copy. All six process trees closed. This result covers the selected TypeScript symbol and single-call tool profiles. Rust and symbol-definition and file-summary comparisons were still open in the wider pilot. [Measurement](../proofs/serena-codegraph-pilot.json) · [Serena tools](https://oraios.github.io/serena/01-about/035_tools.html)

<a name="rust-referansları-sembol-tanımı-ve-dosya-özeti"></a>

#### Rust references, symbol definitions, and file summaries

18 November. The remaining five queries from the same pilot ran on an isolated 194-file source copy in three paired process runs. The Rust 1.92.0 language server ran from the test directory only. `rust-project.json` connected the source modules so internal references could be queried without a build. The earlier TypeScript reference evidence was reused after confirming matching fingerprints. [Rust Analyzer project format](https://rust-analyzer.github.io/book/non_cargo_based_projects.html)

Serena found 4/4 Rust references. A single CodeGraph query found 2/4. Both tools showed the selected Rust and TypeScript definitions, six selected symbols in the Rust file, and two exports in the TypeScript file. Median Rust reference output was 4,005 bytes for Serena and 22,311 for CodeGraph. Median time from startup to the first Rust result was 4.98 and 1.35 seconds, and private process-tree memory was 220.97 and 384.05 MiB. Serena's three tool schemas used 6,743 bytes. CodeGraph's single tool schema used 1,905. All six process trees closed.

Serena is a development tool candidate for narrow, on-demand symbol queries in this project. Results are scoped to the selected symbol and file queries. The Rust and TypeScript pilot measurements are complete. [Measurement](../proofs/serena-codegraph-pilot.json)

<a name="headroom-0440-windows-karşılaştırması"></a>

### Headroom 0.44.0 Windows comparison

16 November 2026. The PyPI Windows x64 wheel was isolated in a package with its own hash and lockfile. The candidate copy updated version and source pins for the controller, supervisor, and optimization layer. The first startup latency trace showed a network connection for the LiteLLM price map. Both versions used the bundled price map in the paired comparison. [Candidate wheel](https://pypi.org/project/headroom-ai/0.44.0/#files) · [Package and startup evidence](../proofs/headroom-044-candidate.json).

The run tested 48 client requests across Responses HTTP/SSE and WebSocket, Chat Completions SSE, and Anthropic Messages SSE. It verified model and effort, tool identity, protected content, warm prefix, pausing, new requests after normal client disconnection, and owned shutdown. The primary submission had the same local token count in each matched example. Six Anthropic usage scenarios and four Learn tests passed on the candidate.

The 0.44.0 lossless `/v1/compress` call restored file lists and search output exactly, including order and final lines. Protected numeric and HTML samples came back byte for byte. The measurement compares local tokens in synthetic messages. The source calculates the new `recent_requests.cost_usd` as a list-price estimate. [Cost calculation](https://github.com/headroomlabs-ai/headroom/blob/v0.44.0/headroom/proxy/cost.py#L548-L581) · [Measurement](../proofs/headroom-044-candidate.json).

The active version at comparison time was 0.41.0. The Windows 0.44.0 package switch and source pins were later verified locally. macOS source preparation is complete. Native macOS, task quality and net token benefit on real work, clean-OS checks, and release acceptance remain open. [Upgrade evidence](../proofs/headroom-044-upgrade.json).

<a name="paket-geçişinin-kabulü"></a>

#### Package transition acceptance

The transition uses versioned Windows source and output roots inside the project. Keep the prior engine package and its evidence. Verify the new lockfile, engine files, source map, controller, and native pins together. Both platform startup configurations use the bundled price map. The model, effort, client auth, and working-profile contracts still apply.

Read the 0.41.0 preferences only with a known version and hash pair. Reading leaves the disk untouched and returns the effective version. Saving preferences still checks revision and fingerprint conflicts. The on-disk preference reference uses the 0.41.0 format that the previous app can read, so preferences remain available after an app rollback. Earlier metric rows and the 0.41.0 metric format are supported.

<a name="geri-dönüşte-ortak-tercih-biçimi"></a>

#### Shared preference format after a rollback

18 November. When reading the shared `preferences.json`, the previous application rejected `experimental_rules` with `unsupported_schema`. Because the engine read its protection list from that document, startup returned `config_conflict`. Write the shared file with the previous v1 fields. Tie the experimental-preview choice to the active document fingerprint in the separate `experimental-rules.json` file. Move a valid existing inline choice at startup with the same revision and settings. Read-only access supports both formats.

When the preview is enabled, stage the new document fingerprint in the separate record under the file lock. Atomic replacement of the shared file puts the settings into effect. If a write is interrupted, the choice attached to the previous document remains active. Compact JSON written by the prior app cannot activate a partly staged choice. The choice stays available if the old app reads settings only. If the old app saves settings, enable the preview again in the new app. A damaged separate record displays the preview as off. The engine keeps reading the protection list from the shared file. Invalidating the choice changes the current record fingerprint.

On Windows 10, 17 preference tests, two protected requests on the 0.44.0 engine, and one protected request with the previous 0.41.0 manager built from `8206da4` passed. The prior reader was tested with both documents and rejected the incompatible format. The corrected format passed an actual startup, exact protected output, and normal shutdown on the final run. The first old-engine shutdown attempt lacked the expected acknowledgement and remains in the record. Acceptance covers the local engine and the prior source version tested. [Evidence](../proofs/preferences-rollback.json).

Normal acceptance includes readiness of the current engine, a real local stream, metrics and Learn, and owned shutdown. The error cases reject a changed file, incorrect pin, and a preference conflict. Record the local quality gate, Windows source staging, and real packaged engine checks on the final candidate. macOS source and staging checks run locally. New native macOS and Windows 11 evidence needs its own environment. The previous GitHub Actions quota limit still applies.

<a name="h041-d-chat-completions-kullanım-ölçümü"></a>

### H041-d Chat Completions usage measurement

The lead agent owns transport scope, counters, and request identity. Observe the shared HTTP clients' submissions on the pinned 0.44.0 engine's `/v1/chat/completions` route. Bind JSON and SSE usage to the real engine request ID. `prompt_tokens` includes the cache details. `completion_tokens` counts output. Evaluate optional cache fields and consistency of `total_tokens` separately. A stream requires the final usage chunk and `[DONE]` for the same response ID. Local tokenizer counts and provider usage each retain their own basis. [OpenAI Chat Completions contract](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create).

1. Normal JSON, standard SSE, and CCR follow-up. Count input including cache once, count the separate requests, and carry the data to existing desktop request details. The synthetic CCR example expects 2,400 input, 29 output, 2,000 cache read, 100 cache write, and two submissions.
2. Missing usage, truncated streams, conflicting counters, and failed submissions. Preserve observed zeroes. A missing or conflicting total stays unknown. A successful retry cannot account for usage from the earlier failed submission.
3. Fragmented and concurrent streams, explicit `include_usage: false`, engine generation and access control on the private read endpoint. The observer passes options, model, reasoning, auth, and chunks to the client intact. Coverage is Chat Completions submissions observed on the shared clients.

After focused parser and native reader checks, use the loopback engine and earlier Responses and Anthropic scenarios. Run the [WORKFLOW quality gate](../development/WORKFLOW.md) and pin check on the final local candidate. Live provider and other OS acceptance require their own environments.

The Windows 10 local candidate verified 11 scenarios and 14 synthetic upstream submissions. Usage from both CCR calls reached the native reader, SQLite, and Activity request details as 2,400 input and 29 output. The valid cache details were 2,000 read and 100 write. A replay from the local response cache produced no new provider measurement. For truncated SSE, the engine only produces a private usage record and error counter. The app creates no inferred Activity row from that record. Optional buffered Chat CCR was tested in a separate profile. [Acceptance](../proofs/chat-completions-usage.json).

<a name="yerel-gerçek-modelle-chat-completions-doğrulaması"></a>

#### Chat Completions verification with a real local model

18 November. Authorized free remote access for Chat Completions was unverified. Run the pinned llama.cpp CPU server and SmolLM2-135M in a project-local loopback test only. Use one synthetic question, a constrained JSON answer, and an SSE request with `include_usage` enabled. Acceptance requires correct JSON, completed SSE, one upstream request, and an engine usage record with the same request ID. Missing cache detail stays unknown. [llama.cpp release](https://github.com/ggml-org/llama.cpp/releases/tag/b12359) · [Model source](https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct)

The test filter stopped the first request with HTTP 403. The runner was adapted to the engine's forwarded `max_completion_tokens` field, and the server enforced its own output limit of 24 tokens. On the final run, the real model produced the expected schema-constrained JSON. SSE usage of 44 input and 15 output reached the engine record with the same request ID. Cache reads were zero and the write counter was missing. Five focused tests passed and both owned processes shut down. Remote Chat Completions acceptance awaits authorized free access. [Measurement](../proofs/chat-completions-usage.json)

<a name="sse-kullanımında-olay-başına-sınır"></a>

#### Per-event limit for SSE usage

18 November. The desktop usage observer limited the combined size of the unfinished event and new network data to 128 KiB. When individually valid Anthropic, Responses, or Chat Completions events arrived together, they could exceed that limit and drop usage counters.

All three observers feed the pinned 0.44.0 engine's existing LF/CRLF parser with a common buffer. Consuming complete events frees space. The buffer retains at most 128 KiB of an incomplete event. Deliver the same bytes and order to the client. If one event exceeds the limit, usage remains unknown and response delivery continues.

The run passed 24 focused Python tests and 5 Rust manifest checks. Windows 10 with the current 0.44.0 engine covered six gzip responses across three protocols. Combinations of valid events retained all available counters. Usage stayed unknown for an event of 131,073 bytes. All six streams returned HTTP 200 with byte-exact delivery. Request records matched exactly and the engine shut down normally. The provider usage values came from synthetic counters. [Local evidence](../proofs/sse-usage-event-limit.json)

<a name="learn-önizlemesi"></a>

### Learn preview

On 15 November, the user requested that Learn be added as an optional feature. Selecting one Codex or Claude Code JSONL session in Settings starts a review. If the picker is cancelled, the previous result remains on screen. The Windows app uses the verified Python in the existing engine package to read only the selected file. The engine does not need to start.

The pinned engine's Learn detector receives groups of at least three consecutive explicit failures in the same session, with the same tool, inputs, and error category. A different tool call, edit, scope change, or intervening concurrent operation separates the group. If the next identical call succeeds, the app shows an observation. The UI displays the suggestion type, count, and first sample line. Changes outside the file and the cause of recovery are unobserved.

Review is limited to 16 MiB, 64,000 lines, and 15 seconds. Content remains in the analysis process. This flow does not send data to a model, scan history, persist results, write files, or estimate token savings. The engine's `--no-learn` default remains in effect.

<a name="learn-öneri-faydasının-yerel-pilotu"></a>

### Local pilot for the benefit of Learn suggestions

The lead agent owns the distinction between measurement and source changes. The desktop preview program in the fixed 0.44.0 package receives records from synthetic tasks belonging only to the project. The same tool operations are compared in Codex and Claude Code formats. Record the process result, call inputs, and before/after fingerprints for related files. Compare the short suggestion with the on-screen text and existing `AGENTS.md` / `WORKFLOW.md` requirements.

1. Three identical failed reads in the wrong working directory followed by success in the corrected directory. Preview should find the failed samples and must not count the success in another directory as recovery of the same call. A matched record showing a fix after the first error should not produce a suggestion.
2. Run the same check after real source edits. Required repetitions are expected to remain when edit records are present. Separately observe the suggestion and success counts that could be misinterpreted when edit events are removed from the same raw call results. This suggestion alone is not enough to accept a rule.
3. Search preview responses for file, call, and content leakage. Measure the local token size of only the short suggestion produced. Approved instructions, engine preferences, and real model traffic remain in their existing scope. Real-task success and net provider usage remain separate matched acceptance criteria.

The local pilot passed. It ran 8 commands across two real fixture flows and 8 Codex and Claude Code previews generated from the same events. The suggested directory change matches the existing working-root rule. When source edits were observed, required repetitions did not produce a suggestion. When edit events were removed, both formats showed the same three failures and one recovery. This sample does not identify the cause of the observed recovery or how many repetitions could be avoided. The suggestion itself measured 18 local tokens in Turkish and 12 in English. Live acceptance remains open. [Evidence](../proofs/learn-merit-pilot.json).

Run the command from the project root.

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/optimization/learn_merit.py --run
```

<a name="izole-codex-ile-ilk-canlı-karşılaştırma"></a>

#### First live comparison with isolated Codex

On 18 November, eight local scenarios passed again using the existing preview sources. The live pair used Windows 10, fixed 0.44.0, the real Codex CLI, and the existing isolated ChatGPT Free account. Both arms received the same working-root rule and task in a synthetic publication test. The package report contained the correct version number, decimal latency value, and sample count. The stale report and missing file path in the source tree were also verified locally. The current English path suggestion was added to the second arm. The account, model, effort, and tool were the same. A limit of four requests per arm and eight per pair was recorded before live traffic.

| Measure | Working-root rule | Working-root rule and Learn |
| --- | ---: | ---: |
| Exact answer and correct package read | Passed | Passed |
| Provider requests and usage records | 3/3 | 4/4 |
| Repeats and incorrect reads | 0 | 0 |
| Separate path check | 0 | 1 |
| Input tokens excluding cache | 10,656 | 11,135 |
| Total input and output tokens | 30,548 | 41,255 |

The first cache read was zero in both arms. Total usage increased by 10,707 tokens (35.05%), including cache reads on later requests. The suggestion showed neither an avoided repeat nor an accuracy increase on this task, despite the extra check. Both engines closed normally, and 40 shared input fingerprints matched. One synthetic task with a fixed arm order does not replace broader benefit or billing acceptance. The Learn suggestion remains a user-selected local preview. [Results and rerun](../proofs/learn-merit-pilot.json).

#### Release decision for Learn

Keep Learn as a local preview in Settings. The user selects a saved Codex or Claude Code session file. Learn shows repeated tool failures and suggests a check.

In two live OpenCode comparisons, both arms gave the correct answers. All eight provider usage records matched the engine. Learn advice prevented zero retries and added 52 tokens (4.58%). Use the preview for troubleshooting. Keep background learning off. [Live evidence](../proofs/learn-merit-pilot.json)

<a name="ilk-ölçümlerin-planı"></a>

## Initial measurement plan

9 November 2026. H041 started after H038 with its dependencies ready. The lead agent owns measurement semantics, the real engine, and performance acceptance. Apply Ponytail full. Free local experiments proceed. Live API-key and other-OS acceptance remain open.

The first slice sends the same synthetic request over the Responses HTTP/SSE surface directly to a local fixture and to the Headroom engine opened through the production controller/helper path. Rust ingress cost is not included in this engine measurement. A separate backpressure slice examines the real relay path. No client settings or account data are used.

Before the run, pin `bench/optimization/plan.json`. Use three new engine processes, four fixtures, and three-turn conversations. Each fixture starts a new conversation and appends two turns to history. Mark the process's first request separately. Do not clear the OS file cache. Tokenizer data is prepared using existing checksums. Startup and measured-request windows are separate.

Preserve source and exact numeric data byte-for-byte in the `Read` tool. Compress repeated logs only in the tool that does not preserve them. A short JSON response also shows low benefit. Preserve call ID, tool name, model, reasoning, and fixture auth. Separately verify the forwarded multi-turn prefix and CCR recovery to original content.

For token comparison, count the sorted JSON request in both arms using the same `o200k_base` encoding. The injected tool schema is included in the count. Preserve negative differences. Do not present this local representation measurement as a bill or provider cache discount. Measure the fixture's first-content-delta time. Real-model TTFT and response quality remain unknown.

Fix the warmed added-p95 latency limit at 1,000 ms, request timeout at 30 s, observed engine private memory at 1,536 MiB maximum, and warmed-request CPU time p95 at 2,000 ms maximum. Expect at least 10% local token reduction for the repeated-log sample. Positive savings are not required for other samples. Do not raise thresholds after seeing results.

Limit measurement to the benchmark process and controller/helper/engine PIDs with verified ownership. CPU time is in single-core equivalents. Observe memory at 50 ms intervals and at request boundaries. Do not persist raw bodies, auth, control secrets, or stderr dumps. Report errors by code/class. Store before/after fingerprints together for source, plan, fixed packages, and environment.

Keep evidence under `work/proofs/optimization/` and the short result in `docs/validation/optimization-benefit.md`. Before a new check runs, verify the existing command and data prerequisites. If product source is unaffected, reuse H038's matching local gate. Verify new benchmark code with its own fixtures and real-engine run.

The second slice sends the same 4 MiB of data over direct TCP and the Rust relay. In each arm, the client's receive buffer is 2048 bytes, and it does not read for 30 seconds. `cfg(test)` counters in the production relay loop count actual WouldBlock events and written bytes only for writes to the client. Samples at 10/20/30 seconds require counters to increase in the last two windows and the transfer to remain incomplete. Compare the final data using SHA-256. The backend-writer observation in the first delayed-reader test does not replace this acceptance.

This test variant's CPU limit is 10% of one core on average. Its limit for observed private-memory growth during the blocked window was recorded in `backpressure-plan.json` as 32 MiB before the run. Sample the single compiled test process from outside. Kernel TCP memory and production release performance are separate scope. Do not increase data size or thresholds to obtain a result.

For the final relay repetition, record in both arms the time from starting to read until EOF and the time from sending the request until EOF. The first represents the cost of continued transfer. The second shows the total including the fixed wait. Report these as measurements. Keep the existing load and timeout unchanged.

The third slice compares the same four contents across Chat HTTP/SSE, Anthropic HTTP/SSE, and Responses WebSocket surfaces. `surfaces-plan.json` combines with the base plan's three new processes/three turns using the same performance thresholds: 108 matched workloads and 216 measured requests. A WebSocket conversation uses the same connection for all three turns. Connection setup time is excluded from request latency. Keep the Anthropic client's explicit cache marker fixed. Actual provider cache hits are not measured. Check auth/model/reasoning and forwarded multi-turn prefixes separately for each surface. Fixture-protocol usage counts are not inputs to token measurement. Do not use new dependencies or make real provider calls.

The remaining timed load is fixed before the run by `stability-plan.json` as 300 seconds of matched workloads at one per second. Rotate through four contents × four surfaces using new three-turn conversations. First warm one conversation for each combination. Record these 96 requests separately. Require at least 250 matched pairs and at least 50 pairs per surface, with zero protocol/integrity errors. Maximum observed engine private-memory growth after warmup is 128 MiB, with 1,536 MiB total. Combined benchmark/fixture/controller/helper/engine CPU during the load window is limited to 25% of one core on average. Preserve the previous 1,000/2,000 ms per-request p95 added-latency/CPU thresholds. Timing waits count in the CPU window. A single controlled five-minute serial load is not presented as multi-hour stability or concurrent-user capacity.

The first timed run exceeded the memory-growth limit and remains recorded as FAIL. A separate 120-second diagnostic run is not an acceptance test. It reads the engine's `/debug/memory` component counters, Win32 memory for the three owned processes, and loaded DLL names. It does not record content/auth. The monitored stores were only approximately 0.2 MiB, while growth was in the engine process. The fixed `server.py` calls `_start_kompress_background_warmup`, which invokes `preload(allow_download=True)` after readiness. Disable this background task by adding `HEADROOM_KOMPRESS_WARMUP=0` to the fixed child environment of the desktop helper. Do not disable Kompress or other compressors. Preserve lazy loading on the needed path. Acceptance of this candidate depends on remeasurement with the same 300 seconds/data/timing/thresholds. PASS results tied to prior runtime fingerprints are not performance evidence for the new candidate.

Kompress files were found in the project cache after the first two 300-second runs. The application setting stops background model loading. The fixed engine can separately pre-download files. The first `HEADROOM_OFFLINE=1` attempt stopped during readiness because it also disabled local statistics collection. Later repetitions add only `HF_HUB_OFFLINE=1` and `TRANSFORMERS_OFFLINE=1` to the run-specific copy of the application layer. Both model targets and the upstream health check use the same loopback fixture. The test records this copy's fingerprint and the zero-file-load condition in the new Hugging Face model cache. No network packet capture is performed. Duration, load, and performance thresholds remain the same. No offline policy is added to product settings.
