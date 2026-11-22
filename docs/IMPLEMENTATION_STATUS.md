# PennyPinch implementation status

## Current status

**45 of 54 main tasks are complete.** H042 is the active local release task. H041 remains open and deferred. [Task list](../TASKS.md)

The unsigned Windows 10 x64 preview includes Headroom 0.44.0 and the verified Codex Activity measurement. The delivery ZIP contains the matching notices. PowerShell verified 676 listed file digests. [Windows 10 delivery evidence](proofs/pennypinch-windows10-activity-delivery.json)

The macOS ARM64 engine and package passed their recorded 0.44.0 checks. Native window acceptance remains open. The Ubuntu x64 build opened a window. Linux tests and packaging remain open. [Platform status](validation/desktop-platforms.md)

### Next H042 release step

The extracted Windows 10 x64 payload passed a live Codex CLI turn. The provider usage matched, and the engine stopped gracefully. [Evidence](proofs/pennypinch-windows10-packaged-codex.json)

An earlier Windows 10 preview installed on this host. PennyPinch saved the test route. A new ChatGPT CLI used it. Activity showed an earlier record from the run. The built-in verifier stopped at the ancestor configuration guard for this profile. Disconnect restored the test configuration. The application closed normally. [Installed flow](proofs/pennypinch-windows10-installed-codex.json)

The updated Windows 10 installer passed live Codex savings verification in Activity. The local pair and the provider count were verified separately. The original WebSocket feed uses the provider count plus a local delta, so its ratio is historical only. [Measurement](proofs/pennypinch-windows10-activity-measurement.json)

The lead agent prepared the delivery ZIP and verified extraction on Windows 10. [Package record](proofs/pennypinch-windows10-activity-delivery.json)

The current Windows 10 release code verified a local signed update with a temporary key. The verifier rejected the earlier preview's runtime resource digest. The live update URLs returned HTTP 404. [Preflight record](proofs/pennypinch-windows10-update-preflight.json)

The public repository is named PennyPinch. Its Pages site serves the new update address over HTTPS. The signed feed is pending. [Repository](https://github.com/yukselbuilds/PennyPinch) · [Update address](https://yukselbuilds.github.io/PennyPinch/updates/) · [Rename checks](proofs/pennypinch-repository-rename.json)

The public README and project site show the benchmark results and measurement scopes. The repository description and topics identify PennyPinch and its client coverage. [Presentation checks](proofs/pennypinch-public-showcase.json)

Next, set the permanent update signing key. Build the first Windows installer with its public key and the verified HTTPS URL. Windows 11 update and rollback and clean-host installation have separate acceptance. [Update acceptance](validation/windows-update.md) · [Installer acceptance](validation/windows-install.md#open-acceptance-criteria)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
