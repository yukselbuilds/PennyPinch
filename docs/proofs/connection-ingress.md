# H040-a/a2: stable ingress and standalone relay local proofs

**Current result (21 October2026): H040 local acceptance passed on Windows10.** RuntimeManager/Tauri wiring, two real pinned-engine generations, the final68-test Rust gate, and native physical UI control/idle keep-alive/normal exit/reopen are accepted. Historical A/A2/B fingerprints remain below; the final acceptance section describes the current candidate. Live clients/auth/models/subagents, Windows11, installer and release gates remain separate.

## Decision and behavior

Decision 003 selects a Rust-owned `127.0.0.1` TCP endpoint in front of the existing dynamic engine port. The listener is created with `WSASocketW`, `WSA_FLAG_NO_HANDLE_INHERIT | WSA_FLAG_OVERLAPPED`, and `SO_EXCLUSIVEADDRUSE` set before `bind`/`listen`; a `WinsockRuntime` guard balances `WSAStartup`/`WSACleanup`. This does not add a dependency or change the engine payload.

The endpoint record is bounded to 1 KiB and contains only schema 1, loopback address, port, UUID endpoint ID, and revision. First bind uses port zero while retaining the socket, writes a `create_new` temp file, flushes it, then publishes with same-directory `MoveFileExW(MOVEFILE_WRITE_THROUGH)` without replace. Reopen binds the saved port exactly. A busy port is a fixed conflict; it does not rewrite the record or select another address. Unknown/duplicate fields, invalid records, unsafe roots, and reparse files fail closed.

`verify_backend_peer` accepts only an IPv4 loopback socket and checks the Windows TCP owner table for exactly one established server-side local/remote 4-tuple before any later relay code may forward bytes. Missing/ambiguous tuples and a PID mismatch return fixed errors. This slice does not itself accept and relay client data, own a manager child handle, or prove that a caller invokes the check before reading the client socket.

## Focused evidence

Before adding the exclusive listener, a dual-`SO_REUSEADDR` local socket pair was bound to the same loopback endpoint; the second bind returned success, causing the initial “unprotected listener rejects contender” assertion to fail. The final suite retains this as a passing positive control. A plain `TcpListener` with only a reuse-enabled contender was separately refused in an early probe; that probe was not takeover evidence and is not generalized as a Winsock guarantee. The production `SO_EXCLUSIVEADDRUSE` listener then rejected the same reuse-enabled contender.

Eight focused tests passed:

- dual-reuse positive control and exclusive-listener negative control;
- endpoint create/reopen preserves endpoint ID and port;
- a competing listener causes conflict without changing the record; releasing it permits binding the same saved port;
- corrupt/unknown and duplicate-field state is rejected without modifying the existing file;
- a real local established connection resolves to the current process PID; a wrong expected PID is rejected;
- synthetic owner rows reject missing, duplicate, and tuple-mismatched entries.

The accepted-connection close/reopen observation returned `accepted_connection_rebind=available; saved_port_preserved=true` on this Windows 10 host. It covers one test-owned TCP connection that was fully closed before immediate rebind. It does not cover a long-lived keep-alive, active SSE, another Windows version, or a real application process restart.

The tests use new UUID directories under `work/proofs/connection-ingress/`; fixture cleanup first confirms the exact canonical UUID target, project-boundary containment, and absence of symlink/reparse entries. No engine, client config, paid provider, or user state was used. No raw body, auth value, user path, or failure log is persisted.

## Reproduction and fingerprints

From `D:\Projeler\Headroom` on Windows 10 x64 build 10.0.19045, with the project-local Cargo cache and target directory:

```powershell
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
rustfmt --check --edition 2021 src-tauri/src/runtime/ingress.rs src-tauri/src/runtime/ingress_tests.rs
cargo test --manifest-path src-tauri/Cargo.toml runtime::ingress:: -- --nocapture --test-threads=1
```

Result: `8 passed, 0 failed`; formatting check passed. `rustc 1.92.0 (ded5c06cf 2025-12-08)`, `cargo 1.92.0 (344c4567c 2025-10-21)`. Environment/command fingerprint SHA-256: `17d70772d03dd94322f254f91e5b61f3830eb50a0f791ae9d53053d7dd5996fa`.

| File | SHA-256 |
| --- | --- |
| `src-tauri/Cargo.toml` | `fe9a64d28e145777072604571418411a29a326d7a8a1b1da3f1f30398d15d298` |
| `src-tauri/Cargo.lock` | `f4e0081f60dc2dba489f0133d4cf4068e75632f7076cfb97a77858355931d140` |
| `src-tauri/src/runtime/mod.rs` | `b8c2167dbf5e6e2177f22b8b9a6dbd1ac35d8b94eff90871be74bd6b901b1338` |
| `src-tauri/src/runtime/ingress.rs` | `66ec79a1b50c6132078ba9e8cf81f6485d3065d68d54d8b59bf70d91edad50a9` |
| `src-tauri/src/runtime/ingress_tests.rs` | `183eab0356242a6f3c415fa6e058130f9f42a6d61e7fd252701cce0cf0881caf` |
| `docs/decisions/003-connection-ingress.md` | `f4267f0970a9bd040df050a7fc35749cc4e5191067df2008a533434c96198589` |

## Primary Windows references

- [SO_EXCLUSIVEADDRUSE](https://learn.microsoft.com/en-us/windows/win32/winsock/so-exclusiveaddruse)
- [SO_REUSEADDR and SO_EXCLUSIVEADDRUSE behavior](https://learn.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse)
- [WSASocketW and no-handle-inherit](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsasocketw)
- [setsockopt](https://learn.microsoft.com/en-us/windows/win32/api/winsock/nf-winsock-setsockopt)
- [bind](https://learn.microsoft.com/en-us/windows/win32/api/winsock/nf-winsock-bind)

## H040-a2: generation-pinned opaque relay and bounded drain

**Result:** the standalone relay/drain layer passed focused local Windows fixtures. H040 remains open: this does not wire the relay to `RuntimeManager`, the pinned Headroom engine, Tauri commands, or a real client.

`src-tauri/src/runtime/ingress_relay.rs` adds an immutable backend lease containing loopback port, backend generation, expected engine PID, and liveness callback. Each accepted TCP connection is pinned to the lease it observed; route changes affect only later connections. Before the relay reads any client payload, it connects to that backend and checks the established TCP owner table against the exact server-side IPv4 4-tuple and expected PID. A missing owner row gets at most 40 ms for Windows table visibility; known foreign PID, ambiguous row, malformed/unavailable table, or invalid target fails closed. A separate process-owned child listener test confirms wrong-PID rejection, zero forwarded client bytes, and that the unrelated child remains alive.

The relay is opaque: no HTTP parsing, request replay, auth/header/body rewrite, or content logging/persistence. One bidirectional nonblocking pump uses bounded fixed-capacity buffers and partial-write offsets, so slow readers preserve the original byte sequence rather than truncating on a short poll tick. It supports half-close, bounded connection count, connect/idle/lifetime deadlines, route-none rejection before payload, and cancellation. `begin_drain` removes the route; `finish_drain` waits for active TCP connections and marks timeout/cancellation as non-graceful. `shutdown` reports listener release only after its bounded accept-thread join. `active_connection_count` counts TCP connections, not HTTP requests or SSE streams.

At the original H040-a2 acceptance point, the combined endpoint/relay gate reported **21 passed, 0 failed** (nine endpoint/peer-owner and twelve relay tests); that historical result remains separate from the manager-integration candidate below. The relay cases covered raw Responses/Anthropic-like bytes and half-close, stream delta before gated terminal frame, old/new backend generation pinning, no replay after partial SSE, delayed-reader large response, no-route/no-payload, connection cap and bounded drain cancellation, idle/lifetime expiry, and actual foreign-listener PID refusal.

Commands and environment:

```powershell
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
$env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup'
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml runtime::ingress -- --nocapture --test-threads=1
```

Host: Windows 10 x64, build `10.0.19045`; `rustc 1.92.0 (ded5c06cf 2025-12-08)`, `cargo 1.92.0 (344c4567c 2025-10-21)`. Environment/command fingerprint SHA-256: `8b415bee82518b5764fd188f7834ba5ef0a53a8aeedd7da81b2e404a877ffcd1`.

| File | SHA-256 |
| --- | --- |
| `src-tauri/Cargo.toml` | `fe9a64d28e145777072604571418411a29a326d7a8a1b1da3f1f30398d15d298` |
| `src-tauri/Cargo.lock` | `f4e0081f60dc2dba489f0133d4cf4068e75632f7076cfb97a77858355931d140` |
| `src-tauri/src/runtime/mod.rs` | `9d0f8550bfe34990b0e220832538dd121bf67edff444117bdeebd0974d6a60c1` |
| `src-tauri/src/runtime/ingress.rs` | `a4c8406447c44748a469fda73d60fcef824613bbf7e52bf672095ddcf2410417` |
| `src-tauri/src/runtime/ingress_tests.rs` | `c86bde7f1996f0d3b6fe56a57fbbd77c72ea58abd3f2255f0878e73b0c511552` |
| `src-tauri/src/runtime/ingress_relay.rs` | `91c63598a401cab73e91fa96adbd52712f4d0a0cb463640f4f7166454544033b` |
| `src-tauri/src/runtime/ingress_relay_tests.rs` | `2488a2329e6ec5989bab0807f0e2e27e51a5b2802fd60252db47b4aee6b0e37c` |

**Limits at the original H040-a2 checkpoint:** tests ran on this Windows 10 host with synthetic local sockets only; no provider/model/API key or real client was used. The fake liveness callback and PID fixture did not prove RuntimeManager Child/Job ownership, process-exit ordering, pinned Headroom readiness, client compatibility, app-exit/tray behavior, Windows 11, or a clean machine. The manager integration evidence is recorded separately below. No packet, auth, request body, or raw failure log was persisted.

## H040-b: RuntimeManager-owned ingress candidate (21 October 2026)

`RuntimeManager` now owns the stable ingress and relay in its actor. Each route lease is tied to the current immutable engine generation and weak references to manager-owned process/job handles; relay connections do not prolong the kill-on-job-close lifetime. Actor readiness combines engine ownership/readiness with the ingress route. Stop removes the route first, requests the pinned engine's graceful stop, then performs bounded relay drain and verifies owned cleanup. A separate `get_ingress_snapshot` reply keeps stable endpoint identity/address apart from backend generation and uses only fixed status/error fields plus a nullable TCP connection count. Runtime snapshot schema 1 remains unchanged. The Tauri command is registered; its UI consumer and native H040-c smoke remain separate.

Focused manager tests reported **18 passed, 0 failed, 1 ignored** at the B checkpoint. The ignored real pinned-engine smoke was not run in that slice. Unit coverage concerned DTO/correlation, stale health, endpoint conflict, cleanup policy and queue boundaries. Actual duplicate start, generation-bound stop and pinned runtime lifecycle are established by C below. The B pass was reused while its manager/test sources were unchanged during the relay-fixture correction.

The final current relay filter reported **13 passed, 0 failed**. The delayed-reader fixture now configures client `SO_RCVBUF=2048` before connect and verifies the effective value; a synthetic GET must be relayed and read by the backend before it begins the 4 MiB response. The client delays reading for 180 ms, then receives the exact response bytes. During a bounded diagnostic run, both test-side `SO_RCVBUF` and `SO_SNDBUF` were 2048 while the fixture writer's nonblocking `WouldBlock` count remained zero on this Windows loopback host. The measurement was in the **backend fixture writer**, not the relay-to-client write path. The final fixture therefore claims delayed-reader byte-exact transfer only, not observed kernel backpressure; no payload size or production limit was increased to manufacture that result. Relay backpressure correctness/performance under a sustained blocked peer remains open for H041.

`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed. `cargo check --manifest-path src-tauri/Cargo.toml --offline` passed with three existing dead-code warnings (`StableIngress::port`, `ActiveConnection::generation`, and `IngressRelay::last_fixed_failure`); no compiler errors. Commands used project-local `CARGO_HOME`, `CARGO_TARGET_DIR`, and `RUSTUP_HOME`:

```powershell
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
$env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup'
cargo test --manifest-path src-tauri/Cargo.toml runtime::ingress_relay::tests -- --test-threads=1
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --offline
```

Manager command reused from the same unchanged manager/test sources:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml runtime::manager::tests -- --test-threads=1
```

Environment/command fingerprint SHA-256 (Windows 10 build `10.0.19045.0`, `rustc 1.92.0 (ded5c06cf 2025-12-08)`, `cargo 1.92.0 (344c4567c 2025-10-21)`, project-local Cargo cache/target): `9bc3c407ea961f575ab3f485e4149b2cd1fd371e23c7bc69d2943b2b75d7a8d4`.

| File | SHA-256 |
| --- | --- |
| `src-tauri/Cargo.toml` | `fe9a64d28e145777072604571418411a29a326d7a8a1b1da3f1f30398d15d298` |
| `src-tauri/Cargo.lock` | `f4e0081f60dc2dba489f0133d4cf4068e75632f7076cfb97a77858355931d140` |
| `src-tauri/src/main.rs` | `77d574ea632713bc0b46d12f1d0311ad0c30105f3a83fe0884bf2d161957a5c3` |
| `src-tauri/src/runtime/mod.rs` | `9d0f8550bfe34990b0e220832538dd121bf67edff444117bdeebd0974d6a60c1` |
| `src-tauri/src/runtime/manager.rs` | `00fee72ee39d61aaab1391da492ccd59ed6943f040a51f1b03481823542a2494` |
| `src-tauri/src/runtime/manager_tests.rs` | `9722d5e0f84cd5a4749c4f1a701aa892cbaea53a5435197580853b91ba242eef` |
| `src-tauri/src/runtime/ingress_relay.rs` | `e2c5f6baa5f06cfb9ea70a5aae329b16072e7f4aac253f4777ba1ca843bde703` |
| `src-tauri/src/runtime/ingress_relay_tests.rs` | `9e552f0a2298b69589c0e43dbc9ec64e0be479d7a17a153d7e5edb67bc9578f7` |

## H040-c pinned-engine lifecycle smoke (21 October 2026)

The opt-in test `runtime::manager::tests::live_manager_stable_ingress_survives_pinned_engine_restart` passed once against the pinned runtime payload without modifying it:

```powershell
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
$env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup'
cargo test --manifest-path src-tauri/Cargo.toml runtime::manager::tests::live_manager_stable_ingress_survives_pinned_engine_restart -- --ignored --exact --test-threads=1
```

Result: **1 passed, 0 failed**, test duration 36.66 seconds. On the Windows 10 x64 host (build `10.0.19045.0`, `rustc 1.92.0`, `cargo 1.92.0`), the real manager started two pinned engine generations. The stable endpoint ID and port survived the restart; `/readyz` traversed ingress; the endpoint remained ready beyond its observation expiry; duplicate start returned the existing generation; a stale-generation stop was rejected; the owned engine listener closed; a separate test-owned foreign listener remained alive; after manager shutdown, the saved endpoint reopened on the same port. During the stopped-engine interval the route was absent, and the no-route socket returned no response bytes. Windows may report this deliberate close as `ConnectionReset`; the test treats only fixed socket-closure errors as an empty close, never as an HTTP response or successful request.

The first diagnostic exposed a Job Object count of five rather than three: the three held controller/helper/engine handles and their individual job-membership checks, generation, parent chain, process liveness, and exact listener owner all matched; the two additional members were console hosts. The production check now requires at least the three expected members and still verifies each expected held handle and membership individually. It does not identify or trust members by basename. A separate startup delay showed that the endpoint observation could expire before successful route publication; publishing a validated route now refreshes the endpoint ready observation and timestamp. These were narrow lifecycle corrections, not changes to the pinned engine or relay protocol.

The test is provider-free: no paid request, external auth, model/reasoning call, or user/client configuration was used. It does not prove a Responses/Anthropic request to the engine, CCR retrieval, a gated SSE stop with the live engine, actual client compatibility, application-exit/tray behavior, Windows 11, or a clean-machine result. The opaque HTTP/SSE fixtures in H040-a2 are separate evidence. No direct-upstream fallback is configured or inferred. Raw output, packet contents, credentials, PIDs, and full user paths are not persisted. Process cleanup was verified after the smoke; no new run root remained, and pre-existing retained roots were left untouched.

The manager suite had **21 passed, 0 failed, 1 ignored** before the final ignored-test-only Windows socket-close assertion was added; this final fixture-only change did not alter manager behavior. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed on the final candidate. `cargo check --offline` and the manager suite are reused from the same production source candidate. The unchanged relay suite previously reported **13 passed, 0 failed**. At that slice checkpoint, the final local gate and native UI acceptance were delegated to the parent; their completed results are below.

The environment/command fingerprint is `9bc3c407ea961f575ab3f485e4149b2cd1fd371e23c7bc69d2943b2b75d7a8d4`. Full final source and pinned-payload hashes, bounded diagnostic outcomes, and cleanup metadata are in [`pinned-smoke.json`](../../work/proofs/connection-ingress/pinned-smoke.json). The final manager SHA-256 is `6f4b227ba00369dfcdfd849057e3c1dc9f6d48a0d66e0c84e87ddcfc3ab91093`; manager test SHA-256 is `4521dc3a1a2b2289654b2a58b1d26e65c866e7d1b2b5c4ab0c356d2a40a5c6ae`.

## Final parent gate and native acceptance —21 October2026

The reviewed candidate ran the full local Rust suite once: **68 passed,0 failed,2 opt-in ignored**,8.167s. The real pinned lifecycle opt-in separately passed1/0; the other ignored discovery/environment gate remains outside H040. `cargo build --manifest-path src-tauri/Cargo.toml --offline --features custom-protocol` produced the embedded-dist debug executable in32.843s. Both commands exited0 with0 warnings. Final fmt/check PASS was reused with matching source/toolchain; no Vite or HTTP dev server was launched. [Gate driver](../../work/proofs/connection-ingress/final_quality.py), [commands/source/toolchain hashes](../../work/proofs/connection-ingress/final-quality.json).

The executable is14,358,016 bytes, SHA-256 `233ddb9a5a008f72091f5907a0a7cf9ff63a58ba1f3540022bfb75e8ceef14e4`. All16 gate source/config hashes stayed unchanged during the gate. Thirteen recorded frontend source/config/dist fingerprints matched; Node25.2.1,Edge155.0.4283.62 and Playwright1.62.1 matched the previous32-layout/14-decoder evidence, reused within its static scope. Native behavior was tested separately against the new executable.

Open Computer Use MCP observed the actual Graphite/English/dark window. Global indexed physical clicks exercised Start→Ready→Stop→Stopped→Start→Ready; `/readyz`200/version0.41.0 traversed the app-owned stable ingress for both engine processes. Windows listener metadata confirmed ingress ownership; three Python roles changed after restart and the previous roles disappeared. Connection remained Not verified, compression control Not implemented, and savings amount/rate unknown; no client connection or fabricated metrics were shown.

Immediately before native Stop, a completed real HTTP `/readyz` left an open idle keep-alive socket. It closed1224ms after the control-request boundary; UI reached Stopped, previous engine roles disappeared and the stable listener remained app-owned. This demonstrates actual idle-connection stop ordering. It is not active pinned-engine gated-SSE stop or a performance guarantee.

Normal global Alt+F4 from the second Ready engine returned exit0; runtime roles and ingress listener were gone. A new app process opened the exact same saved endpoint record/ID/port, showed Stopped, and closed with a second exit0. Final owned-process/listener count0; preference SHA unchanged at `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. [Native proof](../../work/proofs/connection-ingress/native-proof.json), [cleanup](../../work/proofs/connection-ingress/native-cleanup.json).

Tool limits: one click was refused after AX-only observation; a fresh screenshot and fresh idle connection preceded the successful action. Last close returned appNotFound during post-action snapshot, independently confirmed by retained process exit0. No protection was disabled, no test window remains open and no continuous desktop monitoring was added.

This layered evidence accepts H040's local stable-address/ownership/opaque-relay contract. Pinned engine/controller/helper/shim and real client/auth/model settings were unchanged. Direct-upstream fallback stays disabled. Active pinned-engine SSE stop, pause/CCR history, real clients/providers/subagents, sustained kernel backpressure/perf, Windows11, clean-machine and installer/release support stay in their named later gates; see the [failure matrix](../validation/ingress-failure-matrix.md). Another process can acquire the saved port while Headroom is fully closed; normal reopen success does not remove that lifecycle limit.
