<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/branding/pennypinch-horizontal-dark.png">
    <img src="docs/branding/pennypinch-horizontal-light.png" alt="PennyPinch" width="350">
  </picture>
</h1>

<p align="center"><strong>Local context compression and token tracking for AI coding tools.</strong></p>

<p align="center">
  <a href="https://yukselbuilds.github.io/PennyPinch/"><img src="docs/branding/readme-site.png" width="126" height="38" alt="Project site"></a>
  <a href="https://yukselbuilds.github.io/PennyPinch/#benchmarks"><img src="docs/branding/readme-benchmarks.png" width="130" height="38" alt="Measured input-token results"></a>
  <a href="https://github.com/yukselbuilds/PennyPinch/releases"><img src="docs/branding/readme-releases.png" width="147" height="38" alt="GitHub Releases"></a>
</p>

<p align="center">Windows 10 22H2 x64 preview · <a href="#platform-validation">macOS and Linux status</a> · Apache License 2.0</p>

<p align="center">
  <a href="https://yukselbuilds.github.io/PennyPinch/#benchmarks"><img src="docs/branding/pennypinch-benchmarks.png" alt="Windows 10 local proxy test. Six synthetic requests fell from 122,922 to 69,328 input tokens, a reduction of 43.60%." width="100%"></a>
</p>

PennyPinch is an open-source desktop app for local context compression with Codex, Claude Code, and OpenCode. Connect a supported route to Headroom and see the input token change in Activity. Your client keeps control of its model, reasoning level, and sign-in.

## Download the Windows preview

Windows 10 22H2 x64 is the first release target. The `v0.1.1-preview.3` security preview updates OpenTelemetry and Tokio. Use the full installer to get the patches. This preview uses an unsigned installer.

[**Download the Windows 10 preview**](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.3) · [Update status](https://yukselbuilds.github.io/PennyPinch/updates/) · [Platform validation](docs/validation/desktop-platforms.md)

## What PennyPinch does

- **Connect supported clients.** Scan local installations, review the detected target, and confirm a route. You confirm each connection after discovery.
- **Measure local input changes.** Overview compares before and after input counts when the recorded basis supports that comparison. Missing or mixed measurements remain unknown. A measured zero remains zero.
- **Keep controls explicit.** Pause affects new compression requests. The client continues to control its model, reasoning, and authentication.
- **Keep usage reports local.** PennyPinch stores measurements on your device. You can export a content-free report. The Headroom usage beacon starts off.

See how to connect and review traffic in the [user guide](docs/USER_GUIDE.md).

## Measured input-token results

Each result has a specific workload and counter. Provider usage and billing use separate measurements.

| Test scope | Before → after input count | Result | Counter and limit |
| --- | ---: | ---: | --- |
| Six selected synthetic Chat Completions requests through the Windows desktop proxy | 122,922 → 69,328 | **43.60% lower** | Full request JSON, including tool schemas, counted with `o200k_base`. No model calls. [Request counts](docs/proofs/upstream-benchmarks-044.json) |
| One real Codex CLI Responses WebSocket request through the installed Windows app | 18,081 → 17,870 | **211 fewer · 1.17% local estimate** | One matched local pair from Headroom 0.44.0. The provider reported 10,780 input tokens separately. [Measurement](docs/proofs/pennypinch-windows10-activity-measurement.json) |
| Six selected examples from the Headroom SDK benchmark, rerun locally | 126,289 → 55,209 | **56.28% lower** | The SDK runner's benchmark tokenizer and input scope. The SDK runner measures compression directly. [SDK counts](docs/proofs/upstream-benchmarks-044.json) · [Pinned benchmark source](https://github.com/headroomlabs-ai/headroom/blob/b21479e0fa84db5f3e46bb79f96c962de4d5fdc6/docs/content/docs/benchmarks.mdx) |
| One paired task on a local model server | 6,647 → 6,647 | **0% change** | Same log task and local model on both routes. The model server supplied both counts. [Comparison method](docs/validation/optimization-benefit.md#19-kas%C4%B1m--ger%C3%A7ek-yerel-modelde-e%C5%9Flenmi%C5%9F-chat-completions-k%C4%B1yas%C4%B1) |

<details>
<summary>See the six synthetic proxy cases</summary>

The Windows 10 22H2 run used the packaged Headroom 0.44.0 engine. It counted the full request JSON for six selected test cases. The Python source case increased by 0.27%.

| Synthetic input | Direct | Through proxy | Change |
| --- | ---: | ---: | ---: |
| JSON search results · 100 items | 9,519 | 5,731 | 39.79% lower |
| JSON search results · 500 items | 46,791 | 27,765 | 40.66% lower |
| Structured logs · 100 entries | 8,655 | 5,674 | 34.44% lower |
| Structured logs · 500 entries | 40,516 | 25,931 | 36.00% lower |
| Documentation | 14,499 | 1,277 | 91.19% lower |
| Python source · about 200 lines | 2,942 | 2,950 | 0.27% higher |

[Full counts and method](docs/proofs/upstream-benchmarks-044.json) · [Optimization validation](docs/validation/optimization-benefit.md#20-kas%C4%B1m--headroom-benchmark-tekrarlar%C4%B1)

</details>

## Client coverage

Coverage applies to the recorded version, surface, authentication method, route, and operating system. A chat or subagent appears in measurements when its requests use the verified route.

| Client | Recorded evidence | Limit |
| --- | --- | --- |
| Codex CLI and Desktop | Tests observed ChatGPT traffic on selected Windows 10 versions. A separate 0.44.0 preview measurement covers one live CLI request. [CLI evidence](docs/proofs/codex-native-wait.md) · [Desktop evidence](docs/proofs/codex-desktop-live.md) · [0.44.0 measurement](docs/proofs/pennypinch-windows10-activity-measurement.json) | The CLI parent/child check and Desktop chat evidence have separate scopes. Other accounts, versions, chats, and routes have separate validation. |
| Claude Code 2.1.158 | Windows 10 connection checks used synthetic credentials and a local provider. [Evidence](docs/proofs/claude-acceptance.md) | Live provider authentication and other surfaces require separate checks. |
| OpenCode V1 1.19.3 and V2 2.3.0 | Selected Windows 10 CLI, TUI, and packaged Desktop service routes used synthetic credentials and local providers. [Evidence](docs/proofs/opencode-acceptance.md) | The evidence has separate limits for each version and surface. |

See the [versioned compatibility records](compatibility/clients.json) for status by exact client version and surface.

## Platform validation

| Platform | What the current record verifies | Still open |
| --- | --- | --- |
| **Windows 10 22H2 x64** | The preview package installed. Its resources matched the receipt. The native window opened, Headroom 0.44.0 answered a local HTTP request, and the app exited normally. A Windows 10 installation also verified the published preview update and rollback flow. | Clean-host installation without WebView2, broader installed-client traffic, and complete delivery checks remain separate acceptance items. [Windows install](docs/validation/windows-install.md) · [Update validation](docs/validation/windows-update.md) |
| **macOS 15 ARM64** | Earlier desktop development acceptance used Headroom 0.41.0. With 0.44.0, tests verified engine traffic, the Apple Silicon app bundle, and its ad hoc signature. | The 0.44.0 app process started. AppleScript timed out twice while inspecting the window. Native window and engine lifecycle acceptance remain open. [Platform evidence](docs/validation/desktop-platforms.md) |
| **Ubuntu 24.04 x64** | TypeScript/Vite and Rust builds passed. The application opened a visible window under Xvfb. That run recorded 220 Rust passes, 81 failures, and 2 skipped tests. | Runtime, installer, and engine lifecycle acceptance remain open. [Platform evidence](docs/validation/desktop-platforms.md) |
| **Windows 11 x64** | Independent desktop acceptance pending. | Release validation. |

See the [full platform status](docs/validation/desktop-platforms.md).

## Local data and reports

PennyPinch keeps measurements on the device. Activity and usage exports contain allow-listed request metadata, such as client, model, and token counts. Diagnostic exports omit credentials and raw request content. You start each export. The Headroom usage beacon starts off.

The engine can temporarily keep original compressed content in its local retrieval store so the client can recover it. Your connected client still uses its selected model provider. [Application data boundaries](docs/SECURITY_BOUNDARIES.md)

## License and credits

The PennyPinch desktop application source is available under the [Apache License 2.0](LICENSE). Bundled components keep their respective licenses and attribution in [NOTICE.md](NOTICE.md).

PennyPinch uses the [Headroom compression engine](https://github.com/headroomlabs-ai/headroom). Thanks to the Headroom contributors.

<details>
<summary>Build the frontend and Windows preview</summary>

The app uses Tauri 2, Rust, React, and TypeScript. The source-only frontend checks use the checked-in package and lockfiles. Use Node.js 25.2.1 from `.node-version` and pnpm 11.25.0:

```powershell
pnpm --store-dir work/caches/pnpm install --frozen-lockfile
pnpm test:app
pnpm build
```

`test:app` runs the contract tests listed in the package scripts. `build` checks TypeScript and builds the frontend. The Windows desktop runtime also needs the separately prepared engine. The development manifest resolves `../work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime` relative to its directory. The payload and some historical protocol-helper inputs are under the Git-ignored `work/` directory. See [package scripts](package.json), [source-check scope](docs/validation/source-checks.md), and the [development manifest](runtime/manifest.json).

With the pinned development toolchain and engine payload already prepared locally, build the Windows preview:

```powershell
pnpm.cmd build:windows
```

This verifies and stages the engine, then builds an x64 current-user NSIS preview under `work/caches/cargo-target/release/bundle/nsis/`. The installed app requires WebView2. If it is missing, the embedded bootstrapper needs internet access. The installer rejects destinations longer than 100 UTF-16 units.

The Windows package keeps the engine in `runtime/payload` and mutable app state separately under LocalAppData. Before installer compression, the Headroom 0.44.0 payload contains 19,475 files and 505,117,034 bytes. See the [payload inventory](runtime/windows-payload.json), [vendor notices](NOTICE.md), and [installer validation](docs/validation/windows-install.md).

</details>
