# H016 native picker/rescan diagnosis addendum

Date: 2026-10-20. The reported native sequence is: a manually chosen
`opencode.cmd` appears as “Selected file”, then disappears after “Scan again”.
That native loss remains an open product bug; this addendum narrows the
backend evidence without claiming the real dialog path has been exercised by
the tests below.

The exact root-owned no-op fixture described in the native QA manifest was
read-only inspected: it remained zero bytes on a fixed local D: volume, and
the file and its parent directories had no reparse-point attribute. It was not
modified or executed. New isolated unit fixtures under fresh guarded UUID
directories established that a zero-byte `opencode.cmd` selection survives
both an empty-input rescan and `DiscoveryManager::scan()` using this process's
bounded local discovery inputs. The opt-in real-environment test reported only
counts and fixed metadata: before 1, after 2, generation 2, retained 1,
`manual_file`, and aggregate `candidate_location_unavailable=true`. That
warning can belong to another candidate and cannot be attributed to the
selected file. The test does not invoke COM or the live WebView command.

No renderer DTO, picker behavior, user configuration, client executable, or
provider state was changed. Backend flow inspection shows the selection and
scan commands clone the same managed `DiscoveryManager`; a successful
selection stores a canonical path privately, and scan revalidates that path
before adding the candidate to the new snapshot. The new trace is intended to
make that decision observable on a debug build. It uses only the fixed
`headroom_discovery` prefix, event names, integer counts/generations, and the
fixed rejection reasons `missing`, `unsafe`, `unavailable`, and `filename`.
It never logs a path, client kind, environment value, version, request ID,
address, token, or secret. The trace is compiled out when
`debug_assertions` is disabled. Public DTOs and release behavior are unchanged.

## Verification

On the same Windows 10 22H2 x64 host and project-local Cargo cache/target as
H016-c1, with rustc/Cargo `1.92.0`:

| Command | Result |
| --- | --- |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml integrations::discovery_tests::real_environment_scan_retains_zero_byte_cmd_picker_candidate -- --ignored --nocapture` | PASS on the pre-trace source, reused without rerun as requested; 1 opt-in bounded local-environment scan, no client launched |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml integrations::discovery_tests::` | PASS on final trace source, 15 passed, 1 opt-in test ignored |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS |
| `cargo check --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol` | PASS |

Cargo used project-local `work/caches/cargo` and
`work/caches/cargo-target`. The generated zero-byte test fixtures were cleaned
only within their new UUID roots after containment and reparse checks. Existing
frozen diagnostic roots were untouched. No native picker/app was launched as
part of this diagnosis.

## Fingerprints

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `src-tauri/src/integrations/discovery.rs` | 43,323 | `df5a3137d0b96c7b8884b829a162fc2a187d581aa4e0e976669d7b6d13e60c11` |
| `src-tauri/src/integrations/discovery_tests.rs` | 28,066 | `7e306509471ca4bb49756460c960ef0e7bcc5359bc48fdd00d5273936c62e02c` |
| `src-tauri/src/integrations/picker.rs` (unchanged) | 4,811 | `c6dfb9bcbd707eff80883df3f800f1643d202e02dbfb6ba68e2e837b5aed0f20` |
| `src-tauri/Cargo.toml` | 1,101 | `8eae7ff57df5ff48be9c629a0250c17813abe0c0af1e8ba86add9d3abd130096` |
| `src-tauri/Cargo.lock` | 104,391 | `f4e0081f60dc2dba489f0133d4cf4068e75632f7076cfb97a77858355931d140` |

The aggregate local-environment test does not identify which candidate caused
the unavailable-location warning. The live dialog's returned path and the
native application's scan response still need bounded observation to locate
the actual loss. The debug trace has not yet been observed in a newly launched
native application. No production behavior change was made to guess at that
cause.
