<a name="h043--rtk-oss-yerel-teknik-değerlendirmesi"></a>

# H043 — RTK-OSS local technical evaluation

10 November 2026. H043 is complete. None of the eight command paths tested with RTK-OSS 1.1.0 on Windows 10 22H2 x64 qualified for H044. H044 waits for a candidate with measurable benefit that preserves output integrity. [Measurement record](rtk.json) · [Client surfaces](rtk-surfaces.md)

The evaluation placed only the Windows x64 OSS ZIP in the project at `work/proofs/rtk/toolchain/1.1.0`. The GitHub tag matched commit `753a3b0e939f2e4490fc24e3a1984010f40a103c`. The archive matched the SHA-256 in the release record and published checksum file. The executed exe matched the exe in the archive. The Apache-2.0 OSS build excludes the RTK engine feature. The standard RTK package and separate proxy were outside this evaluation. [Release assets](https://github.com/rtk-ai/rtk/releases/tag/v1.1.0) · [Build features](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/Cargo.toml) · [Apache-2.0 license](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/LICENSE)

<a name="eşlenmiş-kıyas"></a>

## Matched comparison

Each command ran three times in three new Headroom 0.41.0 processes. The 72 matched command outputs became 288 Responses HTTP/SSE requests. Control received the raw output for the same event. Headroom received the same output passed through the engine. The RTK arm sent filtered output directly. The combined arm passed that output through Headroom.

The final H041 Responses evidence matched 592 inputs. The command output and request body differed from the earlier evidence. This run therefore measured control and Headroom again.

Command bytes mean stdout plus stderr. The evaluation keeps the RTK bytes/4 estimate separate for raw lines. Input tokens count the whole request, including added tool schemas, with the same `o200k_base` tokenizer. Control is the 0% reference for every sample. The combined rate uses control for the same event. A negative rate means that input grew.

| Sample | RTK command-byte reduction | Headroom input reduction | RTK input reduction | Combined input reduction | Reason not accepted |
| --- | ---: | ---: | ---: | ---: | --- |
| `git status` | 5.62% | 85.10% | 10.71% | 85.29% | Below the 10% byte threshold. Exact original output was not recovered. |
| `git diff` | 1.20% | 0% | −0.35% | −0.35% | Diff bytes changed. Exact original output was not recovered. |
| Successful test | 93.09% | 85.13% | 80.49% | 80.49% | The test preserved the count. Recall did not recover the detailed original output. |
| Failed test | 89.61% | 83.15% | 77.84% | 77.84% | stderr dropped from 80 bytes to 0. Recall merged the two streams. |
| Failed build | 96.33% | 88.12% | 87.91% | 87.91% | The test preserved stderr and exit code. Recall did not return the streams separately. |
| JSON with exact numbers | 97.02% | 0% | 91.26% | 91.26% | The filtered output was not valid JSON. Recall did not recover the original content. |
| Source read | 0% | 0% | 0% | 0% | The source-read test preserved Turkish/Unicode and CRLF bytes. No gain. |
| Special stderr and exit 23 | 99.23% | 95.29% | 95.21% | 95.21% | The test preserved stderr and exit code. Recall did not return the streams separately. |

RTK returned the original command's exit code in all 72 events. `rtk bypass` reran the original command and returned all three fields exactly in these deterministic samples. The rerun generated fresh output. It retrieved no record of the earlier run.

In the failed test, failed build, and exit 23 samples, `rtk recall --full` returned a merged stream 27 times. Recall did not provide the original stdout and stderr separately. The failed test's immediate filtered output also lost its stderr field. Headroom's 54 CCR retrievals exactly matched the content received by the engine. In the combined arm, this content was output already filtered by RTK. [RTK retrieval code](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/retriever.rs) · [RTK command execution code](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/runner.rs)

Across the eight warmed samples, the largest added command p95 was 87.14 milliseconds. The corresponding value for the command and loopback request together was 95.55 milliseconds. The thresholds were set in advance at 250 and 1,000 milliseconds. First runs have separate records. The OS disk cache was not reset.

Counters include the first fixture response, total time, and CPU/memory of owned Headroom processes. They exclude RTK process-tree CPU/memory, provider cache, billing, and model quality. No provider call occurred.

<a name="yüzey-ve-kabul"></a>

## Surface and acceptance

The evaluation tested an explicit `rtk git status` call and `rtk bypass` in Windows PowerShell 5.1 without loading a profile. It sent synthetic `PreToolUse` inputs to the real RTK binary for Codex and Claude. It ran early-pass conditions of the fixed TypeScript plugin for OpenCode in Node. These checks do not verify automatic client integration, Desktop, subagents, or other operating systems. See [surface evidence](rtk-surfaces.md).

The first run stopped on a validation error that compared the CCR field with all tool output. A short diagnostic run restored the field and built the full envelope. The final candidate checked each CCR result against the correct scope and separated the session IDs of the two engine arms.

The 661 comparison inputs and 12 surface inputs matched in their own runs. All three engines closed cleanly. Real RTK user directories were absent before and after the runs. Only benchmark, isolated fixture, and evidence files changed. The product quality gate was not rerun. No RTK connection qualified for the client catalog.

<a name="tekrar-üretme"></a>

## Reproduction

The runner needs pinned Python 3.13.13 x64 and the existing Headroom payload. The first command prepares only the pinned OSS package and related sources in the project. Windows has no independent override for the RTK user settings directory. Therefore, the local runner stops if an RTK directory already exists under Roaming or Local AppData. The runner disables telemetry. The DB, recall data, and temporary files stay in the run directory. [Windows directory resolution](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/user_dirs.rs)

```powershell
$rtkProofPython = 'work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe'
& $rtkProofPython -I -B bench/rtk/prepare.py --run
& $rtkProofPython -I -B bench/rtk/replay.py --self-check
$rtkProofRun = & $rtkProofPython -I -B bench/rtk/replay.py --run | ConvertFrom-Json
if (-not $rtkProofRun.passed) { throw 'RTK kıyası başarısız.' }
& $rtkProofPython -I -B bench/rtk/surfaces.py --benchmark-result (Join-Path $rtkProofRun.run 'result.json')
```

<a name="rtk-oss-120--h044-ön-elemesi"></a>

## RTK-OSS 1.2.0 — H044 pre-screen

15 November 2026. The evaluation verified the official stable `v1.2.0` Windows x64 OSS ZIP against its tag commit and two published SHA-256 records. The isolated project run used the existing H043 commands and the same integrity/10% minimum byte-benefit criteria. `retriever.rs` and `runner.rs` matched version 1.1.0 byte for byte. [Release assets](https://github.com/rtk-ai/rtk/releases/tag/v1.2.0) · [Pinned retrieval source](https://github.com/rtk-ai/rtk/blob/e288a18c04452fd26192f59132e5324a8bb6924d/src/core/retriever.rs).

The evaluation rejected all eight commands. Git status reduced bytes by 5.62% and git diff by 1.20%, both below threshold. Source reading preserved bytes and gave 0% benefit. Filtering broke valid JSON structure. Recall did not recover the original successful-test output. Failed-test stderr dropped from 80 bytes to 0. In failed-test, build, and special-stderr samples, `recall --full` merged the original streams. Eight bypasses reran the original command and returned the same exit code and separate streams. [Command results and fingerprints](rtk-1.2.0-preflight.json).

This single-round command pre-screen stops when it finds no eligible candidate. H043's four-arm engine and client evidence stays with its own version. The result is a byte measurement. The 127 source, fixture, tool, and environment inputs matched before and after. RTK data files were in the run directory. H044 installation and product acceptance remain open.

## RTK-OSS 1.2.1 — H044 pre-screen

22 November 2026. The lead agent verified the Windows x64 OSS ZIP against the v1.2.1 release and publisher checksums. The run used the same eight H043 cases and the same 10% minimum byte reduction on Windows 10 22H2 x64. All 129 inputs matched before and after. [Upstream release](https://github.com/rtk-ai/rtk/releases/tag/v1.2.1) · [Measurement record](rtk-1.2.1-preflight.json)

All eight cases had the same byte reductions and rejection reasons as v1.2.0. The successful test output fell 93.09%. The original output had no recall reference. The failed test's stderr fell from 80 bytes to zero. Three recall calls returned stdout and stderr as one stream. The JSON filter also broke JSON structure. All eight bypass calls reran the commands and returned the original streams and exit codes.

The older file-based recovery mode can save successful output. The pinned Windows binary reads that setting from the user's shared RTK config path. The pre-screen used the default mode and project-local data. It measured command bytes. H044 awaits a command path that preserves output integrity and passes the recovery and benefit checks. [Recovery modes](https://github.com/rtk-ai/rtk/blob/d0f0399a5ff8cd1d81325a033f0b2d8864d665f6/src/core/tee.rs) · [Windows config location](https://github.com/rtk-ai/rtk/blob/d0f0399a5ff8cd1d81325a033f0b2d8864d665f6/src/core/user_dirs.rs)
