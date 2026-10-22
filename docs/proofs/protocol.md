# H007 protocol proof

## H007-a: Responses and Anthropic protocol forwarding

**Result: PASS on the Windows 10 development host, against the real pinned Headroom 0.41.0 proxy.** The runner used the isolated CPython 3.13.13 runtime, a loopback proxy listener owned by its child PID, and an in-memory loopback upstream fixture. All credentials were the nonfunctional proof sentinel; no paid provider or model request was made.

The run verified Responses model and reasoning fields, fragmented tool arguments, opaque input/event fields, Anthropic model and thinking settings, sentinel auth and API-version headers, stream event ordering, fragmented tool arguments, opaque output blocks/events, and a controlled upstream 429 with its `Retry-After` header and body preserved.

Five real Anthropic HTTP requests covered an opaque block at the end, beginning, middle, by itself, and with existing client `cache_control` markers on both the text and opaque blocks. In each case the fixture received the opaque block and nested metadata unchanged. Both explicit markers, including their `1h` TTL, remained intact. In the no-client-marker cases, the workaround retained Headroom's generated marker on a preceding text block when one existed; for a message containing only an opaque block it removed that generated marker rather than modifying the block.

Headroom 0.41.0's `normalize_message_cache_control()` places a legacy generated marker on the final block when no current client breakpoint exists. The proof installs a small startup shim that runs only when both the engine version is exactly `0.41.0` and the packaged `cache/prefix_tracker.py` SHA-256 matches the pinned source below. Its recognized content block names follow the [official Anthropic Python SDK `MessageParam` union](https://raw.githubusercontent.com/anthropics/anthropic-sdk-python/main/src/anthropic/types/message_param.py), with additional already-used block names retained; other types are treated as opaque. The shim changes only engine-generated markers placed on those unrecognized blocks; it leaves explicit client markers and recognized block types alone. A version or source mismatch aborts startup before the proxy becomes ready. Two focused guard tests confirmed both mismatch cases leave the normalizer unpatched. This is a proof-scoped compatibility adaptation, not a vendor runtime edit.

Reproduce from the project root with:

```powershell
python work/proofs/protocol/run_protocol_proof.py --phase a
python -m unittest discover -s tests/fixtures/protocol -p test_cache_control_shim.py
```

Recorded real-proxy run: `h007-20261019T150637Z-d6e879`, on `Windows-10-10.0.19045-SP0`. The runner host was Python 3.14.0; the engine runtime was CPython 3.13.13. The run confirmed `/readyz` version `0.41.0`, listener PID equal to the spawned engine PID, then process exit, owned-port closure, and temporary-state removal.

Fingerprints for the real-proxy PASS:

| Input | SHA-256 |
| --- | --- |
| Engine source set | `1b33ee6630f75be1b7a4e3edc32449309df0b7c77adbc0f97339daa4d001db2e` |
| Engine lock | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| Runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Packaged `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| `prefix_tracker.py` source guard | `f1af4ee279f879efc48ddfebbff125a088893bb64ee960ca2dbd4ad1cdb628ee` |
| Proof runner | `751db3d0bcb84fe868c9c120f733cb65c49e1d54fd6e4a7f4747fd8351df70e2` |
| Startup shim | `ebc80ca2958b178ed488fb953f1a09cf2dd803b23352dcc3f8f0a644414e30c1` |
| Local upstream fixture | `d2e8dfa09137b0b5657735edb2beb1facdb55a38cef906f6668fbce5169fbe8a` |
| Imported runtime/process helper | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Command fingerprint | `9fee5a5c360a69ed3a1c805cd4adfb526fdaa18541e9b11e485e1e9605dd67b6` |
| Safe environment fingerprint | `cba2eeebe1f4f5ac6d0ab6736dd3141630b3f309eb5d17984ec3b60e0a5d1082` |

The standalone mismatch-guard test file SHA-256 is `252201a8653d1e4be02c8dce69b7fa43539dba29af8c91f347ea4df4942d6839`; both tests passed. The proof runner removes each run's temporary state after cleanup and prints hashes rather than request bodies.

## H007-b: Responses compression and CCR retrieval

**Result: PASS on the Windows 10 development host, against the real pinned Headroom 0.41.0 proxy.** The runner started the packaged engine on a loopback port and confirmed `/readyz` plus listener ownership by the spawned engine PID. Its upstream was the in-memory local HTTP fixture; no paid provider or model request was made.

The positive input followed a tool-result turn order: user message, `function_call`, then the newest `function_call_output`. The fixture observed a CCR retrieval marker on the forwarded request. Its 153,599-byte synthetic log result was reduced to 106 UTF-8 bytes (99.93% fewer forwarded bytes). The proof extracted the marker hash from the actual forwarded payload, posted it to the same process's public `POST /v1/retrieve` route, and verified that `original_content` matched the original text exactly at 153,599 bytes. `GET /v1/retrieve/stats` reported `default_ttl_seconds: 7200`.

The same HTTP path also preserved a short `Read` source result and a short numeric result exactly; the request and upstream-forwarded body hashes matched. Those examples appear before a trailing user message, so they establish exact preservation of earlier content only. They do not prove active protection for a large `Read` or numeric result that is the newest observation; that focused case remains pending. The fixture stores requests only in memory, and the run-specific engine state was removed after the owned process exited and its listener closed.

The proof used `HEADROOM_MODE=cache`, `HEADROOM_SAVINGS_PROFILE=coding`, `HEADROOM_OUTPUT_SHAPER=0`, `HEADROOM_MODEL_ROUTER_ENABLED=0`, `HEADROOM_CCR_TTL_SECONDS=7200`, local telemetry enabled, and anonymous beacon disabled. The engine's loopback `/health` response confirmed cache mode, coding profile, optimization enabled, and the output-shaper value `0`.

Reproduce from the project root with:

```powershell
python work/proofs/protocol/run_protocol_proof.py --phase b
```

Recorded run: `h007-20261019T145629Z-34c344`, on `Windows-10-10.0.19045-SP0`. The runner's Python host was 3.14.0; the packaged engine runtime is CPython 3.13.13, verified from the manifest and executable. Fingerprints for this PASS:

| Input | SHA-256 |
| --- | --- |
| Engine source set | `1b33ee6630f75be1b7a4e3edc32449309df0b7c77adbc0f97339daa4d001db2e` |
| Engine lock | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| Runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Packaged `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Proof runner | `aa3c9dfe38386f84ee0c15e2b835cd1d3e127df134ca49ba088df54933acc771` |
| Local upstream fixture | `d2e8dfa09137b0b5657735edb2beb1facdb55a38cef906f6668fbce5169fbe8a` |
| Imported runtime/process helper | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Command fingerprint | `aece9e9adec7373285b0d1fe748a7e65e8447607b1bf901ca6390f6dbc8a453c` |
| Safe environment fingerprint | `13ed7c09bad584c461afe17bbde79023ad1399b175d2c4c179f2a38aa4a78feb` |

This recorded H007-b PASS used runner SHA-256 `aa3c9dfe38386f84ee0c15e2b835cd1d3e127df134ca49ba088df54933acc771`. The later shim wraps `normalize_message_cache_control()`, which the pinned `/v1/responses` compression route does not call; it does not invalidate this B evidence, so the successful B command was not repeated.

## H007-c: Latest large Read and numeric tool observations

**Result: PASS on the Windows 10 development host, against the real pinned Headroom 0.41.0 proxy.** This was a separate `--phase c` run; it did not repeat the accepted A or B scenarios. The runner sent two real Responses requests through the owned loopback proxy to the in-memory upstream fixture, using only the sentinel authorization value.

Both inputs ended in the tested `function_call_output`, after a user item and its matching `function_call`; there was no later user item that could make the output historical. The read case used `exec_command` with `cat src/decision_fixture.ts` and a 60,900-byte TypeScript-like source result containing 420 CRLF lines, Unicode, and exact numeric literals. The proxy forwarded it exactly. `/health` reported the active `cache`/`coding` profile with optimization enabled. The pinned `agent_savings.py` source identified `coding.protect_reads=True`; its `proxy_env()` and default-seeding function both produced `HEADROOM_PROTECT_READS=1`. The exact newest shell-read result confirms the behavior through the live Responses route.

The second latest output was 59,219 bytes of synthetic plain-text numeric decisions, carrying `ACCT-000042`, `0.000100`, `-0.00`, and `9007199254740993123456789012345`. It was forwarded byte-for-byte while preserving model `o4-mini`, reasoning `high`, the proof-only auth sentinel, `call-numeric-c`, and the original tool arguments. The same identity/model/reasoning/auth/call-ID assertions passed for the read request (`call-read-c`). `/health` listed `numeric_report` in effective excluded tools, matching the C-only `HEADROOM_PROTECT_TOOL_RESULTS=numeric_report` setting.

This is a narrow, explicit exclusion for the synthetic `numeric_report` tool name, not a default numeric-content detector or a guarantee for all numeric tool outputs. The pinned engine documents that excluded results can still take shape-specific lossless folds: JSON whitespace minification preserves parsed data but not necessarily byte spelling, while search/log folds have their own shape gates. This proof's plain-text numeric fixture did not match those fold paths and remained exact. The coding profile's read protection is also type/content gated: it protects shell file reads by default while releasing confidently recognized data types. The fixture and contents stayed in memory; the summary contains sizes, values checked, request-body hashes, and environment fingerprints only.

Reproduce from the project root with:

```powershell
python work/proofs/protocol/run_protocol_proof.py --phase c
```

Recorded run: `h007-20261019T151524Z-c49c11`, on `Windows-10-10.0.19045-SP0`. Host Python was 3.14.0; the engine runtime was CPython 3.13.13 with Headroom 0.41.0. Both requests reached the fixture once. The listener PID matched the spawned engine PID; shutdown exited the owned process, closed its port, and removed temporary state. After a review fix made `--phase a|b|c` mandatory (preventing an arg-less A+B run from printing a generic H007 PASS), only the focused C run was repeated; A and B were not rerun.

Fingerprints for this PASS:

| Input | SHA-256 |
| --- | --- |
| Engine source-set fingerprint | `3fbc4faf872a5d7e2d9452b34ebf56ec9d1876cdd418e5185fe0c62a049c2532` |
| `agent_savings.py` | `3d2c7c7035ff0c4765c04b873d6a22a7f75c79627ba0bf76b47718fd10a46502` |
| `transforms/content_router.py` | `96ded380886d2f491f1441dd2d510f4d73ad0bc978e7ebd53e45912fad4b6b45` |
| `proxy/handlers/openai.py` | `05f624cd0c704e9b8ba4b332284cd7fe7e81708f7e237da123fc216aabc8b4af` |
| `proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| `proxy/models.py` | `fcf7736dab880b1661cc202e9d1988a6042f0e781ea637fe5d2fba1f5d61ee9c` |
| `cli/proxy.py` | `19713bfbb09f6506ad51035925eaee20342ab6834294535047d90a3b81d57a74` |
| `config.py` | `73358bd66653396dd365b89fdec589c03d41063d2df52b8435f50a8067787928` |
| Engine lock | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| Runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Packaged `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Proof runner | `0c9146bd4d8ae976306cb20b97fbce2abcce011e8117d82a77bd3a8512535848` |
| Local upstream fixture | `d2e8dfa09137b0b5657735edb2beb1facdb55a38cef906f6668fbce5169fbe8a` |
| Command fingerprint | `9fee5a5c360a69ed3a1c805cd4adfb526fdaa18541e9b11e485e1e9605dd67b6` |
| Safe environment fingerprint | `531a43115e4048a105d89f3fcf49248c6371e71ca00982f6e86c5da542e33489` |

The C-only environment used `HEADROOM_MODE=cache`, `HEADROOM_SAVINGS_PROFILE=coding`, `HEADROOM_PROTECT_TOOL_RESULTS=numeric_report`, `HEADROOM_OUTPUT_SHAPER=0`, `HEADROOM_MODEL_ROUTER_ENABLED=0`, local telemetry on, and anonymous beacon off. The appended phase-C runner branch and per-run exclusion do not alter the recorded A or B inputs; their successful commands were not repeated.

The old short/history byte-exact examples in H007-b remain limited to earlier-prefix preservation and are not evidence for newest large observations. H007-c covers that local Windows 10 case. The coordinator accepted H007's local protocol scope from A, B, C and the guard tests. Windows 11, a clean machine, real clients, and Tauri integration remain separate open gates in H006/H008–H010/H014/H037; this acceptance does not establish those capabilities.
