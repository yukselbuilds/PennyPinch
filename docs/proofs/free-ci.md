# H047 — Free-only CI evidence

22 October 2026. **Main task open. H047-a/b accepted; H047-c workflow/run acceptance pending.** H019 has not started.

## A — Account gate, read-only

Root used the existing GitHub browser session for Protocol272. The repository remains private. Billing Overview showed **GitHub Free**, Actions **0 min used / 2,000 min included**, and Actions storage **0 GB used / 0.5 GB included**. The displayed Actions billable amount after discounts was **$0**. This is an observation of account-wide remaining allowance, not a reservation for Headroom.

Budgets and alerts already contained an account-scoped **Actions $0.00** budget with **Usage blocked** displayed. Its edit form showed budget amount **0**, **Stop usage when budget limit is reached checked**, and **Spending won't exceed your set budget**. It also required a valid payment method before creating/adjusting the budget. Root inspected the existing values and exited with Cancel; no budget, payment, auth, subscription or other repository setting changed. The budget applies to paid usage; actual availability of an included-quota job is checked separately by H047-c, without increasing the budget if GitHub blocks the run.

Observation was22 October2026 at12:44 Europe/Istanbul. Local screenshot `work/proofs/free-ci/actions-budget-before.png` is ignored and contains only the observed budget form. No payment-card details or raw account page is committed.

GitHub documents [included Actions allowance and blocked usage without a valid payment method](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [budget product/scope and metered hard stops](https://docs.github.com/en/billing/concepts/budgets-and-alerts), and [budget configuration](https://docs.github.com/en/billing/how-tos/set-up-budgets). Sources opened22 October2026. These controls are conditional on the account settings remaining in place; this task does not promise permanent or independently reserved allowance.

Allowed next stage: a small manually dispatched source check on a standard hosted Ubuntu runner, under this existing account gate. No paid/larger runner, overage, new payment method, Actions cache, artifact upload, Codespaces, real provider request or live client test. macOS/Linux native product support and paid Windows11 x64 verification remain outside this gate.

## B — Fresh tracked-source archive

Root reviewed the four-file source-gate change (package script, pinned Node file, README and scope document). A new Git-index archive, tree `6c4bd5074091bc3807969f7388fe8e5aff4a5b68`, contained146 tracked files and no `work`, `node_modules`, `dist` or `.git` inputs. Root extracted it beneath a new project-owned run directory. It reused only the existing content-addressed package store, not installed modules or engine/proof helpers.

On the Windows10 host, Node25.2.1/pnpm11.25.0: frozen install **PASS6.06s**; `pnpm test:app` **PASS1.22s** (six explicit test files,49 test declarations, no skip/todo declarations); `pnpm build` **PASS1.90s**. All146 tracked file SHA-256 values remained unchanged after the gate. The test process exited0; the local QA parser expected TAP and did not capture Node25's default reporter counts, so no runtime count field is inferred from that empty field. The source selection and exit status establish the accepted test gate. No command was repeated solely to change its reporter.

Local metadata and file hashes: `work/proofs/free-ci/runs/199ed853-0263-43c3-adf4-70cd8683ed17/acceptance.json` (ignored). Raw command output was held in memory and not persisted. Prior H018 Rust/native evidence remains unchanged; its frontend evidence was refreshed because the package test entry changed and this gate uses a fresh source archive. This result is frontend-only and makes no native engine, live client, installer, macOS or Linux support claim.

## C — Workflow review, run pending

Reviewed `.github/workflows/source-checks.yml` SHA-256 `4b3a90b38e7b3b4a4bd3503f51b4c0fab8649ed6ccb50a8e4525f333ab619e87`: manual-only trigger; default-false boolean guard; one standard `ubuntu-24.04` job; ten-minute timeout; read-only contents permission; duplicate-ref cancellation; immutable checkout/setup-node commits; credential persistence and automatic package-manager cache disabled. pnpm11.25.0 is bootstrapped inside project-owned `work/ci-tools`, and package stores remain under `work/`. No artifact/cache upload, secrets, provider request or paid/larger runner. Output is captured in shell memory; only fixed labels and a recognized numeric test count are logged.

Root parsed the YAML, checked its policy fields and validated the extracted script with Git Bash `--noprofile --norc -n`: **PASS**. All25 relevant source-gate input SHA-256 values still match the fresh archive's accepted manifest; frontend tests/build are reused, not repeated for documentation/workflow-only edits. Local review metadata: `work/proofs/free-ci/workflow-review.json` (ignored). The configuration has not yet established a successful GitHub Actions run.
