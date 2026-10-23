# Source CI refresh — 23 October 2026

**PASS; one manual dispatch.** The previous Ubuntu run covered 49 tests on an older source commit. H019 changed the source and expanded the suite to 64 tests, so that historical CI result did not cover the current candidate. The user authorized the needed refresh.

Before dispatch, the normal GitHub browser session showed GitHub Free, **1/2,000 included minutes**, **0 GB/0.5 GB storage**, Actions billable usage **$0**, and the existing account-scoped Actions **$0 budget / Usage blocked**. No billing/auth/settings change was made. The unchanged workflow SHA256 was `4b3a90b38e7b3b4a4bd3503f51b4c0fab8649ed6ccb50a8e4525f333ab619e87`: manual/default-false guard, one standard Ubuntu 24.04 job, ten-minute limit, pinned tools/actions, no cache or artifact upload.

[Source checks #2](https://github.com/Protocol272/headroom-desktop/actions/runs/39922540641) ran on `main`, commit `7722777d5efa620f8406c90a3ffd761fc3063fcc`, tree `24b900662fa556a3a813d82bde0fae712c2aa083`, at 11:08 Europe/Istanbul. The actual run summary showed **Success, 64 tests passed, frontend build passed**, **29 seconds total / 25 seconds job**, and no artifacts. A transient browser timeout after dispatch was resolved by reading the same run from a fresh tab; no second dispatch or rerun was made.

After the run, the account display showed **2/2,000 included minutes**, **0 GB/0.5 GB storage** and **$0 billable usage**; one additional included minute was consumed. Displayed gross usage and discounts both totaled $0.05. This is a dated account observation, not a future quota reservation. Every later dispatch still requires a fresh free-only check.

Scope: source-only frontend on the hosted Ubuntu runner. This does not establish Linux/macOS native product, Windows11, installer, engine, provider/auth or agent support. The existing local H019 evidence and its limits remain separate; H019 is still open. Only documentation/evidence records change after this tested source, so the successful source gate is not repeated for those records.

Local ignored evidence: `work/proofs/free-ci/refresh-2026-10-23/before-dispatch.json`, `result.json`, `after-run.json`, `ci-success.png`. Screenshot SHA256 `fcce63649aef3884e01cf9473d5cb949d056dd8b70238d3f8847356736e9a92e`. Raw logs, account pages, auth data and payment details were not saved.
