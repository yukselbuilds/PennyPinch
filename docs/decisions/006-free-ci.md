# Decision 006 — Free-only GitHub and CI preparation

**Date:** 22 October 2026  
**Status:** Accepted free-only constraint; H047 account/source/manual-workflow gates accepted22 October2026.

## Decision

On 23 November 2026, the GitHub API reported PennyPinch as public. GitHub lists standard hosted jobs for public repositories as free, separate from private-repository compute usage. Confirm public visibility and the standard runner label before each manual job. The free-only rule still applies to larger runners, paid storage, and other metered resources. [Current repository](https://github.com/yukselbuilds/PennyPinch) · [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)

### Initial private repository

GitHub work must remain free-only. The repository is private, and the first import is source-only. [Protocol272/headroom-desktop](https://github.com/Protocol272/headroom-desktop) was created/published in GitHub Desktop using its existing sign-in flow on22 October2026; the browser Private label was independently observed. Local Git history and commits do not require a GitHub CLI token; the invalid CLI token was not refreshed. [H046 acceptance](../proofs/repository-import.md).

Until H047 verifies the applicable free allowance and a no-charge stop, do not add or run GitHub Actions. If H047 verifies those conditions, standard GitHub-hosted runners within the included allowance are permitted for manually dispatched (`workflow_dispatch`) checks. This can include macOS/Linux native jobs; a local Mac/Linux host is not required. Paid or larger runners, overage spending, new payment methods, metered caches, Codespaces, and a separate cloud-macOS subscription remain prohibited. No payment method or account billing setting was changed. H047 subsequently configured and accepted one manual source-only Ubuntu workflow; no automatic platform matrix is enabled. [H047 acceptance](../proofs/free-ci.md).

Because the initial repository is private, Actions minutes and storage depend on the account plan and may be billed beyond included usage. H047 inspected GitHub Free and the existing account Actions0USD hard-stop, then observed1/2000 included minutes and0USD billable usage after its first source run. This observation does not reserve quota or monitor later account changes; before each future manual run, recheck current remaining allowance and the hard stop. An automatic matrix is not authorized. If a no-charge hard stop cannot be established, CI remains disabled; local checks and manually run native validation remain the path.

H047 completed these ordered slices: verify plan/quota/hard-stop; make intended source-only checks reproducible without hidden `/work/` imports; then design and verify the smallest CI entry points. Prefer short source checks and no metered cache. Retain artifacts only when necessary and for the shortest supported period. macOS/Linux native jobs may be manually dispatched on standard GitHub-hosted runners only if the verified quota and hard-stop ensure no charge; otherwise leave them blocked/not tested. The paid Windows 11 x64 validation is explicitly skipped/not tested under the free-only constraint; it must not be represented as a passing platform. H048 and H049 may begin only after the Windows first working slice and H047's quota probe.

## Basis and limits

GitHub's current documentation says standard hosted runners in public repositories are free, while private repositories use plan-dependent included minutes and storage and can incur charges beyond the allowance. It also lists standard hosted macOS and Linux runners for private repositories, charged against the account's included allowance ([GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [choosing a runner](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job)). GitHub documents spending controls for eligible metered-product budgets, with behavior depending on budget scope ([budgets and alerts](https://docs.github.com/en/billing/concepts/budgets-and-alerts), [setting up budgets](https://docs.github.com/en/billing/how-tos/set-up-budgets)). These official pages were opened and checked on 22 October 2026.

This policy alone is not a continuing account audit or a permanent zero-cost guarantee. The dated allowance, budget and successful source-run observations are in H047's evidence. Later native macOS/Linux runs still require current free-only checks and their own platform acceptance; none ran during H047.
