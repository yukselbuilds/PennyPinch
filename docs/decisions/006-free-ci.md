# Decision 006 — Free-only GitHub and CI preparation

**Date:** 22 October 2026  
**Status:** Accepted user constraint; CI design is deferred to H047.

## Decision

GitHub work must remain free-only. The repository's default visibility is private, and the first import is source-only. Repository creation and publishing are to be done in GitHub Desktop using its existing sign-in flow. Local Git history and commits do not require a GitHub CLI token; the invalid CLI token will not be refreshed for this task. This preparation has not created or published a repository.

Until H047 verifies the applicable free allowance and a no-charge stop, do not add or run GitHub Actions. If H047 verifies those conditions, standard GitHub-hosted runners within the included allowance are permitted for manually dispatched (`workflow_dispatch`) checks. This can include macOS/Linux native jobs; a local Mac/Linux host is not required. Paid or larger runners, overage spending, new payment methods, metered caches, Codespaces, and a separate cloud-macOS subscription remain prohibited. No payment method, workflow, or account billing setting was changed here. CI has not been configured or run.

Because the initial repository is private, Actions minutes and storage depend on the account plan and may be billed beyond included usage. The account's plan, remaining quota, payment configuration, and spending controls have not been inspected. Therefore no automated matrix is authorized by this decision until H047 verifies the relevant account conditions and documents a hard stop against charges. If a no-charge hard stop cannot be established, CI remains disabled; local checks and manually run native validation remain the path.

H047 should proceed in ordered slices: verify plan/quota/hard-stop; make intended source-only checks reproducible without hidden `/work/` imports; then design and verify the smallest CI entry points. Prefer short source checks and no metered cache. Retain artifacts only when necessary and for the shortest supported period. macOS/Linux native jobs may be manually dispatched on standard GitHub-hosted runners only if the verified quota and hard-stop ensure no charge; otherwise leave them blocked/not tested. The paid Windows 11 x64 validation is explicitly skipped/not tested under the free-only constraint; it must not be represented as a passing platform. H048 and H049 may begin only after the Windows first working slice and H047's quota probe.

## Basis and limits

GitHub's current documentation says standard hosted runners in public repositories are free, while private repositories use plan-dependent included minutes and storage and can incur charges beyond the allowance. It also lists standard hosted macOS and Linux runners for private repositories, charged against the account's included allowance ([GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [choosing a runner](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job)). GitHub documents spending controls for eligible metered-product budgets, with behavior depending on budget scope ([budgets and alerts](https://docs.github.com/en/billing/concepts/budgets-and-alerts), [setting up budgets](https://docs.github.com/en/billing/how-tos/set-up-budgets)). These official pages were opened and checked on 22 October 2026.

This is a policy decision, not an account audit. It does not claim that this account has free minutes available, that a budget is configured, or that any workflow currently has a guaranteed zero-cost route. H047 owns that verification. No exact quota amount is recorded here.
