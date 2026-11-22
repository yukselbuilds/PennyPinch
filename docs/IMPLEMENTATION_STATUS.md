# PennyPinch implementation status

## Current status

**45 of 54 main tasks are complete.** H042 passed its Windows 10 release checks. Windows 11 x64 update and rollback remain open. H041 remains open and deferred. [Task list](../TASKS.md)

PennyPinch v0.1.1-preview.1 for Windows 10 22H2 x64 is available from GitHub Releases. The ZIP includes the installer, the PennyPinch license, and the matching notices. Anonymous downloads matched both asset digests. [Download the Windows preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.1) · [Publication evidence](proofs/pennypinch-windows10-installed-update-and-release.json)

The macOS ARM64 engine and package passed their recorded 0.44.0 checks. Native window acceptance remains open. The Ubuntu x64 build opened a window. Linux tests and packaging remain open. [Platform status](validation/desktop-platforms.md)

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

The RTK-OSS v1.2.1 pre-screen rejected all eight Windows 10 cases under the existing H043 checks. H044 awaits an eligible command path. H052 still needs the authorized report service. H053 and H054 depend on H052. [RTK pre-screen evidence](proofs/rtk-1.2.1-preflight.json) · [Task list](../TASKS.md)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
