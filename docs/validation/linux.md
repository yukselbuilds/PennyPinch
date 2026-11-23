# Ubuntu 24.04 x64 validation

H049 passed on the standard Ubuntu 24.04 x64 GitHub runner with ext4 and Xvfb.
The Tauri build generated the AMD64 Debian package with Headroom 0.44.0.
The job compared the installed runtime inventory with the staged inventory.
It checked root ownership and required private user state.
[Package evidence](../proofs/h049-linux-package.json) · [GitHub Actions run](https://github.com/yukselbuilds/PennyPinch/actions/runs/45235051126)

## Reproduce

1. Open the Source checks workflow.
2. Select the source branch.
3. Select `free_only_confirmed` for standard runner compute.
4. Select `linux_package`.
5. Start the workflow.
6. Compare the input fingerprints in the job summary.

## Installed lifecycle

- The native window opened in a fresh profile.
- The engine started, stopped, and restarted through the window.
- The exit action closed the app and the engine. The listeners closed.
- Package purge removed the app files and retained the separate user state.

The job used loopback only. It did not use a live provider or start a coding client.
Other Linux distributions, CPU architectures, display servers, and public release delivery require separate checks.
The earlier full Linux Rust suite recorded 81 failures. This acceptance used focused native checks.
[Platform history](desktop-platforms.md)
