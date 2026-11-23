# PennyPinch implementation status

## Current status

**45 of 54 main tasks are complete.** H042 passed its Windows 10 release checks. Windows 11 x64 update and rollback remain open. H041 remains open and deferred. [Task list](../TASKS.md)

PennyPinch v0.1.1-preview.1 for Windows 10 22H2 x64 is available from GitHub Releases. The ZIP includes the installer, the PennyPinch license, and the matching notices. Anonymous downloads matched both asset digests. [Download the Windows preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.1) · [Publication evidence](proofs/pennypinch-windows10-installed-update-and-release.json)

macOS ARM64 passed native desktop development checks with engine 0.41.0. The later 0.44.0 engine and package passed. Native window acceptance for 0.44.0 remains open. The Ubuntu x64 build opened a window. Linux runtime tests and packaging remain open. [Platform status](validation/desktop-platforms.md)

### Windows release evidence

The extracted Windows 10 x64 payload passed a live Codex CLI turn. The provider usage matched, and the engine stopped gracefully. [Evidence](proofs/pennypinch-windows10-packaged-codex.json)

An earlier Windows 10 preview installed on this host. PennyPinch saved the test route. A new ChatGPT CLI used it. Activity showed an earlier record from the run. The built-in verifier stopped at the ancestor configuration guard for this profile. Disconnect restored the test configuration. The application closed normally. [Installed flow](proofs/pennypinch-windows10-installed-codex.json)

The updated Windows 10 installer passed live Codex savings verification in Activity. The local pair and the provider count were verified separately. The original WebSocket feed uses the provider count plus a local delta, so its ratio is historical only. [Measurement](proofs/pennypinch-windows10-activity-measurement.json)

The lead agent prepared the delivery ZIP and verified extraction on Windows 10. [Package record](proofs/pennypinch-windows10-activity-delivery.json)

The current Windows 10 release code verified a local signed update with a temporary key. The verifier rejected the earlier preview's runtime resource digest. The live update URLs returned HTTP 404. [Preflight record](proofs/pennypinch-windows10-update-preflight.json)

The PennyPinch project site serves the update address over HTTPS. The preview channel serves signed updates. [Repository](https://github.com/yukselbuilds/PennyPinch) · [Update address](https://yukselbuilds.github.io/PennyPinch/updates/)

The public README and project site show the benchmark results and measurement scopes. The repository description and topics identify PennyPinch and its client coverage. [Presentation checks](proofs/pennypinch-public-showcase.json)

The local signing key and same-profile encrypted backup passed a signing check. The signed preview channel serves v0.1.1-preview.2. The Windows 10 update manager verified its signature, download, and extracted application. [Signed preview evidence](proofs/pennypinch-windows10-signed-preview.json)

On Windows 10, the real installation upgraded from v0.1.0 with the full installer. Settings then applied the signed v0.1.1-preview.2 package. Rollback restored the previous executable and Windows registration. The test preferences matched across both restarts. The 19,484 recorded resource files matched after rollback. [Installed acceptance](proofs/pennypinch-windows10-installed-update-and-release.json)

The lead agent ran the H044 ripgrep comparison on Windows 10 in 136.4 seconds. The combined route saved two more tokens than Headroom on the first request. Full readback required Headroom recovery and two RTK recalls. H044 waits for a candidate with a useful gain and a verified client recovery path. [Paired evidence](proofs/rtk-1.2.1-rg-paired.json)

The 23 November check found RTK-OSS v1.2.1 is still the latest stable release. The paired evidence applies to that release. [Upstream release](https://github.com/rtk-ai/rtk/releases/tag/v1.2.1)

On 23 November the lead agent confirmed Workers Free in the Cloudflare account dashboard. H052 still needs the controller and transfer checks. Verify the processing terms and any transfer mechanism before accepting real reports. H053 and H054 depend on the live report service. [Privacy decision](decisions/032-usage-sharing-privacy.md)

### Public privacy and presentation

The two public branches and two release tags use the rewritten Git history. GitHub Support accepted the request to remove old cached objects. Confirm removal by anonymous access after Support replies.

GitHub About, README, and Pages now describe separate platform statuses. The live release asset IDs, SHA256 values, and signed update files matched. Secret Protection, push protection, and Dependabot alerts are on.

The new local Windows 10 build and NSIS package passed the build path checks. The installation and release checks for that candidate remain open.

Tokio 1.54.1 passed the Windows 10 product gate and signed update package checks with an isolated key. The real QA app passed signature, resource pin, extraction, and probe validation. [Tokio verification](proofs/tokio-windows-1541.json) · [Public privacy and presentation checks](proofs/pennypinch-public-privacy.md)

OpenTelemetry API 1.45.2 passed the Windows 10 product gate, the staged engine lifecycle test, and a local Responses stream. The local NSIS installer contains the patched package. Existing installations require the full installer for the new engine resources. Linux GTK has a separate update check. [OpenTelemetry verification](proofs/opentelemetry-windows-1452.json)

The local 0.1.1-preview.3 full installer upgraded the public Windows 10 preview. The installed runtime reports OpenTelemetry API 1.45.2. The app kept the test preferences and started and stopped the engine. Publication and Windows 11 checks remain open. [Installed verification](proofs/opentelemetry-windows-installed-upgrade.json)

The local 0.1.1-preview.3 delivery ZIP, external checksum, and release notes are ready. Windows extraction matched 677 internal digests and the installer from H042-ae. The next step is the GitHub prerelease for this full installer. Windows 11 checks remain open. [Delivery evidence](proofs/pennypinch-windows10-preview3-delivery.json) · [Release notes](releases/0.1.1-preview.3-windows10.md)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
