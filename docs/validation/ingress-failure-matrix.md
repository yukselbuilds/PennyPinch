<a name="kararlı-giriş-ve-arıza-davranışı"></a>

# Stable ingress and failure behavior

21 October 2026. This is the H040 local acceptance matrix for Windows 10. The standalone relay, real pinned engine, and native UI each have their own evidence. [Architecture decision](../decisions/003-connection-ingress.md) · [local evidence](../proofs/connection-ingress.md).

<a name="rota-ve-auth-sınırı"></a>

## Route and authentication boundary

The Rust ingress forwards TCP opaquely. It does not select the HTTP request, authentication method, model or reasoning fields. It opens connections only to the verified Headroom backend that the app owns. This task does not upgrade an existing client connection plan or catalog entry to a supported status.

| Route | H040 evidence | Direct upstream on engine failure |
| --- | --- | --- |
| Local Responses / synthetic API key | A2 byte sequence, half-close, SSE/no replay. No real provider. | Closed |
| Local Anthropic Messages / synthetic API key | A2 byte sequence and half-close. No real provider. | Closed |
| ChatGPT gateway / OAuth | Not tested through this ingress. | Closed. It does not switch to an API-key route. |
| Claude OAuth / managed auth | Not tested through this ingress. | Closed |
| WebSocket / HTTP2 / HTTPS-to-local | Not tested through this ingress. | Closed |
| CCR history / retrieval from an old generation | Continuity after restart is not proven. | Closed. Do not assume old content exists in the new engine. |
| Model/subagent that uses a separate provider, process or cloud route | Require separate evidence for the relevant client route. | Closed |

Here, “closed” means Headroom added no client fallback. It also means Rust sent no second upstream POST. Verify the client's own retry behavior separately. The opaque relay does not synthesize an HTTP response on error. It closes the connection. Passing local tests does not prove a live model or all subagents.

<a name="arıza-ve-yaşam-döngüsü"></a>

## Failure and lifecycle

| State | Expected behavior | Evidence status |
| --- | --- | --- |
| A process holds the registered endpoint port | Keep the same record. Do not select another address or claim/stop a foreign process. | A1 and manager conflict regression PASS |
| Backend absent or stopping | Close before reading the payload of the new connection. | A2 and the real manager passed. The closed route returned 0 response bytes. A Windows reset is a valid close. |
| Backend PID mismatch or ambiguous peer table | Do not forward payload. Keep the foreign child alive. | A1/A2 PASS |
| New engine generation | Send a new TCP connection to the new generation. Keep an open connection on the old generation. | A2 pinning PASS. Two real pinned generations and native restart used the same endpoint. |
| Partial SSE / backend disconnect | End the partial stream. Do not move it to another backend or resend it. | A2 PASS |
| Delayed reader | Use bounded buffers/partial-write offsets. Preserve byte order within the deadlines. | A2 4MiB fixture PASS. No real long kernel backpressure measurement. |
| Connection limit / idle / total lifetime / drain limit | Bound resource use. A forced close is not graceful. | A2 PASS |
| Engine stop | Close new routes. Request an engine stop that accounts for active requests. Drain TCP within the limit. Clean up owned resources. | Real manager and native idle keep-alive passed. H029 passed pinned gated-SSE normal, stuck, no-replay, and cleanup checks. Active app close and tray were open in H030 at this checkpoint. |
| App close | Check that the owned engine and ingress close. Require proof of cleanup. | Both native runs exited with code 0, with 0 processes and 0 listeners. A new app process opened the same record and address. |
| Old observation / listener loss | Keep endpoint and engine observations separate. Do not count old ready state or unknown counters as success/zero. | TTL/listener Start/poll and post-startup heartbeat regressions PASS |

The relay permits 32 TCP connections by default. It uses a 16 KiB buffer in each direction. Connect and idle limits are 2 s and 300 s. Connection lifetime is 8 hours. The cancellation grace period is 2 s.

The connection counter measures TCP. It does not count active HTTP requests or SSE streams. These times still need measurement for silent SSE across providers.

The final candidate passed 68 Rust tests with 0 failures and 2 opt-in tests ignored. The embedded debug build produced no warnings. The pinned lifecycle opt-in check passed 1 test.

The native proof (local receipt `work/proofs/connection-ingress/native-proof.json`) records real click and keyboard input and an idle-close observation of 1224 ms. The final gate (local receipt `work/proofs/connection-ingress/final-quality.json`) records fingerprints. The timing starts at one control request and applies to that observation.

Windows 11, clean machines, installers, and live model or subagent behavior need separate evidence.

<a name="sonraki-ürün-kapıları"></a>

## Next product gates

H040 targets a stable address across engine restart. When the app closes fully, it releases the listener. Another process can take the port if a client remains connected to that address. Protection after app exit requires H017/H029 connection rollback and H030/H034 tray and background lifecycle evidence. H011/H028 pause and CCR continuity are separate gates.

This matrix has no approved direct fail-open route. A future route needs separate acceptance for the same auth/upstream/model/reasoning, exact state before sending, one POST, cache/CCR and stream limits.
