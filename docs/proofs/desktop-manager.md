# H014-b local manager proof

This proof covers the Rust runtime manager and its pinned local engine lifecycle on Windows 10 Pro 22H2 x64 (build 19045). The manager owns the consoleless controller in a Windows Job Object, validates the controller/helper/engine ancestry and listener PID, and accepts only the pinned readiness/profile/shim fields. Its command queue and helper frame reads are bounded; snapshots do not wait on lifecycle commands. State and runtime paths are confined to the project in debug builds. No provider request was made.

## Verification

The final focused gate used project-local Cargo paths:

```powershell
$env:CARGO_HOME='work/caches/cargo'
$env:CARGO_TARGET_DIR='work/caches/cargo-target'
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --bin headroom-desktop
cargo check --manifest-path src-tauri/Cargo.toml
```

Result: 14 passed, 0 failed, 1 ignored (the opt-in live lifecycle test); `cargo check` completed without warnings. Toolchain: `rustc 1.92.0 (ded5c06cf 2025-12-08)`, `cargo 1.92.0 (344c4567c 2025-10-21)`. Relevant locked dependencies include `serde_json 1.0.154`, `sha2 0.10.9`, `windows-sys 0.61.2`, and Tauri `2.12.4`. Cargo state used `CARGO_HOME=work/caches/cargo`, `CARGO_TARGET_DIR=work/caches/cargo-target`.

The single live proof command was:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --bin headroom-desktop runtime::manager::tests::live_manager_start_duplicate_start_and_generation_bound_stop -- --ignored --exact --nocapture
```

Result: 1 passed in 20.07 seconds. It started pinned Headroom `0.41.0`, observed the owned loopback listener and exact readiness/profile/shim, waited 6 seconds and saw the same generation remain ready, rejected a stale-generation stop while readiness and listener ownership remained intact, and confirmed duplicate start returned the same generation and port. A correct stop completed through the graceful controller path; the owned engine PID disappeared from the listener table while a separate test-owned listener remained reachable. The test then closed its own listener and removed its UUID-scoped temporary state after manager shutdown was verified.

## Fingerprints

| Input | SHA-256 |
| --- | --- |
| `src-tauri/src/runtime/manager.rs` | `be8a16024caa5db36b10535a37b3851d2a342c564e1044916baddf09ddee34d9` |
| `src-tauri/src/runtime/manager_tests.rs` | `daa7f17a2ed880d7cde5ab76d877e60ecaded7d0989fb46adda8cfd5f5462f35` |
| `src-tauri/src/runtime/mod.rs` | `93f08a8d53f93567647504663fdb6eb867ff5f837871d381bd3f090a25f07497` |
| `src-tauri/src/main.rs` | `6913dd5878e7f0a7421404bc657d024439360dc67d86c009e8bfd933ac252381` |
| `src-tauri/Cargo.toml` | `b2b3b4c237e30fdb0b196b57110383273219b62bb2c58c613dcf6761871d4de4` |
| `src-tauri/Cargo.lock` | `61f5fd42ecdff941fa80f0ade6a16c0611c403106b2f3448a95e68a93ae90319` |
| `runtime/manifest.json` | `e57249e194eb5e5ac8c0396956338c40f35d144b357adb3db826bfb4ce133c57` |
| `runtime/desktop_controller.py` | `a56a82bed8c2878416f99f817f899a294daf5907ba5c21ba53b06a22b2e2537d` |
| `runtime/desktop_helper.py` | `70c0d5b8c52c3f74cb417781c4e1fe60fce942ed6f2375871f90cafbdb1dc398` |
| `runtime/cache_control_shim.py` | `ebc80ca2958b178ed488fb953f1a09cf2dd803b23352dcc3f8f0a644414e30c1` |
| Packaged H006 runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Packaged H006 runner | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Packaged CPython 3.13.13 executable | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Pinned `prefix_tracker.py` | `f1af4ee279f879efc48ddfebbff125a088893bb64ee960ca2dbd4ad1cdb628ee` |

## Boundaries

The successful proof exercised the manager/controller/helper/engine lifecycle from a Rust test process, not from a Tauri GUI process. The hidden helper and graceful Ctrl+Break path worked; the native window close integration is still for H014-c. The force-stop fallback was not triggered in the successful smoke. The shared resolver’s missing-runtime path is covered by a unit test; release bundle resource resolution and installer packaging remain untested. Windows 11 and a clean-machine install were not tested. The frontend has not yet been wired to the new manager commands or the engine error/stop fields; that is also H014-c.
