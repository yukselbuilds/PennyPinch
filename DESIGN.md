# PennyPinch — Visual Language

UX001 introduced the Graphite and mint visual design for the four original screens on 5–6 November 2026, before H029 began. H051 records Windows 10 acceptance for the Sessions screen. [UX001 visual acceptance](docs/proofs/ui-taste.md) · [H051 session acceptance](docs/proofs/sessions.md)

## Product and direction

PennyPinch is a local desktop tool for everyday use. Users should see input-token reduction first, then engine status and application connections. Calculate the token count and percentage from the same measured period. Show account connections and chat metadata in Sessions according to the evidence for each. [H050 account acceptance](docs/proofs/accounts.md) · [H051 session acceptance](docs/proofs/sessions.md)

The [design engineering](.agents/skills/emil-design-eng/SKILL.md) guide by Emil Kowalski is the primary reference; its MIT license and pinned source are in `UPSTREAM.json`. Other references reviewed: [Taste](https://github.com/Leonxlnx/taste-skill), [Impeccable](https://github.com/pbakaus/impeccable), and [Hallmark](https://github.com/Nutlope/hallmark). Do not layer four separate design systems. Install any new material in the project only.

- Direction: a calm, crisp work tool. Slim navigation, a spacious main canvas, one strong savings area, with a real chart and applications below. This is not a marketing page or hero animation.
- Typography: Windows Segoe UI Variable/Segoe UI, retaining native fallbacks. 14px body, 12px helper text, 36px screen title, 72px metric. Use tabular numerals, few weights, and no tracked all-caps labels.
- Color: warm graphite and soft mint in dark mode; off-white and dark green in light mode. Use semantic CSS tokens. Never communicate a state by color alone. Use subtle local borders, emphasis for primary actions/metrics, and visible warnings for serious errors.
- Shape: one hero surface, fine dividers, and 8–16px corner radii. No stacks of equally weighted cards, neon glow, glass effects, or decorative gradients. Use row and spacing hierarchy in data-dense tables.
- Icons follow one SVG stroke style for interactive controls. The sidebar uses the brand kit's symbol for the active theme. The Windows bundle uses the kit's ICO, and the README uses the horizontal logo for the reader's theme. About displays the original PennyPinch artwork. Every button must have a clear function. An icon-only control requires an accessible name.
- Motion: frequent navigation and numeric updates happen immediately. Pointer press gets brief feedback; keyboard actions do not wait for animation. Respect reduced motion and distinguish hover from pointer behavior. Add no motion dependency.
- Empty states: no fake chart, percentage, account, or agent count. Waiting for data and read errors are distinct. Missing metrics get meaningful text and a real action to the existing connection screen; a measured zero remains visible.
- Trust: `unknown`, scope/measurement limits, pending work, conflicts, and persistent-setting warnings remain legible. Simpler wording must not make stronger claims about reality. Start/Pause/Resume changes only after a real engine acknowledgement.

## UX001 acceptance

1. In the native app, Overview, Applications, Activity, and Settings share the same visual language. Real Start/Pause/Resume/Stop, navigation, dialogs, and preference controls work.
2. Real empty/unknown/error states are distinct from explicitly isolated sample data, zero/negative/mixed/stale states. At TR/EN, light/dark, 1280/1024, and actual 125/150 WebView scale, core controls and text do not overflow; focus/keyboard and reduced motion are observed.
3. Inspect the final render visually; record before/after comparison and source/environment fingerprints. Close the final candidate with the local quality gate, TASKS/checkpoint, and commit/push. Do not change engine/auth/measurement semantics. Do not report a development token-savings percentage unless measured.
