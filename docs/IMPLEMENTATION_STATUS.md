# PennyPinch implementation status

## Current status

**46 of 54 main tasks are complete.** The Windows 10 preview.4 release is available. Windows 11 x64 update and rollback remain open. H041 remains open. [Task list](../TASKS.md)

PennyPinch v0.1.1-preview.4 is available for Windows 10 22H2 x64. The full installer includes Headroom 0.45.0. Both public downloads matched their SHA-256 digests. [Download the Windows preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.4) · [Publication evidence](proofs/pennypinch-windows10-preview4-publication.json)

The source pins Headroom 0.45.0. Windows 10 built the local installer and passed the staged engine lifecycle and loopback usage tests.
Ubuntu 24.04 x64 and macOS 26 ARM64 passed direct engine checks. [Engine verification](proofs/headroom-045-upgrade.json)

The matched Windows 10 benchmarks produced equal counts on both engines in all 33 SDK and eight proxy cases.
In six headline cases, the proxy reduced the count from 122,944 to 69,350 tokens (43.59%).
HTML extraction returned F1 of 0.9168. The tests used local fixtures and zero model calls. [Benchmark evidence](proofs/upstream-benchmarks-045.json).

The lead agent updated the Unix status adapter. The local window build and quality gate passed with 522 matching inputs.
The Ubuntu 24.04 x64 package passed install, start, stop, restart, exit, and purge checks on GitHub Actions.
The macOS 26 ARM64 package passed a controlled Codex Desktop turn, stored local usage, and exited normally. The probe and input comparison finished at the time limit. GitHub marked the overall job cancelled. [Native job](https://github.com/yukselbuilds/PennyPinch/actions/runs/45271792284).
Luna reviewed the upstream sources. Development token savings remain unknown.
[Package and adapter evidence](proofs/headroom-045-managed-packages.json)

The stock Headroom 0.45.0 proxy and PennyPinch forwarded the same JSON in eight matched Windows 10 cases. The six headline cases returned 43.59% fewer tokens in both proxies. The SDK uses a message estimator. [Matched comparison](proofs/upstream-benchmarks-045.json).

Codex CLI 0.171.0-alpha.13 stopped the named Windows 10 file tests before any command started. The unelevated backend refused the profile. MXC was unavailable. Resume this check on a compatible native Windows sandbox. Check an allowed read and an explicit denial. [Open H041 criteria](validation/optimization-benefit.md#open-h041-acceptance-criteria).

Ubuntu passed the configuration transaction and recovery checks on ext4. [H049-b2 evidence](proofs/h049-linux-transaction.json)

The lead agent completed H049-b3 on Ubuntu 24.04 x64. The desktop manager ran Headroom 0.44.0 and forwarded a local Responses stream from the saved Codex endpoint.
Apply and Undo passed for isolated Codex, Claude Code, and OpenCode settings. [H049-b3 evidence](proofs/h049-linux-managed.json)

Ubuntu 24.04 x64 passed H049-b4. PennyPinch found the three pinned native CLI files without starting them. The app saved explicit choices and restored them on the next scan. [H049-b4 evidence](proofs/h049-linux-discovery.json)

H049 is complete for Ubuntu 24.04 x64. The installed Debian package passed the native window and engine lifecycle checks. The package is a CI test candidate. Distribution release checks remain separate. [Linux validation](validation/linux.md)

The lead agent verified the H049-b4 regression on macOS 26.7.2 ARM64. The job passed 16 Rust tests, the frontend build, and the locked native build. GitHub cancelled the macOS 15 job before runner assignment. [Regression evidence](proofs/h049-macos-cli-regression.json)

GitHub lists free standard hosted compute for public repositories. Standard Windows x64 runners use Windows Server. The standard Windows 11 desktop runner uses ARM64. Windows 11 x64 acceptance remains open. [Windows acceptance](validation/windows-install.md#open-acceptance-criteria)

### Windows release evidence

H037-a8 passed. The public Windows 10 preview ZIP download resumed at 8 MiB. The complete SHA-256 and 673 indexed license and notice files matched the delivery record. The next H037 check requires a clean Windows desktop without WebView2. [Public delivery evidence](proofs/windows-installer-public-preview-resume.json) · [Open installation checks](validation/windows-install.md#open-acceptance-criteria)

The extracted Windows 10 x64 payload passed a live Codex CLI turn. The provider usage matched, and the engine stopped gracefully. [Evidence](proofs/pennypinch-windows10-packaged-codex.json)

An earlier Windows 10 preview installed on this host. PennyPinch saved the test route. A new ChatGPT CLI used it. Activity showed an earlier record from the run. The built-in verifier stopped at the ancestor configuration guard for this profile. Disconnect restored the test configuration. The application closed normally. [Installed flow](proofs/pennypinch-windows10-installed-codex.json)

The updated Windows 10 installer passed live Codex savings verification in Activity. The local pair and the provider count were verified separately. The original WebSocket feed uses the provider count plus a local delta, so its ratio is historical only. [Measurement](proofs/pennypinch-windows10-activity-measurement.json)

The lead agent prepared the delivery ZIP and verified extraction on Windows 10. [Package record](proofs/pennypinch-windows10-activity-delivery.json)

The current Windows 10 release code verified a local signed update with a temporary key. The verifier rejected the earlier preview's runtime resource digest. The live update URLs returned HTTP 404. [Preflight record](proofs/pennypinch-windows10-update-preflight.json)

The PennyPinch project site serves the update address over HTTPS. The preview channel serves signed updates. [Repository](https://github.com/yukselbuilds/PennyPinch) · [Update address](https://yukselbuilds.github.io/PennyPinch/updates/)

The public README and project site show the benchmark results and measurement scopes. The repository description and topics identify PennyPinch and its client coverage. [Presentation checks](proofs/pennypinch-public-showcase.json)

The local signing key and same-profile encrypted backup passed a signing check. The signed preview channel serves v0.1.1-preview.2. The Windows 10 update manager verified its signature, download, and extracted application. [Signed preview evidence](proofs/pennypinch-windows10-signed-preview.json)

On Windows 10, the real installation upgraded from v0.1.0 with the full installer. Settings then applied the signed v0.1.1-preview.2 package. Rollback restored the previous executable and Windows registration. The test preferences matched across both restarts. The 19,484 recorded resource files matched after rollback. [Installed acceptance](proofs/pennypinch-windows10-installed-update-and-release.json)

The lead agent deferred H044. The Windows 10 ripgrep comparison ran for 136.4 seconds. RTK saved two more tokens than Headroom on the first request. Full readback required Headroom recovery and two RTK recall calls. The Windows release ships without RTK. [Paired evidence](proofs/rtk-1.2.1-rg-paired.json)

The 23 November check found RTK-OSS v1.2.1 is still the latest stable release. The paired evidence applies to that release. [Upstream release](https://github.com/rtk-ai/rtk/releases/tag/v1.2.1)

On 23 November the lead agent confirmed Workers Free in the Cloudflare account dashboard. H052 still needs the controller and transfer checks. Verify the processing terms and any transfer mechanism before accepting real reports. H053 and H054 depend on the live report service. [Privacy decision](decisions/032-usage-sharing-privacy.md)

### Public privacy and presentation

The two public branches and two release tags use the rewritten Git history. GitHub Support accepted the request to remove old cached objects. Confirm removal by anonymous access after Support replies.

GitHub About, README, and Pages now describe separate platform statuses. The live release asset IDs, SHA256 values, and signed update files matched. Secret Protection, push protection, and Dependabot alerts are on.

The lead agent completed H042-ak. The public ZIP contains the installer that passed the Windows 10 upgrade. The public download pages link to the release. All three signed update files kept their digests. H042 still needs an installed update and rollback on Windows 11 x64. [Publication evidence](proofs/pennypinch-windows10-preview4-publication.json)

The lead agent prepared the H042-al Windows desktop checks. The script reads the OS build, processor architecture, ZIP digest, and WebView2 registration. Six Windows 10 cases and four simulated OS cases passed in 8.822 seconds. Run the current installer and signed rollback procedures on Windows 11 x64 when a desktop is available. [Procedure](validation/windows-preview-check.md) [Preflight evidence](proofs/windows-preview-preflight.json)

Tokio 1.54.1 passed the Windows 10 product gate and signed update package checks with an isolated key. The real QA app passed signature, resource pin, extraction, and probe validation. [Tokio verification](proofs/tokio-windows-1541.json) · [Public privacy and presentation checks](proofs/pennypinch-public-privacy.md)

OpenTelemetry API 1.45.2 passed the Windows 10 product gate, the staged engine lifecycle test, and a local Responses stream. The local NSIS installer contains the patched package. Existing installations require the full installer for the new engine resources. [OpenTelemetry verification](proofs/opentelemetry-windows-1452.json)

The lead agent applied the upstream `glib` fix to the Linux GTK3 dependency. The locked Ubuntu 24.04 x64 build opened PennyPinch under Xvfb. Dependabot closed alert #2 as fixed. [Security verification](proofs/glib-0429-linux-backport.json)

The local 0.1.1-preview.3 full installer upgraded the public Windows 10 preview. The installed runtime reports OpenTelemetry API 1.45.2. The app kept the test preferences and started and stopped the engine. Windows 11 checks remain open. [Installed verification](proofs/opentelemetry-windows-installed-upgrade.json)

The v0.1.1-preview.3 ZIP passed Windows extraction with 677 matching digests. The signed update files kept their recorded digests. [Delivery evidence](proofs/pennypinch-windows10-preview3-delivery.json) · [Release notes](releases/0.1.1-preview.3-windows10.md)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
