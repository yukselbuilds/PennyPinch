# H017-C2 UI preview

Date: 2026-10-21. Local implementation evidence only; root built-artifact QA and final frontend gate remain pending.

Applications now opens a read-only connection preview for detected installations. Multiple candidates require an explicit choice; a sole candidate is preselected. The preview is created or refreshed only by a user action, and preview requests are invalidated when the selected installation set or discovery generation changes, or when the dialog closes/unmounts. While it is open, discovery mutations are locked. The dialog presents only fixed endpoint/config enums and preservation flags; it does not render installation paths, config fingerprints, secret values, headers, or hook contents.

The dialog scopes the preview to user configuration, states that runtime-effective settings are unknown, distinguishes a missing config from a new-file requirement, shows the fixed blockers/errors and expiry, and keeps Apply disabled until H018. It does not claim that a connection was made, that current sessions adopted the settings, or that main/subagents share a verified route. Browser-only mode still disables Connect.

Focused verification:

- RED: candidate selection defaulted to the first item when several existed, candidate identity depended on array order, and expiry never activated.
- `node --experimental-strip-types --test tests/app/connection-plan-view.test.mjs`: 3 passed.
- `pnpm typecheck`: passed.
- No build, Vite server, or native application was started in this slice; root will run the final built-artifact QA and frontend gate.

Environment: Windows 10 Pro 22H2 x64 build 19045, Node.js `v25.2.1`, pnpm `11.25.0`. No dependency or package script changed. The C1 typed bridge and Rust planner were not modified. The Rust format-pass metadata in `work/proofs/connection-plan/format-pass.json` now records the raw `rustc --version` string; its 22 recorded Rust-source/config hashes were checked and still match.

Final source/config SHA-256:

| File | SHA-256 |
| --- | --- |
| `src/features/integrations/ConnectDialog.tsx` | `0a1d6d26c640765b8fc34e06235acfc2b422df68d6daf9f85b4257fff3e8030c` |
| `src/features/integrations/Applications.tsx` | `fc459b1fe37cfa4ed49131810d51805659002a03866db98ec58b10b01c858a31` |
| `src/lib/connectionPlanView.ts` | `98ddb618f63ee92b35fe91e2dcb8aa414351747c7f8686a506313ca629fe75d7` |
| `tests/app/connection-plan-view.test.mjs` | `d45db34980c092c5e2301ad7807d4c34612437a7c430ab09187a6cb4ca23dcb1` |
| `src/styles.css` | `50d7d0d888900220eb5cd40029040b807541e09c52d9ac47f8578c63823c5fc2` |
| `src/lib/connectionPlan.ts` | `202c665cfef8b7ec121184b39f5954d156572d1be6ce518f5dbc9f4c4dc37a5f` |
| `src/lib/contracts.ts` | `5a1c04285f43cb31bc00e4f7eab707a62c9b0d729eeae7815c4c4835768e4c6a` |
| `src/lib/integrations.ts` | `55c49ee40521a28f4f1bc1bbf01f447f93fe9eb407eda90f4344bc6ab0bb6cb6` |
| `package.json` | `f3bf9a0cc83b072d0f3d5178588ec00b49ededd026c2c31960570c46e7ac1aad` |
| `pnpm-lock.yaml` | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |

The preview UI test covers singleton/multiple-candidate selection, stable identity across fresh array objects/reordering, candidate-set change, and exact expiry boundary. Keyboard, layout, and native-window behavior require the parent’s static/native QA; this proof does not claim those gates passed.
