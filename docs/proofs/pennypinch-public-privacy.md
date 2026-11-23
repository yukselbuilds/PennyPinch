# PennyPinch public privacy and presentation checks

23 November 2026. The lead agent checked the public Git refs, old commit access, release assets, and repository protection. Luna drafted the README and site. The lead agent reviewed and verified both drafts before publication.

## Public history and releases

- Rewrote the two published branches and the two existing release tags with git-filter-repo 2.47.0. Verified 363 reachable commits and 3,032 blobs in the rewritten graph. The scan found zero matches for the known private email and workstation account markers.
- The GitHub repository uses a local `noreply` identity for new commits. The rewrite kept the current source tree and Pages tree before the presentation changes. The tag source archives changed only where the privacy rewrite redacted text.
- Checked both release entries and their four assets by ID, name, byte count, and published SHA256. The release filenames and tags match the original records. The live preview manifest, signature, and update installer matched the Git blobs byte for byte.
- An old GitHub commit URL still returned the previous metadata after the ref update. GitHub Support accepted a private request to clear obsolete objects and cached views. Removal remains open until a new anonymous check passes. A local backup is available during the cleanup.

## Repository protection

- The GitHub settings show Secret Protection, push protection, and Dependabot alerts on. This was a public-data and configuration check with a targeted dependency review. A full product security audit has separate scope.
- Gitleaks 8.30.1 scanned the reviewed branches and tags. Its 34 matches consisted of 29 checksums, one public test key, and four test values. The review found no private credentials among those matches.
- The production JavaScript audit reported zero advisories on the current lockfile. Dependabot reported three moderate advisories in Rust and the Python engine lockfile. The registry and lockfile coverage of the two checks differs.
- **Tokio 1.54.0.** The desktop dependency graph used this version during the privacy review. The Windows 10 candidate uses 1.54.1 and passed the local product and signed package checks. [Windows patch evidence](tokio-windows-1541.json) · [Tokio advisory](https://github.com/tokio-rs/tokio/security/advisories/GHSA-5wjr-9356-4wc8)
- **glib 0.18.5.** The Linux GTK3 build uses the upstream fix. The Ubuntu build and window check passed. Dependabot closed alert #2 as fixed. [Linux patch evidence](glib-0429-linux-backport.json) · [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
- **OpenTelemetry API.** The public privacy review found 1.45.1. The Windows 10 candidate contains 1.45.2. The application tests, local engine checks, and installer checks passed. [Windows patch evidence](opentelemetry-windows-1452.json) · [OpenTelemetry advisory](https://github.com/advisories/GHSA-mf5v-w3w3-f6cg)

## Public presentation

The GitHub About box describes an open-source desktop tool and has a cross-platform topic. The README and Pages site give each platform its own test status. Windows 10 22H2 x64 has the public preview. Windows 11, macOS, and Linux have separate release checks.

The candidate check verified 78 links and asset references, four platform labels, and the benchmark counts. Browser checks covered 1920 and 390 pixel widths. Light theme notes reach a minimum contrast ratio of 4.87 to 1 on the tested surfaces. The live HTML and CSS matched their Git blobs. [README](../../README.md) · [Project site](https://yukselbuilds.github.io/PennyPinch/)

## Windows build path check

The published Windows binaries contain the build checkout path from the previous build. The new Windows release script remaps the checkout and user profile paths during Rust compilation. Four flag tests verified argument boundaries and caller settings.

On Windows 10 22H2 x64, the new build produced an NSIS installer in 811.1 seconds. The runtime inventory matched 19,475 files. Two executable checks passed on the built app and again on the app extracted from the installer. The checks cover the GUI subsystem and the absence of both build path prefixes. The extracted app differed only by the three-byte Tauri bundle marker. The input fingerprints matched before and after each check. This evidence covers the local build and package contents.

The Learn benchmark also passed eight local preview checks after the change to the workspace rule. Its live runner accepted the new evidence. This check made zero provider calls.
