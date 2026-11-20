<a name="001--seçilen-görsel-yön-graphite"></a>

# 001 — Graphite visual direction

**Date:** 19 October 2026<br>
**Status:** Selected by the user.

The user answered the asynchronous concept question with **Graphite — dark graphite and mint**. The production UI uses the Graphite layout. Light and dark color themes share the same navigation and component structure.

<a name="uygulama-yaklaşımı"></a>

## Implementation approach

- The four screens are Overview, Applications, Activity, and Settings. Use labeled navigation on the left. Engine, connection, and compression each have their own status.
- Apply dark and light colors with semantic CSS tokens. System theme is the default. The existing concept HTML serves as a prototype.
- Use Segoe UI Variable/Segoe UI system fonts and tabular figures for metrics. Color is not the only status indicator. Preserve visible keyboard focus.
- Main background/panel/text/accent colors are dark `#111315` / `#1A1D21` / `#F0F2F4` / `#47C7A5` and light `#F7F8FA` / `#FFFFFF` / `#1C2027` / `#147D68`. Validate secondary-text and interaction-state contrast separately in the production app.
- Show an empty state when there is no real measurement. Do not carry over demo values from the concept.
- Windows 10 does not require Windows 11-only effects for the basic interface. Start/pause actions depend on the actual manager result.

Keep Ivory and Signal in `outputs/ui-concepts/` as comparable design alternatives. Supporting three different navigation layouts in one app under the name of themes would unnecessarily expand first-release scope. Color theme and layout concept are separate decisions.

The decision draws on section 8 of `HEADROOM_DESKTOP_SPEC.md`, `UI_CONCEPTS.md`, and `outputs/ui-concepts/graphite.html`.

<a name="5-kasım2026--kullanıcının-istediği-tasarım-yenilemesi"></a>

## 5 November 2026 — User-requested design refresh

The user rejected the old appearance. Preserve the Graphite/mint family while rebuilding hierarchy, spacing, typography, icons, and presentation of the four screens. The palette/layout values above remain historical. The current visual contract is [DESIGN.md](../../DESIGN.md) and the project's Emil design engineering guide. UX001 refreshes existing functionality visually. It does not add an account/chat feature or produce H029 acceptance.
