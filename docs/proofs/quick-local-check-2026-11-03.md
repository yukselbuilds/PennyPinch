<a name="h023e-kadar-hızlı-toplu-kontrol"></a>

# Quick combined check before H023

3 November 2026. The lead agent ran the combined check at the user's request. The check covered existing tasks. H024 was outside its scope. **The local check passed. Full acceptance for the 23 tasks requires separate evidence.**

| Check | Result |
| --- | --- |
| Current standard gate | 97 app tests and 285 Rust tests passed in the recorded gate. The gate ignored 3 opt-in tests. The team reused these passes because 81 inputs, 3 dist files, the binary, the tools, and the environment matched. The format, frontend build, and native build evidence also matched. |
| New real-engine test | The isolated native manager started, reached Ready, stopped, and restarted. The ingress stayed the same. The restart used a new generation. The manager rejected a stop for the wrong generation and preserved a foreign listener. The test checked closure and cleanup. It sent only local readiness GETs. No provider request occurred. |
| New real-discovery test | An empty manual OpenCode candidate stayed in the real environment scan. The test ran the scan without launching the candidate. No installation, client launch, or application of the user configuration occurred. |
| Opt-in duration | Two tests took 83.5 seconds together. Performance and long-term stability require separate measurements. The filesystem child helper did not run by itself. |
| Runtime integrity | 8 critical manifest, helper, and runtime pins matched. Verification of all 22,225 payload files was outside this check. |
| Task/evidence organization | The task list had 51 main tasks and 20 acceptances. H001–H023 had 17 accepted and 6 open or partial. The check found 66 local evidence links. |

The check corrected two outdated statements. The TASKS.md historical paragraph presented the old 18/51 count as current. The source-checks record still listed eight test files. No product code changed. H024 did not start.

H006 and H008–H012 remain open/partial. Live account and provider checks need separate acceptance. So does coverage of all models and subagents. Separate checks remain for Windows 11, clean installation, macOS, and Linux. The full GUI, pause during an active stream, retrieval, and long-term stability also need separate checks. [H023 scope](metrics-normalization.md), [task status](../../TASKS.md).

Command: `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1 --skip external_filesystem_operation_child`. The command used the existing Cargo and Rust 1.92.0 toolchain. It used the project's offline cache and target and one test thread. It refreshed this opt-in operation check for the current stage. It did not rerun unchanged deterministic standard tests.

Evidence: `work/proofs/quick-local-check/23538ad5-e0de-4d61-b726-cc39f93b4025/{live-check,summary}.json`. The same step recorded results and before/after source/environment fingerprints. The run used no raw error/HTTP/prompt/auth records, paid resources, or CI. Development token savings were not measured.
