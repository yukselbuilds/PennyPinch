# Local Application Security Boundaries

Baseline review dated 19 October 2026, before H005. Controls are tested in the relevant implementation slices; this document is not evidence that security has been implemented.

| Boundary / asset | Concrete abuse case | Required control and evidence |
| --- | --- | --- |
| Desktop UI → Rust manager | The UI submits an arbitrary command, file path, or remote URL | Narrow typed commands; paths and arguments chosen by the manager; a CSP that does not execute remote content; no general shell permission. |
| Manager → engine listener | Another HTTP service on the same port is mistaken for Headroom or terminated | Child-process handle/PID, loopback ownership check, expected readiness body; fail or choose another port if occupied; stop only the process we own. H005. |
| Client settings → adapter | A user change made after setup is deleted during disconnect | Calculate the proposed change first; atomic write; ownership and fingerprint for changed fields; on conflict, do not overwrite the whole file with an old backup. H017–H018. |
| Existing credentials → proxy → provider | Credentials leak into logs, state, UI, or the wrong upstream | Preserve the existing authentication flow; fixed, allowed provider targets; no arbitrary routing; forward only required headers; content-free reporting. H008–H010. |
| Engine → metrics/storage | Prompt or tool output enters activity records or exports | Allowlisted metadata fields; filter content and credentials; bound size and duration; never pass unknown fields to UI/logs. H023–H027, H033. |
| Original CCR content | Original content disappears unexpectedly when the process closes or compression is paused | TTL and ownership; preserve the same retrieval store and active streams while paused; explicit deletion and shutdown semantics. H007, H011, H028. |
| Package source → installation | Wrong package, modified engine, or unexpected dependency | Pin `headroom-ai`; lock/hash and official source; only required extras; package licenses; run in a clean environment. H005–H006, H037. |
| Application exit → open client | An open session routed through the proxy is silently disconnected | Minimize to the tray; show active connection/stream information; safely remove owned settings or clearly explain the effect of exiting. H029–H030, H038. |

The first technical experiment does not change existing client settings or require a real provider request. Hidden values are not read and copied into diagnostic output. Raw logs from failed tests are not retained as evidence.

Tauri sidecars and Windows packaging mechanisms are candidate approaches; Headroom's portability must be proven separately. Sources: [Tauri sidecars](https://v2.tauri.app/develop/sidecar/), [Windows installer](https://v2.tauri.app/distribute/windows-installer/). Last reviewed: 19 October 2026.
