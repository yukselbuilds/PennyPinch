# H019-d1 — Current Codex setting observation

22 October 2026. **H019-d1 local acceptance complete; H019 main task remains open.**

## Scope

Recheck the disk setting associated with an active Codex ownership record using the existing safe config-root and ownership rules. Return fixed states only: matching the saved endpoint, different, missing, unavailable, not owned, or not rechecked for unsupported installations. A disk match does not prove runtime adoption, model traffic, authentication or agent coverage.

The config file and parent-directory handles remain pinned during the overlapping ownership observation. Missing files are observed without creating a file, including temporary transacted files. File contents, paths, URLs, auth literals and parse errors do not enter the response. No auth-store file is opened; bounded config bytes remain transient in memory.

## Accepted evidence

- Backend accepted after root review: 19 Codex status tests PASS; the unchanged config reader retains its 17-test PASS. Regressions cover an initially different config root containing the same URL and held read handles that block writing/renaming. Failed fixture runs are preserved; successful cleanup revalidates its exact UUID root and safe ancestor chain. Exact OS rollback-failure injection is not exercised; its fixed unavailable branch is reviewed.
- Strict TypeScript decoding rejects state/ownership/version contradictions. Correctly correlated Desktop replies cannot claim a checked disk match. The visible TR/EN result distinguishes six fixed outcomes; warnings use the theme warning token and a match uses mint. Neither runtime readiness nor runtime endpoint binding is required for matching the saved owner address.
- Final candidate: **62 Node PASS, 184 Rust PASS/3 ignored**, format, TypeScript/frontend build and embedded `custom-protocol` build PASS. One full gate ran after review and fixes. Previous candidate evidence was invalidated by backend/DTO/UI changes; unchanged focused PASS results were not repeated separately. Build output contained three warning lines, no error.
- Real native embedded app, PID21908, isolated child environment: UI Apply; matching saved address with engine stopped; changed address; missing field; malformed TOML reported unavailable; missing file; restored match. Each of the six status observations preserved config bytes/existence, ownership fingerprint and preferences. UI Undo restored the original config bytes; the released record displayed the quiet not-owned result.
- Open Computer Use used accessibility Invoke and scrolling. Root inspected the actual dark English native layout and the warning/mint treatments. This is not a new physical mouse/keyboard or manual file-picker acceptance. Closing the identity-checked owned process with `CloseMainWindow` produced **exit0**, no force termination, PID0 and stable-port51778 listener0; preferences stayed unchanged.

Final manifest: `work/proofs/codex-setting-check/acceptance.json`; gate reports `frontend-quality.json` and `final-quality.json` contain 62 source/config hashes, command/environment/toolchain details and artifact hashes. Native evidence: `native-observations.json`, `native-run.json`, `native-close.json`, `native-cleanup.json` and `native-matching.png`. EXE16,020,992 bytes, SHA256 `7582b340e23370cb4d3f3a0f16df0ff1cb8bc23a621ad1d4a015d90a2eb3dad0`.

## Limits and next step

Windows10 Pro22H2/build19045, local NTFS only; Windows11 and other operating systems are not covered. No client process, provider/auth request or subagent was executed. The next H019 work remains the product live-verification path and its route/auth/agent acceptance; deferred sign-in is not silently retried. Historical manual selection/rescan acceptance and the unnamed previous Rust failure remain open. The new candidate's single full PASS does not explain that old failure.

Local QA workspace: `work/proofs/codex-setting-check/`. Installation metadata and configs in native QA are synthetic fixtures; they are not evidence of a live client/provider route. The user's sign-in deferral remains in effect. The historical unnamed single Rust failure is documented in [the previous proof](codex-connection.md) and is not marked resolved by this feature.
