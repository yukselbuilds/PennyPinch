# Desktop platform validation, 19 November 2026

The initial checks below used commit 10f32e3. The macOS and Linux checks in the table used CircleCI Free. Windows 10 22H2 x64 is the first release target. Windows 11 x64 verification is pending.

| Environment | Verified coverage | Open acceptance criteria |
| --- | --- | --- |
| Windows 10 22H2 x64, build 19045 | The current NSIS package installed. All 19,484 resources matched the receipt. The native window opened, and the 0.44.0 engine answered a local HTTP request. The app exited normally and the test installation was removed. | A clean Windows environment without WebView2, real client traffic against the installed candidate and download from the release URL require further verification. |
| macOS 15 ARM64, CircleCI M4 Pro | The 0.44.0 engine passed a local traffic test. The ARM64 application bundle was built and its ad hoc signature and contents were verified. | The application process started. AppleScript timed out with −1712 on two attempts to inspect the window. The native window and engine lifecycle checks remain open. [Run 5](https://app.circleci.com/pipelines/circleci/KoDRDU6JmR6wiWsrCamPDH/b460f300-344a-4d50-8849-edc9e66c82b8/5/details) |
| Ubuntu 24.04 x64, CircleCI | The frontend and Rust builds passed. The window opened under Xvfb. The Rust suite recorded 220 passes, 81 failures, and 2 skipped tests. [Run 6](https://app.circleci.com/pipelines/circleci/KoDRDU6JmR6wiWsrCamPDH/b460f300-344a-4d50-8849-edc9e66c82b8/6/details) | Later checks covered installation and the engine lifecycle. [Linux validation](linux.md) |

## Linux configuration reads, 23 November 2026

The Ubuntu 24.04 x64 standard runner passed four configuration tests and opened PennyPinch under Xvfb.
The tests used isolated Codex, Claude Code, and OpenCode settings and compared the original file bytes.
They also checked file replacement, unsafe paths, and the permissions of the application-owned directory.

The read guard permits local ext-family, Btrfs, and XFS filesystems. This run verifies the Ubuntu runner's filesystem.
The transaction has separate verification. [Transaction evidence](../proofs/h049-linux-transaction.json)
[Configuration evidence](../proofs/h049-linux-config.json)

## Linux direct engine verification, 23 November 2026

Ubuntu 24.04 x64 ran the pinned Headroom 0.44.0 wheels with CPython 3.13.13 and ONNX Runtime 1.31.0.
The runner moved the payload before start. The engine completed two synthetic Responses streams.
The test checked the Unix socket peer PID and UID before it sent HTTP.
The engine stopped after each run and left no socket. The payload fingerprint matched before and after.
The network namespace exposed only loopback. The following check covers the desktop manager.
[Engine evidence](../proofs/h049-linux-engine.json)

## Linux managed engine and client configuration

The Ubuntu 24.04 x64 test ran the pinned engine from the desktop manager.
It checked the saved loopback port, socket peer identity, pause, resume, restart, and orderly stop.
The native window opened under Xvfb and reserved the saved endpoint.
The connection test used simulated installation records and real configuration files.
The saved Codex address carried one synthetic Responses stream.
The app applied the route in each profile. Undo restored the original file bytes and permissions.
[Managed engine evidence](../proofs/h049-linux-managed.json)

## Linux CLI discovery

The standard Ubuntu 24.04 x64 GitHub runner found Codex, Claude Code, and OpenCode in an isolated home and PATH.
The test used pinned Linux x64 npm executables. It checked unsafe paths, duplicate links, version hashes, and file changes.
The app reopened saved choices. Separate tests checked concurrent writers and process exit during publication.
The Linux app requires ext4 for selection and client configuration writes.
The Windows 10 product gate passed. The macOS ARM64 rerun awaits runner capacity. [H049-b4 evidence](../proofs/h049-linux-discovery.json)

The Ubuntu 24.04 x64 package passed installation, engine control, exit, and package purge.
Real client processes and live provider traffic require separate checks. [Linux validation](linux.md)

<a name="windows-10-kurulumu"></a>

## Windows 10 installation

Verification covered the current source and 234 Windows build inputs. The prior staging receipt described older overlay files. Two Python bytecode files that had been added to the payload were archived, and the payload was staged again from the pinned inventory. The installer rejected a destination longer than 100 UTF-16 units. Installation to a separate valid destination and normal exit passed.

The installer is 123,653,119 bytes. Its SHA-256 is 9725ecfd72005185542119684558f4fd27733a90acd746d7d05e291219cd64eb. The gate recorded 227 frontend and 478 Rust passes with 299 matching inputs. It skipped 24 opt-in Rust tests. [Build receipt](../../work/proofs/windows-install/runs/10dc9e1a-1db8-4450-9326-305c5d3a8181/recovered-build.json) · [Local quality gate](../../work/proofs/windows-install/runs/10dc9e1a-1db8-4450-9326-305c5d3a8181/quality-gate.json) · [Installation result](../../work/proofs/windows-install/runs/9bf2f318-01b9-4348-b810-b9c084d32453/result.json) · [Cleanup](../../work/proofs/windows-install/runs/9bf2f318-01b9-4348-b810-b9c084d32453/cleanup.json)

## CircleCI

The GitHub App was installed only on the private repository at its then-current name, Protocol272/headroom-desktop. The Free plan initially showed 30,000 available credits. The pipeline runs on manual dispatch. Its temporary tag trigger was removed and the default platform parameter leaves jobs idle. The first failures were a missing Rust tool on macOS and a configuration parsing error on Linux. Pinning the tool and fixing the parser let both environments reach the application checks. [Pipeline configuration](../../.circleci/config.yml)
