# H017-C2 modal focus regression

Date: 2026-10-21. Root’s Headless Edge 155 built-artifact QA found that Tab could leave the open preview dialog and focus `BODY`; Escape restoration passed and was left unchanged.

While the modal is open, `ConnectDialog` now captures document-level Tab/Shift+Tab and cycles among currently visible, enabled controls. If focus is outside the dialog it returns to the first/last control according to direction; if no control is available it focuses the dialog itself. The listener is removed on cleanup. Escape, backdrop close, and original-focus restoration are unchanged.

`pnpm typecheck`: PASS (Node.js `v25.2.1`, pnpm `11.25.0`). Root should rerun the existing focused Edge artifact regression; no build, Vite server, or native application was started in this fix.

Final source SHA-256:

| File | SHA-256 |
| --- | --- |
| `src/features/integrations/ConnectDialog.tsx` | `d3f711b7518de45ebd48736a3b86956ab03ddb394ecfad3a2998381fb3450265` |
| `src/features/integrations/Applications.tsx` | `fc459b1fe37cfa4ed49131810d51805659002a03866db98ec58b10b01c858a31` |
| `src/styles.css` | `bd42c46c8062077a689ad7518469b65fba09dbc8d28b26946cd27f6576166e27` |
