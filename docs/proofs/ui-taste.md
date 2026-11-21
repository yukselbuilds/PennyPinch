<a name="ux001--yeni-görsel-dilin-yerel-kabulü"></a>

# UX001 — Local acceptance of the visual refresh

Work began on 5 November and passed local acceptance on 6 November 2026. The Graphite and mint refresh gave four screens a shared hierarchy, typography, spacing, SVG icons, and empty states. The refresh covered existing functions. H029 had yet to start. [Design contract](../../DESIGN.md) · [Review](../reviews/UX001.md)

<a name="kaynak-ve-kurulum"></a>

## Sources and installation

The lead agent read the four sources from the user in their primary GitHub repositories. The project selected Emil design engineering for regular work on the product UI. It installed only `.agents/skills/emil-design-eng` with the official skill-installer. The MIT license, pinned commit, and hashes are in `UPSTREAM.json`. Global settings and model settings stayed the same. The project added no UI or motion dependency. It reviewed the other three sources without installing them. Their review metadata is under `work/references/design-skills/`. [Emil design engineering](https://github.com/emilkowalski/skills/tree/c8a79627b59d72d764d6ab6f2c87c09a48169833/skills/emil-design-eng)

| Check | Observed | Evidence |
| --- | --- | --- |
| Real native controls | Start, Pause, and Resume used the same generation and endpoint. A separate Stop ended that generation. Restart used a new generation. The run exercised Overview, Applications, Activity, and Settings. The model call count was 0. | `work/proofs/taste/ui-runs/b6f83419-3f3a-4833-a6e7-6d8071cb1224/result.json` |
| Pending and error | The same app ran in an iframe without native IPC. It withheld success until acknowledgment and blocked a second action. The state became unknown when the acknowledgment and read both failed. A fresh read restored the state. | Same native run, isolated full-app fixture |
| Data and visual state | The strict client processed normal, 0, negative, and mixed values. Stale state preserved prior data. A failed initial read had a separate error state. The fixture used 16 actual buckets and 2 model groups. Sample data stayed in the test fixture. | Same native run. Four dataset checks in `data-validation-1.json` |
| Language, theme, and scale | The run covered Turkish and English, light and dark themes, and frame widths of 1024/819/683. The WebView used zoom levels 125/150/100 and reset. The run checked Enter, Space, Escape, the Activity dialog, and reduced motion. | Same native run. 10 control groups and 0 page errors |
| Contrast | The test measured four critical text and control selectors in two languages and two themes. The lowest ratio was 4.68:1. Full WCAG certification was outside the scope of this check. | The native run's `contrasts` array |
| Visual review | The lead agent checked real empty screens and clearly marked screens with sample measurements. The full-surface captures used an expanded, isolated frame. The background native window stayed outside the captures. | `visual-runs/f392ab84-2f92-4177-8818-4369ba11f221/result.json`, `sample-dark.png`, `sample-light.png` |
| Local product gate | 138 frontend and 349 Rust tests passed. The gate ignored 6 existing opt-in tests. TypeScript, Vite, formatting, and the offline custom-protocol build also passed. | The first two product builds in `render-build-2.json` plus `final-checks-1.json`. All 152 inputs match the same final candidate. |

All work paths are under `work/proofs/taste/`. The `acceptance.json` receipt compares the result with the current files. It checks source, config, tools, environment, native EXE, dist, fixture, runner, design guide, license, and handoff records. The final candidate kept the same inputs, so the successful builds still applied. The team combined two measurement blocks with the same fingerprint. The sample dataset correction refreshed only the QA bundle. Production code and the EXE stayed the same.

Test windows exited with code 0. The owned debug ports closed. The fingerprints for Codex config, preferences, onboarding, and ownership matched. The run retained no raw provider content, authentication data, or exception dump. The UI screenshots show this task's app or its labeled fixture.

This development acceptance covered Windows 10 22H2, build 19045. The native engine cold start exceeded some 40 s test waits. The check passed under the existing 100 s native and 90 s helper limits. Startup performance and sustained load require separate checks. Windows 11, macOS, Linux, installers, and live providers were outside this run. Sample percentages describe fixture data. User and development savings require measured usage. At that checkpoint, functional product acceptance stayed at 27/51. UX001 refreshed presentation.
