# Headroom Desktop — Three UI Concepts

**Date:** 19 October 2026
**Scope:** Visual direction and interaction previews. All counters are sample data; no client settings or real model traffic are changed.
**Related tasks:** [H003 / H004](TASKS.md). A production design had not yet been selected.

The gallery contains all three concepts. Preview files were local to the design workspace.

Gallery (`outputs/ui-concepts/index.html`)

## 01 — Graphite

**Recommended starting direction.** Dark graphite surfaces, soft mint accents, and a labeled left navigation. Input reduction gets a wide area; the chart and applications balance in two columns. Calm and readable for a developer tool that stays open all day.

- Layout: persistent left navigation; spacious summary, chart, and connection columns.
- Character: simple, native-desktop feel; low visual noise.
- Strength: primary action and status are easy to find; room to grow across four screens.
- Watch item: balance the prominence of the top summary in a small window.

Interactive Graphite (`outputs/ui-concepts/graphite.html`) · Screenshot (`outputs/ui-concepts/graphite.png`)

## 02 — Ivory

Warm light background, forest green, and large serif numerals. Top navigation opens the screen horizontally; savings and chart sit in one column, while connected applications use a separate, quiet surface.

- Layout: horizontal top navigation; asymmetric content and application area.
- Character: airy, editorial, and less technically dense.
- Strength: numerals and primary actions stand out; the light theme is strong.
- Watch item: keep top navigation from becoming crowded as advanced filters grow; design its dark counterpart separately.

Interactive Ivory (`outputs/ui-concepts/ivory.html`) · Screenshot (`outputs/ui-concepts/ivory.png`)

## 03 — Signal

Midnight blue surfaces, ice-blue accents, and a narrow icon rail. Summary and chart sit on the left, with application connections in a persistent right column. It feels like a more compact monitoring tool.

- Layout: narrow navigation rail; workspace and connection rail.
- Character: technical, controlled, somewhat denser.
- Strength: connection status stays in view; uses wide windows well.
- Watch item: accessible names and, in the final product, tooltips must support icon meaning; simplify the layout in narrow windows.

Interactive Signal (`outputs/ui-concepts/signal.html`) · Screenshot (`outputs/ui-concepts/signal.png`)

## Comparison

| Decision | Graphite | Ivory | Signal |
| --- | --- | --- | --- |
| Preview theme | Dark / mint | Light / green | Dark / blue |
| Navigation | Labeled left rail | Horizontal top navigation | Narrow icon rail |
| Information density | Balanced | Low | Medium |
| Usage focus | Daily control | Simplicity and savings | Traffic and connections |
| Suggested fit | First candidate | Users wanting a light/open approach | Users wanting technical monitoring |

These are not merely color variations: layout, numeral hierarchy, and navigation differ. The single selected direction will be developed with both light and dark themes in the final product; the three concepts will not be maintained as separate app versions. The Graphite layout could be combined with Ivory's light palette if desired.

## What the preview supports

1. Switch between the three concepts from the top bar.
2. Navigate Overview, Applications, Activity, and Settings.
3. **Sample session / First launch / Connection issue** states.
4. Start/pause and sample error recovery.
5. One-time connection dialog, pending verification, and disconnect.
6. Today / Last 7 days filters; sample activity filters for main agent / Luna.
7. Request details, local preferences, and sample CSV download.

The preview is plain HTML/CSS/JavaScript and requires no installation. Open `index.html` or a concept file directly in a browser. If a local preview server is already running, the same files can also be viewed over HTTP. Production technology, real integration tests, and the Windows installer are outside the scope of this design artifact.

## Preview verification performed

- Three concepts × four screens × five widths: 1440, 1024, 768, 390, and 320px; no page-level horizontal overflow in 60 screen/size checks.
- Start/pause, period filter, add/remove connection, activity filter/details, keyboard switching, sample CSV download, and error recovery flows passed.
- Adding a connection at first launch does not generate data; no chart or savings value appears without a request.
- No browser JavaScript errors or network requests outside the preview were observed. Screenshots were visually reviewed.
- The task list's 39 unique IDs, acyclic dependencies, and valid local document links were checked.

These checks apply to the design preview in the local Edge browser; they do not replace Windows 10/11 native app, engine, or real client compatibility tests.

## Decision record

- [x] The user selected **Graphite — dark graphite and mint**.
- [x] A balanced-density approach with light/dark semantic themes in the same Graphite layout was established.
- [x] The selection was recorded in the spec and [H004 decision file](docs/decisions/001-ui-direction.md).

The selected production direction is **Graphite**. Ivory and Signal remain comparison previews; demo figures are not carried into the production app.
