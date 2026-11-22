# PennyPinch implementation status

## Current status

**45 of 54 main tasks are complete.** H042 is the active local release task. H041 remains open and deferred. [Task list](../TASKS.md)

The Windows 10 x64 preview includes Headroom 0.44.0. The delivery ZIP contains an unsigned installer. Extraction verified 676 file digests. [Windows 10 delivery evidence](proofs/pennypinch-windows10-delivery.json)

The macOS ARM64 engine and package passed their recorded 0.44.0 checks. Native window acceptance remains open. The Ubuntu x64 build opened a window. Linux tests and packaging remain open. [Platform status](validation/desktop-platforms.md)

### Next H042 release step

The extracted Windows 10 x64 payload passed a live Codex CLI turn. The provider usage matched, and the engine stopped gracefully. [Evidence](proofs/pennypinch-windows10-packaged-codex.json)

The current Windows 10 preview installed on this host. PennyPinch saved the test route. A new ChatGPT CLI used it. Activity showed an earlier record from the run. The built-in verifier stopped at the ancestor configuration guard for this profile. Disconnect restored the test configuration. The application closed normally. [Installed flow](proofs/pennypinch-windows10-installed-codex.json)

The updated Windows 10 installer passed live Codex savings verification in Activity. The local pair and the provider count were verified separately. The original WebSocket feed uses the provider count plus a local delta, so its ratio is historical only. [Measurement](proofs/pennypinch-windows10-activity-measurement.json)

Next, bind the Windows 10 delivery ZIP and notices to the updated installer. [Measurement contract](decisions/010-metrics-normalization.md)

Production updates require the release key and the live HTTPS update source. Windows 11 update and rollback and clean-host installation have separate acceptance. [Update acceptance](validation/windows-update.md) · [Installer acceptance](validation/windows-install.md#open-acceptance-criteria)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
