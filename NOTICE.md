# Headroom Desktop development package

This is an unsigned development preview. It is not a signed production release.

The Windows engine payload includes Headroom (`headroom-ai` 0.41.0), Copyright
2025 Headroom Contributors, under Apache-2.0. Its original LICENSE and NOTICE
are preserved at `runtime/payload/Lib/site-packages/headroom_ai-0.41.0.dist-info/licenses/`.
The pinned CPython 3.13.13 distribution's license is preserved at
`runtime/payload/LICENSE.txt`. Python dependency metadata, license files and
notices remain in their original `.dist-info` and package directories.

The payload is copied without modifying vendor source. Headroom Desktop's own
controller, hidden helper, cache-control shim and optimization-control overlay
are separate files under `runtime/`; the overlay verifies its upstream source
hashes in `runtime/optimization-sources.json`. They are not vendor replacements.
The original pinned supervisor inside the payload was authored in this project
at `work/proofs/packaging/headroom_runtime.py`; it is copied byte-for-byte and
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
