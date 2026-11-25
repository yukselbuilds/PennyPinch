# PennyPinch implementation status

## Current status

**46 of 54 main tasks are complete.** The Windows 10 preview.7 release is available. The owner deferred Windows 11 x64 verification to December 2026. H041 remains open. [Task list](../TASKS.md)

PennyPinch v0.1.1-preview.7 is available for Windows 10 22H2 x64. The full installer includes Headroom 0.45.0. Both public downloads matched their SHA-256 digests. GitHub Pages links to the new release. [Download the Windows preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.7) · [Publication evidence](proofs/pennypinch-windows10-preview7-publication.json)

Use the full installer to install preview.7. The signed preview channel serves 0.1.1-preview.2 to compatible installations. The three signed feed files matched their recorded SHA-256 digests before and after publication. [Update status](https://yukselbuilds.github.io/PennyPinch/updates/)

The lead agent completed H042-ao on Windows 10. The public preview.5 showed a verification error for the older signed release. The corrected test build showed that the channel had no newer release. All 227 application tests and 481 Rust tests passed. The preview.6 installer carries the fix. [Evidence](proofs/windows-update-older-preview.json)

The lead agent completed H042-ap and H042-aq. The preview.6 installer upgraded preview.5 and kept the saved test preferences. The app found no newer release on the signed preview channel. A local Responses stream and normal exit passed. The public ZIP contains the tested installer and 673 third-party notices. Windows 11 x64 verification remains open. [Installed evidence](proofs/pennypinch-windows10-preview6-local.json) · [Publication evidence](proofs/pennypinch-windows10-preview6-publication.json)

The lead agent completed H042-ar. The Windows 10 Settings window showed the full installer instruction in Turkish and English. A newer signed manifest required a different engine pin. The app dropped the previous candidate and made no installer request. The local gate passed 228 app tests and 482 Rust tests. The published preview.6 predates this change. [Evidence](proofs/windows-update-full-installer.json)

The lead agent completed H042-as. The local preview.7 installer upgraded the public preview.6 on Windows 10. The app kept the saved test preferences. The installed engine completed a local Responses stream. The delivery ZIP contains the tested installer and 673 third-party notices. At this checkpoint, the public Windows 10 release was preview.6. [Local package evidence](proofs/pennypinch-windows10-preview7-local.json)

The lead agent completed H042-at. GitHub Releases serves preview.7 for Windows 10 22H2 x64. The public ZIP matched the installed upgrade, and five preflight cases passed. The signed preview feed serves preview.2 to compatible installations. [Publication evidence](proofs/pennypinch-windows10-preview7-publication.json)

The lead agent completed H042-au on Windows 10 22H2 x64. The local preview.8 installer upgraded the public preview.7 and kept the saved test preferences. The installed engine forwarded the repeated result and recovered the compressed log exactly. The ZIP contains the verified installer and 673 third-party notices. Publication is the next distribution step. [Local package evidence](proofs/pennypinch-windows10-preview8-local.json)

The lead agent ran H037-a9 with the public preview.4 installer and a fresh application profile. The existing Windows 10 22H2 x64 desktop had WebView2. Default preferences, engine start, one local stream, normal exit, and cleanup passed in 73.966 seconds. The test matched 19,491 installed resources and the package receipt. [First-use evidence](proofs/windows-preview4-first-use.json)

The lead agent completed H037-a10 with the public preview.6 installer and fresh test profiles. On Windows 10 22H2 x64, the installed app connected Codex CLI to Headroom 0.45.0. One CLI turn completed against a loopback Responses provider. Undo restored the exact test configuration. Engine stop, normal exit, and cleanup passed. [First-use evidence](proofs/windows-preview6-first-use-codex.json)

Windows 10 is the first-release priority. H006 and H037 need a clean Windows 10 x64 desktop. H037 also needs a host without WebView2. Use the current ZIP and the Windows preview procedure when a suitable host is available. [Procedure](validation/windows-preview-check.md)

The lead agent ran H041-a3 with Headroom 0.45.0 on Windows 10. The same 300-second plan passed 300 request pairs and 249 exact CCR recoveries. Extra p95 latency was 47.87 ms. The engine stopped and released its port. All four fixtures matched the 0.44.0 token counts. [Timed load evidence](proofs/optimization-windows10-stability-045.json)

The source pins Headroom 0.45.0. Windows 10 built the local installer and passed the staged engine lifecycle and loopback usage tests.
Ubuntu 24.04 x64 and macOS 26 ARM64 passed direct engine checks. [Engine verification](proofs/headroom-045-upgrade.json)

The matched Windows 10 benchmarks produced equal counts on both engines in all 33 SDK and eight proxy cases.
In six headline cases, the proxy reduced the count from 122,944 to 69,350 tokens (43.59%).
HTML extraction returned F1 of 0.9168. The tests used local fixtures and zero model calls. [Benchmark evidence](proofs/upstream-benchmarks-045.json).

The lead agent updated the Unix status adapter. The local window build and quality gate passed with 522 matching inputs.
The Ubuntu 24.04 x64 package passed install, start, stop, restart, exit, and purge checks on GitHub Actions.
The macOS 26 ARM64 package passed a controlled Codex Desktop turn, stored local usage, and exited normally. The probe and input comparison finished at the time limit. GitHub marked the overall job cancelled. [Native job](https://github.com/yukselbuilds/PennyPinch/actions/runs/45271792284).
Luna reviewed the upstream sources. Development token savings remain unknown.
[Package and adapter evidence](proofs/headroom-045-managed-packages.json)

The stock Headroom 0.45.0 proxy and PennyPinch forwarded the same JSON in eight matched Windows 10 cases. The six headline cases returned 43.59% fewer tokens in both proxies. The SDK uses a message estimator. [Matched comparison](proofs/upstream-benchmarks-045.json).

Codex CLI 0.171.0-alpha.13.2 opened both synthetic files under the built-in read-only profile on Windows 10. The unelevated backend refused the named profiles before the command started. MXC was unavailable. Resume the named profile check on a compatible native Windows backend. Verify an allowed read and an explicit denial. [File scope evidence](proofs/targeted-query-comparison.json) · [Open H041 criteria](validation/optimization-benefit.md#open-h041-acceptance-criteria)

The lead agent checked Learn with Headroom 0.45.0. Eight previews passed. A local model returned the exact values for two paired report tasks. The advice saved no retries and added 55 tokens. Keep the advice in the opt-in preview. Broader live acceptance needs a real client task with an observed repeat that the advice can prevent. [Learn evidence](proofs/learn-merit-pilot.json)

The lead agent corrected exit-code handling for Codex `exec_command` on Responses. Successful compression keeps the exit code visible. Failed output stays intact. HTTP/SSE and WebSocket passed eight local pairs each on Windows 10. [Shell status evidence](proofs/responses-shell-status-045.json)

The lead agent tested exact Responses repeats with Headroom 0.45.0 and SmolLM2 on Windows 10. In four paired tasks, the full-output arm answered three correctly. The reference arm answered two correctly. Model input fell from 1,420 to 964 tokens. The quality criterion failed. [Evidence](proofs/responses-repeat-local-model-045.json)

The lead agent removed Responses repeat references from the 0.45.0 runtime. Windows 10 passed 52 wire checks, three exact CCR recoveries, 228 app tests, and 482 Rust tests. The local model matched the full-output control on all four answers and token counts. The correction requires a new Windows package. Resume verification from the open H041 criteria. [Correction evidence](proofs/responses-repeat-output-045.json) · [Open criteria](validation/optimization-benefit.md#open-h041-acceptance-criteria)

The preview.5 full installer upgraded the public preview.4 on Windows 10 22H2 x64. It preserved preferences. The test matched 19,491 installed resources and the package receipt. The installed engine passed the successful and failed output checks with the same inputs in both versions. The older installer refused the downgrade. [Installed evidence](proofs/windows-preview5-shell-upgrade.json)

The lead agent prepared the preview.5 delivery for H042-am. The ZIP includes the release notes and 673 third-party notices. Windows matched all 678 listed SHA-256 digests and the tested installer. [Delivery evidence](proofs/pennypinch-windows10-preview5-delivery.json)

The lead agent completed H042-an. The release tag points to the tested source. The downloaded installer matched the Windows 10 upgrade test. Three Pages links pointed to preview.5 at publication. The desktop check script accepted the ZIP on Windows 10. Use the current ZIP and the signed rollback procedure when a Windows 11 x64 desktop is available. [Publication evidence](proofs/pennypinch-windows10-preview5-publication.json)

Ubuntu passed the configuration transaction and recovery checks on ext4. [H049-b2 evidence](proofs/h049-linux-transaction.json)

The lead agent completed H049-b3 on Ubuntu 24.04 x64. The desktop manager ran Headroom 0.44.0 and forwarded a local Responses stream from the saved Codex endpoint.
Apply and Undo passed for isolated Codex, Claude Code, and OpenCode settings. [H049-b3 evidence](proofs/h049-linux-managed.json)

Ubuntu 24.04 x64 passed H049-b4. PennyPinch found the three pinned native CLI files without starting them. The app saved explicit choices and restored them on the next scan. [H049-b4 evidence](proofs/h049-linux-discovery.json)

H049 is complete for Ubuntu 24.04 x64. The installed Debian package passed the native window and engine lifecycle checks. The package is a CI test candidate. Distribution release checks remain separate. [Linux validation](validation/linux.md)

The lead agent verified the H049-b4 regression on macOS 26.7.2 ARM64. The job passed 16 Rust tests, the frontend build, and the locked native build. GitHub cancelled the macOS 15 job before runner assignment. [Regression evidence](proofs/h049-macos-cli-regression.json)

GitHub lists free standard hosted compute for public repositories. Standard Windows x64 runners use Windows Server. The standard Windows 11 desktop runner uses ARM64. Windows 11 x64 acceptance remains open. [Windows acceptance](validation/windows-install.md#open-acceptance-criteria)

### Windows release evidence

H037-a8 passed. The public Windows 10 preview ZIP download resumed at 8 MiB. The complete SHA-256 and 673 indexed license and notice files matched the delivery record. The next H037 check requires a clean Windows desktop without WebView2. [Public delivery evidence](proofs/windows-installer-public-preview-resume.json) · [Open installation checks](validation/windows-install.md#open-acceptance-criteria)

The extracted Windows 10 x64 payload passed a live Codex CLI turn. The provider usage matched, and the engine stopped gracefully. [Evidence](proofs/pennypinch-windows10-packaged-codex.json)

An earlier Windows 10 preview installed on this host. PennyPinch saved the test route. A new ChatGPT CLI used it. Activity showed an earlier record from the run. The built-in verifier stopped at the ancestor configuration guard for this profile. Disconnect restored the test configuration. The application closed normally. [Installed flow](proofs/pennypinch-windows10-installed-codex.json)

The updated Windows 10 installer passed live Codex savings verification in Activity. The local pair and the provider count were verified separately. The original WebSocket feed uses the provider count plus a local delta, so its ratio is historical only. [Measurement](proofs/pennypinch-windows10-activity-measurement.json)

The lead agent prepared the delivery ZIP and verified extraction on Windows 10. [Package record](proofs/pennypinch-windows10-activity-delivery.json)

The current Windows 10 release code verified a local signed update with a temporary key. The verifier rejected the earlier preview's runtime resource digest. The live update URLs returned HTTP 404. [Preflight record](proofs/pennypinch-windows10-update-preflight.json)

The PennyPinch project site serves the update address over HTTPS. The preview channel serves signed updates. [Repository](https://github.com/yukselbuilds/PennyPinch) · [Update address](https://yukselbuilds.github.io/PennyPinch/updates/)

The public README and project site show the benchmark results and measurement scopes. The repository description and topics identify PennyPinch and its client coverage. [Presentation checks](proofs/pennypinch-public-showcase.json)

The local signing key and same-profile encrypted backup passed a signing check. The signed preview channel serves v0.1.1-preview.2. The Windows 10 update manager verified its signature, download, and extracted application. [Signed preview evidence](proofs/pennypinch-windows10-signed-preview.json)

On Windows 10, the real installation upgraded from v0.1.0 with the full installer. Settings then applied the signed v0.1.1-preview.2 package. Rollback restored the previous executable and Windows registration. The test preferences matched across both restarts. The 19,484 recorded resource files matched after rollback. [Installed acceptance](proofs/pennypinch-windows10-installed-update-and-release.json)

The lead agent completed H044-d with the Headroom 0.45.0 payload. In 108 local requests, RTK reduced the first matching request from 156 to 154 tokens. Full readback needed two additional calls and 4,947 more bytes. H044 remains deferred. Resume when an eligible route shows a net benefit and a verified client recovery path. [Current evidence](proofs/rtk-1.2.1-rg-045.json)

The 23 November check found RTK-OSS v1.2.1 is still the latest stable release. The paired evidence applies to that release. [Upstream release](https://github.com/rtk-ai/rtk/releases/tag/v1.2.1)

### Reporting pilot

The lead agent completed the H052 PostgreSQL port because it changes persistent data and quota handling. All 26 report service tests passed in 2.443 seconds. Native PostgreSQL 17.12 tests passed in 5.348 seconds. The lead agent deleted the previous empty pilot database after the owner approved this action. [PostgreSQL evidence](proofs/h052-postgres-port.json)

The lead agent applied H052-h to the Neon Free pilot in Frankfurt. PostgreSQL 18.7 accepted the migration in one transaction. The synthetic storage checks took 177 milliseconds. The catalog confirmed the function matches the migration source. The catalog lists direct grants to the object owners only. The test rolled back all rows. [Neon migration evidence](proofs/h052-neon-migration.json)

The lead agent implemented H052-i because it defines database access. The Neon pilot passed the service role checks. The probe inserted and read synthetic rows, checked denied actions, and rolled back all writes. All 28 local service tests passed. On Node.js 24, the Function bundle passed TLS, retry, and idle disconnect recovery checks against native PostgreSQL. [Role and Function evidence](proofs/h052-neon-runtime.json)

The lead agent deployed the closed H052-j Function on Neon Free in Frankfurt. All four requests returned 404 with no-store. The operator connected to PostgreSQL with TLS. The probe confirmed an empty pilot and a disabled runtime login. The lead agent revoked the project key. Neon showed the key removed after reload. [Live evidence](proofs/h052-neon-closed-function.json)

The lead agent completed H052-j on a temporary branch from the verified empty pilot. The hosted probe returned 204 with a restricted role. PostgreSQL denied three restricted operations. The test denied login after revocation and verified branch removal. Four parent routes returned 404. Neon confirmed removal of the project API key. [Live connection evidence](proofs/h052-neon-restricted-connection.json)

The lead agent implemented H052-k because it controls deletion. All 34 local reporting tests passed. Native PostgreSQL verified TLS and the cleanup role. The first invocation removed 4,000 expired rows. A retry removed the final expired row. The test preserved the rows at the retention boundary. H052-q covers the deployed role and schedule. The earlier Function probe inputs still match. [Cleanup evidence](proofs/h052-neon-retention-local.json)

The lead agent implemented H052-l because it controls administrator access. The local Function uses the Managed Better Auth token contract. It requires the configured issuer, audience, and administrator subject. The packaged handler verified a test session and read a summary over TLS from native PostgreSQL. All 38 local reporting tests passed. H052-p verified the signed token contract on Neon Free. [Administrator access evidence](proofs/h052-neon-admin-local.json)

The lead agent implemented H052-m because it handles browser credentials. The Function bundle passed the login and sign-out flows in isolated Edge. The tests used synthetic provider responses and confirmed same-origin form requests. All 41 local reporting tests passed. The connection probe inputs still match. H052-p verified the browser flow with the real Auth provider. [Browser access evidence](proofs/h052-neon-admin-browser-local.json)

The lead agent implemented H052-n because it controls deletion during recovery. Native PostgreSQL copied synthetic report data from before an erasure. Recovery removed two erased reports, their rate row, and 4,001 expired reports. Two cleanup passes preserved the current record and daily quota. The operator kept report routes closed until cleanup finished. The report service rejected a revoked uploader after reopening. All 43 local reporting tests passed. H052-r covers Neon point-in-time recovery. [Restore evidence](proofs/h052-neon-restore-local.json)

The lead agent completed the H052-o disclosure review on the private report response. The local fixture used 21 synthetic installations. An initial response covered 20 installations overall. One measurement basis exposed one contributor. Two overlapping totals also revealed an exact contribution. Keep the aggregate response private. Real report intake remains closed. [Disclosure evidence](proofs/h052-aggregate-disclosure.json)

The lead agent completed H052-p because it controls administrator access. Edge tested two synthetic users on a temporary Neon Free branch in Frankfurt. The allowed user read the private summary. The other user and an invalid password returned 401. Sign-out removed the browser cookie and closed the summary. The test found zero provider sessions. The lead agent removed the branch and revoked the API key. H052-q covers scheduled cleanup. [Live sign-in evidence](proofs/h052-neon-admin-live.json)

The lead agent implemented H052-q because it controls deletion and trigger authentication. A scheduled call reached the Function over internal HTTP. The previous handler returned 404 before it queried PostgreSQL. The fix accepts the internal call with the trusted Neon header and matching envelope. All 43 local reporting tests passed. On Neon Free, the first call removed 4,000 expired synthetic reports. A retry removed the final expired report after 62 seconds. The cutoff report and current counters remained. The lead agent removed the test schedule and branch and revoked the key. H052-r covers point-in-time recovery. Provider logging, privacy acceptance, and production enrollment remain open. [Live retention evidence](proofs/h052-neon-retention-live.json)

The lead agent ran H052-r because it controls restoration, deletion, and access. Neon Free recovered the synthetic source at its exact pre-erasure write-ahead log position. Two cleanup passes removed two erased reports, one rate row for the erased installation, and 4,001 expired reports. The operator kept report routes closed during both passes. The private test handler connected over TLS with the restricted role. It rejected the old uploader and returned one retained report to the verified test operator. The lead agent revoked the login and project key and removed both temporary branches. H052-s covers the backup inventory and visible logs. Production cutover remains open. [Live restore evidence](proofs/h052-neon-restore-live.json)

The lead agent ran H052-s on the closed Neon Free pilot. The project API returned a six-hour restore window, one branch, and zero snapshots. The backup schedule API returned an empty list. A rejected synthetic upload produced one start record and one end record. Neither record contained the test report identifier, token, or query marker. The lead agent revoked the temporary project key. H052-t covers the public terms and transfer review. Internal retention and account evidence remain open. Real report intake stays closed. [Provider evidence](proofs/h052-neon-provider-retention.json)

The lead agent completed the H052-t public terms review. The current Neon schedule refers to the Databricks agreement and Data Processing Addendum. The schedule lists a US infrastructure subprocessor. The public KVKK rules require a separate transfer basis for regular reports. Confirm the account terms and recipient. Verify the Article 9 safeguard, internal retention, operator, and rights contact before production. Keep real intake closed. [Terms review](proofs/h052-neon-processing-terms.md)

On 25 November, the owner deferred H052, H053 and H054 from the first release. The lead agent recorded the decision. The H052-u inquiry remains a local draft. Keep real report intake closed. Resume the account, retention, and transfer checks before production. [Scope decision](decisions/032-usage-sharing-privacy.md#25-november--first-release-scope)

### Public privacy and presentation

The two public branches and two release tags use the rewritten Git history. On 24 November the GitHub Support portal showed the removal request as Open. The portal showed only the original request. Verify anonymous access after Support replies.

GitHub About, README, and Pages now describe separate platform statuses. The live release asset IDs, SHA256 values, and signed update files matched. Secret Protection, push protection, and Dependabot alerts are on.

The lead agent completed H042-ak. The public ZIP contains the installer that passed the Windows 10 upgrade. The public download pages link to the release. All three signed update files kept their digests. H042 still needs an installed update and rollback on Windows 11 x64. [Publication evidence](proofs/pennypinch-windows10-preview4-publication.json)

The lead agent prepared the H042-al Windows desktop checks. The script reads the OS build, processor architecture, ZIP digest, and WebView2 registration. Six Windows 10 cases and four simulated OS cases passed in 8.822 seconds. Run the current installer and signed rollback procedures on Windows 11 x64 when a desktop is available. [Procedure](validation/windows-preview-check.md) [Preflight evidence](proofs/windows-preview-preflight.json)

Tokio 1.54.1 passed the Windows 10 product gate and signed update package checks with an isolated key. The real QA app passed signature, resource pin, extraction, and probe validation. [Tokio verification](proofs/tokio-windows-1541.json) · [Public privacy and presentation checks](proofs/pennypinch-public-privacy.md)

OpenTelemetry API 1.45.2 passed the Windows 10 product gate, the staged engine lifecycle test, and a local Responses stream. The local NSIS installer contains the patched package. Existing installations require the full installer for the new engine resources. [OpenTelemetry verification](proofs/opentelemetry-windows-1452.json)

The lead agent applied the upstream `glib` fix to the Linux GTK3 dependency. The locked Ubuntu 24.04 x64 build opened PennyPinch under Xvfb. Dependabot closed alert #2 as fixed. [Security verification](proofs/glib-0429-linux-backport.json)

The local 0.1.1-preview.3 full installer upgraded the public Windows 10 preview. The installed runtime reports OpenTelemetry API 1.45.2. The app kept the test preferences and started and stopped the engine. Windows 11 checks remain open. [Installed verification](proofs/opentelemetry-windows-installed-upgrade.json)

The v0.1.1-preview.3 ZIP passed Windows extraction with 677 matching digests. The signed update files kept their recorded digests. [Delivery evidence](proofs/pennypinch-windows10-preview3-delivery.json) · [Release notes](releases/0.1.1-preview.3-windows10.md)

## History

The repository documentation is in English. The archive keeps earlier handoffs, measurements, and their original acceptance limits. [Implementation history](IMPLEMENTATION_HISTORY.md) · [English documentation check](proofs/pennypinch-final-implementation-history-english.json)
