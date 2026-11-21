<a name="h041--context-diet--context-mode-yerel-pilotu"></a>

# H041 — Local pilot for Context Diet and Context Mode

16 November 2026. The lead agent owned the pilot because the measurement basis and the handling of failures and original output required decisions. The user excluded Billion Context.

The pilot measures what both candidates add to Headroom 0.44.0 without model calls. Its evidence covers the local pilot. Automatic client integration, task quality, provider billing, and support for other operating systems require separate evidence. The pilot keeps product and live client settings intact. It keeps packages, processes, and state under `work/proofs/context-tools/`. The download record includes pinned commits or versions and programmatically calculated hashes.

The pilot uses Context Mode's local MCP package. Gateway, setup, and global hooks are outside the pilot. The pilot runs the unchanged Context Diet filters with the existing TypeScript compiler. This test covers the filters without an MCP installation.

<a name="önceden-belirlenen-kabul"></a>

## Acceptance criteria set before the pilot

1. **Normal operation.** Use fixed logs with repeats, test output with unique lines, and large JSON with a target record. Compare raw output, Headroom, Diet → Headroom, and Mode → Headroom. Count the initial tool response and any required search response separately. Treat a query for a JSON subset as a different operation from compression of the full JSON.
2. **Errors.** Test a failure on stdout, a warning on stderr, and a nonzero exit status. Preserve the error identifiers, numbers, and exit status. Check stderr from a successful process too. A lower token count cannot compensate for lost information. Diet must report an error if its output limit cannot fit a protected line. Compare the original Mode output and search result with the expected synthetic content.
3. **Boundary cases.** Test short output, Turkish and other Unicode text, long exact integers and decimals, and a source file. Product integration must pass source and numeric inputs through when they require a complete read. Check the filter result directly. The pilot must expose errors that a pass-through could hide.

Limit each fixture to 1 MiB. Limit each tool call to 30 seconds. Run calls in sequence with one owned Mode process and one owned Headroom process at a time. Run each fixture three times. Start new processes and state for each repeat. Measure startup separately.

Each engine processes 44 proxy requests. Keep the product's default limit of 60 requests per minute. The previous test with a single engine received a local HTTP 429 on its 64th rapid request. The rate limit caused that response. Each repeat must yield the same token counts and acceptance outcomes. The source, toolchain, and configuration fingerprints must match before and after verification. The timing results describe this small run. The pilot sets no production service-level agreement.

The pilot uses the local `o200k_base` tokenizer for the full UTF-8 tool text and the same Responses request envelope. Exclude Diet's characters/4 estimate from the percentage calculation. Account for tool schemas, call arguments, and extra search responses separately. A reduction in the selected output alone measures that output. Keep the model, authentication method, reasoning level, and tool identity the same in all Headroom arms. Reject a candidate as an automatic general filter if it loses an error or cannot retrieve the original archive.

The pilot links results to `docs/proofs/context-tools-pilot.json` and the original short assessment in Turkish. Repeat the full product quality gate only if product code changes. Run the pilot through the actual MCP, filter, and Headroom paths. Run the pilot's error checks. The main H041 acceptance stays open.

<a name="yeniden-üretme"></a>

## Reproduce the pilot

Use a work directory that does not exist. Run the commands from the project root.

```powershell
python -B bench/context_tools/prepare.py work/proofs/context-tools/yeni-kosu
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/context_tools/pilot.py work/proofs/context-tools/yeni-kosu
```

The preparation step downloads only the pinned source and package. It transpiles Diet to JavaScript with the existing TypeScript 7.0.2 compiler. It installs no dependencies or global tools. Transpilation does not check types. Six upstream test files verify filter and CLI behavior separately. Reuse a prior passing filter result with `--reuse-diet-proof <result.json>` only if the test sources and Node fingerprint match. This reuse covers the filter checks only.

A smaller output must still pass the acceptance checks. The report records visible errors, the need for a complete read, and the repeat count in separate fields. It also records Mode archive normalization and exact recovery with Headroom's lossless decoder or CCR in separate fields. `evaluation_completed` means the experiment finished. Automatic product integration requires separate acceptance.

Keep the full expected error line out of `intent` and `queries` in the search check. The tool can echo the query even when it finds no results. Use a short keyword as the query. Accept the response only if the tool retrieves the full expected line from the file or index. Reject a query echo that appears to recover a missing stderr warning.
