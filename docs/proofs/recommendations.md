<a name="h045--yerel-öneriler-kabulü"></a>

# H045 — Local recommendations acceptance

10 November 2026. Local acceptance passed on Windows 10 22H2 x64. The **Local recommendations** option in Settings starts off. When enabled, Overview shows recommendations with sufficient evidence. A row disappears when a new measurement or installation check removes its condition. [Measurement and preference contract](../decisions/029-local-recommendations.md) · [Evidence record](recommendations.json)

| Check | Result |
| --- | --- |
| Signal rules | Tests covered empty/partial/mixed measurements, zero or negative benefit, old time, duplicate origin, conflict, different engine/model/route, and threshold boundary. Test fixtures showed the recommendation disappear with new healthy samples. |
| Preference and IPC | A missing field defaults to off. A save in the older format preserves the current preference. A wrong type or revision/fingerprint conflict preserves the prior record. With the preference off, the client sends no recommendation request. Old and invalid recommendation replies stay hidden. |
| Real Windows app | The QA history's 1,000 → 1,250 token example verified enabling and disabling on the same negative measurement. The app exited normally. It loaded the enabled preference after reopening. After the test added a second valid measurement, the QA history showed a net reduction of 400 tokens. The input increase recommendation disappeared. The app saved the disabled preference and exited with code 0 again. |
| UI | Tests used isolated Tauri transport in the real App. They checked four recommendations, one Activity action, one Applications action, removal after a current measurement, and keyboard use. The TR/EN, light/dark, and narrow views had no accessibility violations in 88 checks. A viewport check is not real OS DPI acceptance. |
| Final local gate | 181 frontend and 417 Rust tests passed, with 15 opt-in Rust tests ignored. TypeScript/Vite, fmt, and embedded custom-protocol build passed. 239 quality inputs and 248 native inputs stayed the same in their own runs. |

| Before | After | Why |
| --- | --- | --- |
| Each measurement row had its own Activity button. | One Activity button appears under the section. One Applications button appears when needed. | This reduced repetition and keyboard stops on narrow screens. |
| The row key contained list order. | A fixed key uses signal type and, when needed, client. | Other row identities stay stable when a recommendation disappears. |
| The cache note did not state the source. | The note says the count combines provider and local response-cache hits. | The displayed ratio describes the field reported by the engine. |

The test driver stopped at the enable step in the first native attempt. The QA preference file confirmed a `true` record. The team reused the same QA binary with a wait-based keyboard flow.

The first cleanup step found a file-sharing lock. An automated check later rejected the folder-delete command with `blocked by policy`. A separate QA profile remained in place with the preference off and two synthetic measurements. The last two runs exited normally and closed the QA debug ports.

The real app test used a separate QA identity and local history database. It did not start the engine. Deterministic metadata fixtures tested cache and compression rules. No model provider call occurred. This acceptance does not cover Windows 11, production distribution, live provider effects, or fallback to an older binary with the new setting on.
