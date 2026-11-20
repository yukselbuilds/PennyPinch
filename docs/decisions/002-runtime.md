<a name="002--yerel-masaüstü-mimarisi"></a>

# 002 — Local desktop architecture

The decision was recorded on 19 October 2026. H012 finalized the architecture and compatibility contract on 15 November. Implementation and support scope depend on the evidence below. [H012 acceptance](../proofs/architecture-acceptance.md)

<a name="seçim"></a>

## Selection

Use a Tauri 2 / Rust manager, React / TypeScript UI, and an app-owned CPython 3.13.13 + `headroom-ai[proxy]==0.41.0` runtime directory. Target Windows 10 22H2 and Windows 11 x64. Bundle the UI locally. On Windows it uses the system WebView2. Current manifests pin Tauri 2.12.4, React 19.3.1, TypeScript 7.0.2, and Vite 8.3.4. Dependencies are resolved by Cargo/pnpm lockfiles. These are versions in the local candidate, not a claim that they are the latest.

Keep this component selection. Do not add another desktop framework or general-purpose helper service. Tauri responses use `schema_version=1`, and production helper stdio JSONL messages use `protocol_version=1`. These two version fields are independent. Request and error envelopes are specific to each command family. The [local contract registry](../contracts.md#8-uygulanan-ipc-kaydı-14-kasım-2026) maps the current 39 Tauri commands to source. H006-a's historical helper and the production controller/helper used in H014/H028 are not the same evidence. The Graphite UI uses real backend state, zero/unknown metrics, and separate account/session views.

H006-a proved the portable runtime directory and the console-free parent → hidden-console helper → engine lifecycle. Historical payload size was 523,394,231 bytes. H037 verified local installation. H006-b verified controller-owner loss while ready. H006-c verified packaged GUI loss and reopening. H006-d verified that a controller whose owner exits before Job assignment exits when its input pipe closes. Main Windows acceptance awaits two separate clean Windows 10 and Windows 11 x64 environments. [Windows installation record](../validation/windows-install.md) · [Controller Job result](../proofs/windows-engine-crash.json) · [Packaged GUI result](../proofs/windows-gui-crash.md) · [Owner loss during startup](../proofs/windows-engine-startup-crash.json).

<a name="süreç-ve-güven-sınırı"></a>

## Process and trust boundary

- The WebView invokes only narrow Tauri commands. It is not granted general shell access, proxy management through arbitrary URL/port/path, or execution of remote pages. The UI does not receive engine debug/log objects containing content or secrets.
- The Rust manager tracks the helper handle/PID and loopback ownership for its own runtime generation. Helper/manifest version is validated. Do not publish `ready` until listener and pinned readiness agree. Do not claim or close a port occupied by another engine.
- Helper control uses size-bounded stdio JSON messages. Engine logs are reduced separately to content-free state codes. A JSON line in stdout alone is not identity/readiness evidence.
- One Rust manager owns one runtime generation. Each request/response is correlated by `request_id`, and engine responses by `runtime_generation`. An unknown snapshot is not `ready`. The fixed UI error envelope and initial states are defined in the [local contract](../contracts.md). Raw child-process output does not enter the UI DTO.
- The application state root is separate from the runtime payload. Fixed bundle files are not modified. Connection settings and user preferences are written durably/atomically. Existing sign-in secrets are not copied into app state.
- Normal shutdown starts with a stdio request and bounded drain. Timeout/Job Object fallback applies only to owned processes. Forced close is not reported as graceful. The forced result from H009's test controller does not invalidate the H006-a helper approach and does not prove the real Tauri connection.
- H007's opaque-block cache-marker adaptation has been carried into the production shim. Package version, manifest, controller/helper, shim/control, and full `prefix_tracker.py` hash guards are checked in the [Rust startup path](../../src-tauri/src/runtime/manager.rs). The engine does not become ready on mismatch. H014 production binding and H028 pause/CCR evidence are local Windows 10 acceptance. Silently changing the package/manifest or patching without guards is not accepted. Do not assume the same acceptance for another OS, new package, or live provider.

<a name="istemci-adaptörleri"></a>

## Client adapters

Discovery, change planning, apply, verification, and undo are separate operations. Discovery does not edit files. The plan proposes only required routing fields while preserving the active model/reasoning/auth method and scope. Recheck the fingerprint at apply time. Undo must not overwrite later user changes.

Client version, surface, auth, protocol/transport, and capability are separate compatibility dimensions. Initial records are kept with version and evidence in [`compatibility/clients.json`](../../compatibility/clients.json). Success with an API fixture is not promoted to live OAuth or desktop/subagent support. Failure to find an installation does not mean unsupported. Label unverified combinations accurately. The app does not silently change model/effort/agent count.

Do not rely on routing keys ignored in Codex project config. Process/user scope and the existing ChatGPT gateway route are verified separately. Claude bare/API fixture and OpenCode V1/V2 are not the same support record. Automatically restarting open user sessions or copying auth files into a new fixture is not an integration method.

OpenCode 1.19.3 and 2.3.0 CLI are recognized on Windows by a pinned file digest. V1 and V2 use the same connection/ownership manager. V2 plans and receipts carry optional `opencode_schema:"v2"`. Absence of the field preserves the old V1 record. Eligible V2 `providers` endpoints and `mcp.servers.headroom` are bound in one atomic operation. Version, schema, user permissions, and retrieval-bridge eligibility are separate gates. [H010 acceptance](../proofs/opencode-acceptance.md), [route and record contract](009-opencode-provider-routes.md).

<a name="duraklatma"></a>

## Pause

Stopping the engine, removing the client connection, and pausing compression are different operations. Pause must let the current stream finish, keep the historical CCR store accessible, and preserve the prior prefix. Do not assume the contract is met merely by applying `HEADROOM_OPTIMIZE=0`, a bypass header, or a proxy restart.

H011-a source review is a historical record in [`pause.md`](../proofs/pause.md). Pinned upstream bypass/runtime-env/restart controls do not replace safe pause. In H028, a production control module was connected to real pause/resume through generation/revision and a private acknowledgement. Active HTTP/SSE and WebSocket, old prefix/CCR, and new compression after resume were [validated with local replay](../proofs/safe-pause.md). If control cannot be read while the engine is running, state is `unknown`. If the engine is unavailable, it is `unavailable`. Only acknowledged state is shown as paused. This Windows 10 fixture evidence is not live-provider or all-OS support.

<a name="ölçüm"></a>

## Metrics

The main card shows `Σ(B-A)` and `100×Σ(B-A)/ΣB` from the same scope. Content-free field selection, same-tokenizer basis, event deduplication, runtime restart, and missing intervals are part of the normalization contract. Do not copy raw `/stats` percentages before validating their product meaning. See the [metrics note](../research/metrics-0.41.0.md).

Request-level wire reduction, unique-content gain, novel content/cache, and cost use different bases. Do not equate an estimated bill with the primary token metric. Do not infer unattributed project/agent/model fields. A report for the panel does not require an LLM call.

<a name="uygulama-ve-yayın-sırası"></a>

## Implementation and release order

Local product slices H013–H036, H038–H040, H045, and H050–H051 are implemented. The evidence covers the H014 helper connection, H015–H018 state, discovery, planning and transactions, H019–H021 client connections, H023–H027 content-free metrics and attribution, and H028 safe pause individually. H050 account records and H051 session metadata are separate capabilities. Live routing is not inferred from account presence, nor traffic/savings from chat/model information. Local signed update/recovery is in H042. Production key/source and release remain separately open.

H012 accepts this component, process, IPC, record, metrics, and compatibility contract. H008's Windows 10 ChatGPT CLI/Desktop evidence and H010's local OpenCode CLI/TUI/CLI-configured Desktop acceptance are recorded within their own scopes. The catalog contains 14 clients, 1 capability, and 3 environment gates. Live Claude/OpenCode auth, API-key, clean Windows/Windows 11, and production release acceptance remain open in their support and release gates.

K3 requires architecture decisions to be grounded in concrete evidence. This contract can be finalized because unknown combinations remain visible in the catalog. H048's macOS 15.8.2 Apple Silicon result does not automatically verify new account/session capabilities. H049 and H052–H054 are deferred by user decision. [H012 closure record](../proofs/architecture-acceptance.md) binds the current product/catalog and final quality-gate fingerprints.

<a name="h006-c--gerçek-gui-sahibi-kaybı-için-yerel-kabul-planı"></a>

## H006-c — Local acceptance plan for loss of the real GUI owner

**Local slice accepted 15 November.** Packaged GUI loss and reopening passed under the scope/threshold set in advance. [Result](../proofs/windows-gui-crash.md). H006 main task is not complete.

15 November 2026. The lead agent owns process/settings ownership and acceptance. Unlike H006-b's controller fixture, this validates unexpected loss of a real packaged Tauri window. Clean OS, Windows 11, the race before Job assignment, and production release are out of scope. The main H006 box remains open.

Build the current source and staged runtime in release mode with a separate application identity. The test profile is only under `%LOCALAPPDATA%/dev.headroom.desktop.gui-crash-<id>`, owned by this identity. Evidence and WebView profile are under project `work/proofs/windows-gui-crash/`. Do not connect to real client settings. Use the native UI Start → real manager → packaged controller/helper/engine Ready path. Do not change CLI/model/auth/agent settings. Provider endpoints belong only to the local rejecting fixture.

Keep the observer's external Job handle open throughout the experiment. This Job is cleanup only. Do not close it to manufacture the observed success. Apply TerminateProcess only to the GUI's own held Child handle. Observe the three previously opened engine process handles become signaled and the engine/fixed ingress ports close within 5000 ms. A foreign listener must continue receiving its expected witness bytes. The experiment cannot PASS if the external Job is closed before natural shutdown is observed. This GUI experiment does not distinguish EOF from Job effects. That mechanism distinction belongs in H006-b's separate control arm.

Reopen with the same isolated identity. The window must be Stopped, retain the fixed endpoint identity/port, and support an explicit Start → Ready → Stop → normal Exit. The final check covers file locks after crash and restart state. All owned processes/ports must close. Fixture model request count must be zero. One crash/recovery run is not general reliability or a latency benchmark.

Verification uses `bench/windows/gui_crash.py` and the native WebView driver. Record source, both benchmark files, fully staged payload, executable/dist, tools, and Windows fingerprints with the result. Reuse the final H009 full quality gate only if its 344 unchanged product inputs and tool/OS inputs still match. If relevant product code changes, run focused validation and one full local gate on the final candidate. Preserve failed runs with fixed reason/metadata. Raw output or secrets do not belong in the report.

<a name="h006-d--job-atamasından-önce-sahip-kaybı"></a>

## H006-d — Owner loss before Job assignment

**Local control complete.** Both arms passed with shutdown observations of 147 ms and 8 ms. [Test result](../proofs/windows-engine.md#h006-d--job-atamasından-önce-sahip-kaybı-15-kasım-2026).

15 November 2026. The lead agent owns the local test because it concerns process ownership. The real packaged controller is paused for testing immediately after the production `spawn` call and before Job assignment and the `start` message. This control point exists only in the Rust test build. The goal is to verify that the input pipe closes on sudden owner loss and that neither helper nor engine starts in that interval.

In both arms, forcibly terminate only the test owner's held process handle. In the first arm, the controller must exit with code 2 within five seconds after owner loss and produce no protocol output. In the second arm, keeping the observer's write end open must keep the controller alive for at least one second longer. Once the last write end closes, the same five-second bound applies. Keep the external observer Job open while observing the result. Initial members are limited to the test owner, controller, and observed Windows `conhost.exe` processes. The cumulative Job count must not increase after this point, and no process may remain at the end. The original assumption of only two total processes was corrected after the two additional Windows console-host processes' real identities were confirmed.

The opt-in test runs against a fixed Windows stage. Record source/package/test-command/result/tool/OS fingerprints in the same step. Production processes, user settings, and model traffic are out of scope. Separate clean Windows 10 and Windows 11 x64 environments remain open in main H006 acceptance.
