# H017-C2 review fixes

Date: 2026-10-21. This addendum records the first UI review changes; the original C2 proof remains unchanged as the historical pre-review candidate.

The dialog no longer exposes the H018 task number. Its disabled Apply note says that this version provides a preview only. Endpoint labels now use the precise fields `env.ANTHROPIC_BASE_URL` and `provider.openai.options.baseURL`. The comparison values use readable label/value rows, and the missing-file note sits outside the definition list. Repeated scope/session warnings were consolidated into a short scope note and two session/route boundary statements; Turkish copy uses “Sağlayıcı” and “Kimlik bilgisi tanımlı”.

Focused validation: `pnpm typecheck` passed with Node.js `v25.2.1` and pnpm `11.25.0`. The 3 helper tests from the immediately preceding candidate were not rerun: `src/lib/connectionPlanView.ts` and `tests/app/connection-plan-view.test.mjs` are unchanged, and the parent explicitly asked to reuse that matching PASS. No build, Vite server, or native window was started. Root built-artifact QA remains pending.

SHA-256 for the final reviewed UI source and relevant unchanged contracts:

| File | SHA-256 |
| --- | --- |
| `src/features/integrations/ConnectDialog.tsx` | `d9851cb3355648d96fe5b373b47aa044fb48320e6dc37cc31f63e95d060919a0` |
| `src/styles.css` | `bd42c46c8062077a689ad7518469b65fba09dbc8d28b26946cd27f6576166e27` |
| `src/features/integrations/Applications.tsx` | `fc459b1fe37cfa4ed49131810d51805659002a03866db98ec58b10b01c858a31` |
| `src/lib/connectionPlan.ts` | `202c665cfef8b7ec121184b39f5954d156572d1be6ce518f5dbc9f4c4dc37a5f` |
| `src/lib/contracts.ts` | `5a1c04285f43cb31bc00e4f7eab707a62c9b0d729eeae7815c4c4835768e4c6a` |
| `src/lib/connectionPlanView.ts` | `98ddb618f63ee92b35fe91e2dcb8aa414351747c7f8686a506313ca629fe75d7` |
| `tests/app/connection-plan-view.test.mjs` | `d45db34980c092c5e2301ad7807d4c34612437a7c430ab09187a6cb4ca23dcb1` |
| `package.json` | `f3bf9a0cc83b072d0f3d5178588ec00b49ededd026c2c31960570c46e7ac1aad` |
| `pnpm-lock.yaml` | `b62e8ab3c31462abbd9075b626e390a24198901baa963947b44f8e337f99952b` |

The Rust format metadata’s raw compiler-version correction remains recorded separately in `work/proofs/connection-plan/format-pass.json`; no Rust source changed in this UI review fix.
