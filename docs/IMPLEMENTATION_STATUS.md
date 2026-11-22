# PennyPinch implementation status

## Current status

**45 of 54 main tasks are complete.** H042 is the active local release task. H041 remains open and deferred. [Task list](../TASKS.md)

The Windows 10 x64 preview includes Headroom 0.44.0. The delivery ZIP contains an unsigned installer. Extraction verified 676 file digests. [Windows 10 delivery evidence](proofs/pennypinch-windows10-delivery.json)

The macOS ARM64 engine and package passed their recorded 0.44.0 checks. Native window acceptance remains open. The Ubuntu x64 build opened a window. Linux tests and packaging remain open. [Platform status](validation/desktop-platforms.md)

### Next H042 release step

The first-use priority is to verify the current Windows 10 22H2 x64 installer with real Codex traffic. [Task list](../TASKS.md)

Production updates require the release key and the live HTTPS update source. Windows 11 update and rollback and clean-host installation have separate acceptance. [Update acceptance](validation/windows-update.md) · [Installer acceptance](validation/windows-install.md#open-acceptance-criteria)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
