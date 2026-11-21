<a name="h012-mimari-ve-uyumluluk-kabulü"></a>

# H012 Architecture and compatibility acceptance

15 November 2026. The review reconciled H012's component, process, adapter, measurement, and support contracts with the Rust and TypeScript sources at that checkpoint. [Programmatic acceptance](architecture-acceptance.json), [review](../reviews/H012.md), [runtime decision](../decisions/002-runtime.md), [interface contract](../contracts.md).

| Acceptance item | Observed result |
| --- | --- |
| Shell and engine selection | Tauri 2.12.4, React 19.3.1, TypeScript 7.0.2, Vite 8.3.4, CPython 3.13.13, and Headroom 0.41.0 match their manifests. |
| IPC and trust boundary | The document recorded each of the 39 Tauri commands in that source once. The runtime, account, session, update, and export families keep their separate envelopes. |
| Adapter and persistent records | The document covers the V1/V2 plan and receipt split, shared `opencode_v1` slot, V2 `mcp.servers.headroom`, and permission gate. |
| Measurement basis | The document separates the real field/status names for period and request totals, the empty runtime placeholder, and source collection status. |
| Support catalog | The review verified the identity, status, and evidence target of 14 client, 1 capability, and 3 environment records. The catalog represents unknown combinations. |
| Layer scope | Headroom protocol routing and terminal-filter measurements are separate. No accepted integration record exists for RTK-OSS 1.1.0. |

The review linked H005–H011 technical evidence and H050/H051 account/session decisions by their hashes. H010's new main acceptance made the current OpenCode scope precise. External-environment/account gaps for H006/H009 and release tasks remain open in support status.

Normal acceptance checks that command and support records match. It stops on a repeated/missing command, an unrecognized status, missing evidence, wrong dependency version, or source/catalog drift. It checks that local links in the document point to real files in the project.

This pass changed only documents and acceptance records. The product/catalog fingerprint matched the final local gate's 344 inputs. The team reused evidence for 222 Node/462 Rust tests, 19 ignored tests, fmt/frontend/embedded build. It recorded hashes from before the document change separately from hashes during verification.

The saved acceptance command is below. At that checkpoint the validator read the Turkish task file as UTF-8. The evidence preserves the first validator and its hash.

```powershell
& 'D:/Projeler/Headroom/work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I -B 'D:/Projeler/Headroom/work/proofs/architecture-contracts/close.py' check acceptance-1794738683856465400
```

H012 acceptance is an interface/architecture contract. No new live provider, operating system, installer, or production release experiment ran. The user's H049 and H052–H054 deferrals remain. Development token savings were not measured.
