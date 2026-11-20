# PennyPinch development package

This is an unsigned development preview. It is not a signed production release.

The Windows engine payload includes Headroom (`headroom-ai` 0.44.0), Copyright
2025 Headroom Contributors, under Apache-2.0. Its original LICENSE and NOTICE
are preserved at `runtime/payload/Lib/site-packages/headroom_ai-0.44.0.dist-info/licenses/`.
The pinned CPython 3.13.13 distribution's license is preserved at
`runtime/payload/LICENSE.txt`. Python dependency metadata, license files and
notices remain in their original `.dist-info` and package directories.

The payload is copied without modifying vendor source. PennyPinch's own
controller, hidden helper, cache-control shim and optimization-control overlay
are separate files under `runtime/`; the overlay verifies its upstream source
hashes in `runtime/optimization-sources.json`. They are not vendor replacements.
The original pinned supervisor inside the payload was authored in this project
at `work/proofs/engine-0.44.0/headroom_runtime.py`; it is copied byte-for-byte and
identified by the runner SHA-256 in both source manifests.

The payload, lock identity and exact file-tree inventory are recorded in
`runtime/windows-payload.json` in the source checkout. The built package records
its resource inputs and hashes in `package-inputs.json`. Optional separately
acquired models, services, MCP servers and RTK are not installed by this package.
The existing locked proxy dependency set is retained; its size is reported
separately from the application and WebView2 bootstrapper.

Tauri and its Rust dependencies, React, and the bundled WebView2 prerequisite
retain their respective licenses. A complete application-wide third-party
license inventory and signed publication are still release acceptance work;
this preview notice is not a claim of release readiness.

## Tauri Windows installer template

The NSIS template in src-tauri/windows-installer.nsi is adapted from
Tauri CLI 2.12.3, commit 59b63ab500da9859f12d8888c3298bc29bba0b92,
crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi. Headroom
customizes the uninstall confirmation, delegates connection, startup and
local-data cleanup to its installed executable, and checks for a locked
main executable before removing resources. The template uses the MIT license.

MIT License

Copyright (c) 2017 - Present Tauri Apps Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
