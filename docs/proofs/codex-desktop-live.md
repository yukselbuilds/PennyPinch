<a name="h008--gerçek-desktop-hesabı-ve-aynı-sohbet"></a>

# H008 — real Desktop account and same chat

4 November 2026, Windows 10 Pro 22H2 x64. The lead agent ran this verification because of authentication, route and process ownership. Product source did not change.

<a name="gözlenen-davranış"></a>

## Observed behavior

The run used installed Desktop `26.1027.6604.0` with a verified OpenAI signature, backend `0.166.0-alpha.5.1` and pinned Headroom `0.41.0`. The user completed official ChatGPT sign-in in a separate application-state profile and skipped the first-use screen. The profile's default model and effort were preserved. Two turns showed `gpt-6-luna` / `medium`. This is not a product model requirement.

| Scenario | Result |
| --- | --- |
| Desktop startup with the Headroom endpoint saved in the profile and a short request | The run-specific full response was verified in the visible UI. |
| Move to the new chat screen | The test chat's selection and first response were no longer visible. Hidden presence of the old chat in the DOM was not an acceptance criterion. |
| Open the same chat from the sidebar and send a second request | The first response returned, the native chat identity matched, and the second full response became visible. |

The chat record also matched two user inputs, two full assistant responses and two model/effort records with the same identity. Review was limited to this experiment's identity. Text content and auth files were not included in evidence.

The engine reported five local request rows, one cache record and zero failed requests. These counts cannot be equated one-to-one with the two user turns because request purposes were not classified separately. A real response completed over WebSocket. Raw local token estimates from three rows were `22628→22212`, `15082→14885` and `23440→23413`. These fields are not provider billing or verified quota savings. The [measurement contract](../decisions/010-metrics-normalization.md) remains in effect.

<a name="sahiplik-ve-doğrulama"></a>

## Ownership and verification

- Microsoft's package diagnostic launcher and loopback CDP for the test process only were used. This is not acceptance of the standard launcher or installer package.
- The test window was closed through its owned Job. This did not count as a normal GUI shutdown test. The engine stopped gracefully with exit code 0. Test processes and two listeners closed.
- The normal Codex config hash matched. The owned endpoint field was removed from the separate test profile. Other profile settings added by Desktop were preserved. Credentials remained in the app-state root.
- Desktop/backend files matched before and after. Helper and engine source fingerprints matched. The previous full quality gate had the same 101 product inputs, so the full gate was not repeated.
- The installed Desktop package and backend file changed after earlier opt-in tests. Real discovery/pinning and isolated CLI+Desktop Apply/Undo tests were rerun. Both passed. These checks made no model call.

Local evidence root: `work/proofs/codex-current/desktop-live-runs/dd004ad6-2551-403a-b07e-8a5a236f80ce/`. `desktop-live.json`, `finish.json`, `native-chat-metadata.json`, `installed-check.json` and `verification.json` contain content-free results and fingerprints.

Earlier run `9de4c852-5008-4870-b03a-4c184decc742` closed at timeout with an empty UI and no observed model request. The package and test environment changed between the runs. This PASS does not establish the root cause of the earlier empty screen.

<a name="çalışan-uygulamada-ayar-ve-tam-yeniden-açılış"></a>

## Setting while the app is running and full reopen

On the same day, a separate run `6d0d6209-1743-43b0-84bb-4df18dac61f0` used the existing test account and chat. The first Desktop process started without a Headroom setting. After the same native chat became visible, only the endpoint field in this test profile was added atomically.

The first extra response completed while the same GUI process remained open. Headroom request and WebSocket counters stayed at zero. This version did not use the externally added setting for that turn.

The test window was closed through its owned Job and reopened with the saved setting. The old chat and response returned. The second extra response completed through Headroom with the same native identity.

The engine snapshots before and after the second request showed a new row. This is consistent with the current UI reopen warning. Product copy did not change.

The frontend's temporary `client-new-thread` identity from the first process mapped to the native identity after reopening. The mapping used the same native chat. This did not relabel the chat database.

Two test GUI processes and all three loopback listeners closed. The engine exited gracefully with code 0. Normal config and source/package fingerprints matched. The owned endpoint was removed from the test profile. Local evidence is in the second run's `before-apply.json`, `after-change.json`, `before-second.json`, `finish.json`, `adoption.json` and `verification.json` files.

<a name="kabul-sınırı"></a>

## Acceptance limit

H008 closed under the user's scope decision of 4 November with Windows 10 ChatGPT CLI/Desktop evidence. The live API-key provider test remains open/not_tested in H041 pre-release acceptance. API-key HTTP/SSE and local Desktop WebSocket fixture evidence remain separate. The open turn in this version did not use the new setting. After a full reopen, the existing chat routed through Headroom.

The earlier cancelled wait from the different-model CLI run is historical. A later run completed the [native wait](codex-native-wait.md). Desktop subagents, Windows 11/macOS/Linux, all model features, setting changes during an active stream and long-term stability are outside this short Desktop run.
