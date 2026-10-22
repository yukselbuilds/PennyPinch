# H047 — Free-only CI evidence

22 October 2026. **Main task accepted. H047-a/b/c complete;15/49 main tasks complete.** H019 has not started.

## A — Account gate, read-only

Root used the existing GitHub browser session for Protocol272. The repository remains private. Billing Overview showed **GitHub Free**, Actions **0 min used / 2,000 min included**, and Actions storage **0 GB used / 0.5 GB included**. The displayed Actions billable amount after discounts was **$0**. This is an observation of account-wide remaining allowance, not a reservation for Headroom.

Budgets and alerts already contained an account-scoped **Actions $0.00** budget with **Usage blocked** displayed. Its edit form showed budget amount **0**, **Stop usage when budget limit is reached checked**, and **Spending won't exceed your set budget**. It also required a valid payment method before creating/adjusting the budget. Root inspected the existing values and exited with Cancel; no budget, payment, auth, subscription or other repository setting changed. The budget applies to paid usage; actual availability of an included-quota job is checked separately by H047-c, without increasing the budget if GitHub blocks the run.

Observation was22 October2026 at12:44 Europe/Istanbul. Local screenshot `work/proofs/free-ci/actions-budget-before.png` is ignored and contains only the observed budget form. No payment-card details or raw account page is committed.

GitHub documents [included Actions allowance and blocked usage without a valid payment method](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [budget product/scope and metered hard stops](https://docs.github.com/en/billing/concepts/budgets-and-alerts), and [budget configuration](https://docs.github.com/en/billing/how-tos/set-up-budgets). Sources opened22 October2026. These controls are conditional on the account settings remaining in place; this task does not promise permanent or independently reserved allowance.

The accepted next stage was a small manually dispatched source check on a standard hosted Ubuntu runner, under this existing account gate; its result is recorded in C below. No paid/larger runner, overage, new payment method, Actions cache, artifact upload, Codespaces, real provider request or live client test. macOS/Linux native product support and paid Windows11 x64 verification remain outside this gate.

## B — Fresh tracked-source archive

Root reviewed the four-file source-gate change (package script, pinned Node file, README and scope document). A new Git-index archive, tree `6c4bd5074091bc3807969f7388fe8e5aff4a5b68`, contained146 tracked files and no `work`, `node_modules`, `dist` or `.git` inputs. Root extracted it beneath a new project-owned run directory. It reused only the existing content-addressed package store, not installed modules or engine/proof helpers.

On the Windows10 host, Node25.2.1/pnpm11.25.0: frozen install **PASS6.06s**; `pnpm test:app` **PASS1.22s** (six explicit test files,49 test declarations, no skip/todo declarations); `pnpm build` **PASS1.90s**. All146 tracked file SHA-256 values remained unchanged after the gate. The test process exited0; the local QA parser expected TAP and did not capture Node25's default reporter counts, so no runtime count field is inferred from that empty field. The source selection and exit status establish the accepted test gate. No command was repeated solely to change its reporter.

Local metadata and file hashes: `work/proofs/free-ci/runs/199ed853-0263-43c3-adf4-70cd8683ed17/acceptance.json` (ignored). Raw command output was held in memory and not persisted. Prior H018 Rust/native evidence remains unchanged; its frontend evidence was refreshed because the package test entry changed and this gate uses a fresh source archive. This result is frontend-only and makes no native engine, live client, installer, macOS or Linux support claim.

## C — Workflow review and actual run

Reviewed `.github/workflows/source-checks.yml` SHA-256 `4b3a90b38e7b3b4a4bd3503f51b4c0fab8649ed6ccb50a8e4525f333ab619e87`: manual-only trigger; default-false boolean guard; one standard `ubuntu-24.04` job; ten-minute timeout; read-only contents permission; duplicate-ref cancellation; immutable checkout/setup-node commits; credential persistence and automatic package-manager cache disabled. pnpm11.25.0 is bootstrapped inside project-owned `work/ci-tools`, and package stores remain under `work/`. No artifact/cache upload, secrets, provider request or paid/larger runner. Output is captured in shell memory; only fixed labels and a recognized numeric test count are logged.

Root parsed the YAML, checked its policy fields and validated the extracted script with Git Bash `--noprofile --norc -n`: **PASS**. All25 relevant source-gate input SHA-256 values still match the fresh archive's accepted manifest; frontend tests/build are reused, not repeated for documentation/workflow-only edits. Local review metadata: `work/proofs/free-ci/workflow-review.json` (ignored). The actual run below established the intended source-only gate.

Root committed the reviewed source as `a7cf6bab24e3122735b5e65c93dce7968a0f7556` and pushed through GitHub Desktop's existing account session. Desktop reported push complete; local HEAD equaled origin/main. No CLI token, SSH key, credential or billing configuration was changed. Root manually dispatched the workflow on `main` after refreshing the current allowance and existing hard-stop observation. The default-false confirmation was explicitly checked; no automatic trigger was added.

[Source checks run #1](https://github.com/Protocol272/headroom-desktop/actions/runs/39759872053) **SUCCESS**, manually triggered22 October2026 at13:01 Europe/Istanbul. Commit `a7cf6bab24e3122735b5e65c93dce7968a0f7556`; total workflow duration29s, frontend job21s. The real runner's setup log showed Ubuntu24.04.5 LTS. Fixed step labels confirmed pinned pnpm bootstrap/version, frozen install, **App tests: passed (49)** and **Frontend build: passed**. Artifacts: none. The generated summary explicitly scopes acceptance to frontend source, excluding engine/native runtime. Screenshot: `work/proofs/free-ci/ci-run-success.png` (ignored).

Post-run account observation22 October2026 at16:31 Europe/Istanbul: GitHub Free; **1 min used /2,000 min included**, **0 GB used /0.5 GB included**, Actions **$0 billable usage after discounts**. The account-scoped Actions budget still showed **$0.00 /$0.00 spent** and **Usage blocked**. No billing setting was changed. The account-wide displayed usage increased from0 to1 minute; this is the panel observation, not a claim of finer-grained per-run invoice attribution. Job duration is not rounded billing minutes. These controls and remaining allowance must be checked again before any future dispatch; the workflow checkbox does not query billing.

**H047 accepted and stopped.** H019 has not started; H048/H049 native platform work remains pending after H019. Windows11 paid x64 validation is skipped/not_tested. Local Windows frontend and Ubuntu frontend success do not establish a working installer, engine packaging, live Codex/Claude/OpenCode/model/subagent traffic, macOS/Linux native compatibility or release readiness. Development token savings were not measured.
