<a name="headroomu-kendi-modellerinle-deneme"></a>

# Test Headroom with your own Codex models

This guide covers a development test in a **separate Codex CLI chat**. A new Desktop chat uses the client's existing route. Desktop connection and the product's automatic Verify had separate open acceptance at this stage.

On 23 October, the launcher passed preliminary checks with the actual Headroom window. This developer tool covers this project path and the verified installed CLI version `0.163.0-alpha.11.2`. Product distribution has separate acceptance. The earlier live CLI test used an existing login. [Existing login evidence](proofs/codex-existing-auth.md).

<a name="test-sırası"></a>

## Run the test

1. Open the Headroom development window.
   Select `Overview / Genel bakış → Start / Başlat`.
   Wait for `Ready / Hazır`.
   Skip **Manage connection → Apply** for this test.
   The launcher checks that Headroom is ready. It sets the address only for the CLI process that it starts.

2. In File Explorer, double-click `D:\Projeler\Headroom\scripts\manual-codex.cmd`.
   The launcher opens a separate CLI chat in a test directory inside the project.
   The CLI uses a read-only sandbox and runs without the shared daemon.
   It uses your existing CLI login as usual. The launcher starts the process without copying login data or tokens, creating an account, or editing permanent configuration.
   Model requests use your existing account quota.
   The CLI uses its usual chat history settings. The launcher adds no plaintext TUI capture and does not export the chat history as test evidence.
   A CLI update can stop the test at the version check. That check covers the executable. Model support requires separate evidence.

3. Enter `/model` in the CLI.
   Select a main model and reasoning level available on your account.
   Send this prompt.

   ```text
   This is a Headroom connection test.
   Reply with only HEADROOM_MAIN_OK.
   Do not use tools or files.
   ```

4. In the same chat, enter `/model` again.
   Select a different model.
   Send the next prompt.
   This checks whether the new model can use the existing connection.

   ```text
   Reply with only HEADROOM_MODEL_CHANGED_OK.
   Do not use tools or files.
   ```

5. Replace the bracketed fields with your chosen subagent model and reasoning level.
   Use a selection that your account offers. Keep compatibility unknown for a selection that your account does not offer.
   Send the prompt.

   ```text
   This is a Headroom subagent connection test.
   Keep this test separate from project development.
   Keep the main agent's model and reasoning level.
   Start exactly one subagent with [subagent model] and [subagent reasoning level].
   If either selection is unavailable, report which one.
   Stop without substituting another selection in that case.
   Tell the subagent to reply with only HEADROOM_CHILD_OK.
   Tell the subagent to avoid tools and files.
   Wait for the subagent's result.
   Report whether the subagent started.
   Do not start another subagent.
   Do not change files or settings.
   ```

   Enter `/subagents` to check that the CLI started a subagent. The CLI view shows creation. The proxy log records requests. The main agent's report alone leaves the subagent route unverified.

6. After the replies finish, enter `/exit`.
   The double-click launcher requests a local statistics summary.
   Review only the recorded model and token measurements.
   Other clients that use the same proxy can contribute to its recent records.
   Keep records with unverified warm-up, response, or agent identity out of savings totals.
   Close Headroom yourself when you finish if necessary.
   The launcher leaves Headroom and other programs open.

<a name="sonuçları-okuma"></a>

## Read the results

- Check that the CLI returns a reply. Check separately that Headroom records a new request. A missing proxy record leaves routing unverified.
- Two model records alone leave the request-to-agent match unknown. Without that match, agent identity stays unknown and product Verify acceptance stays open.
- Report “not measured” if data is absent. Report 0% for a measurement of 0. A short response is unlikely to show savings. Quality and savings comparisons require a separate controlled A/B test.
- You can share the selected main and subagent models, the steps that worked, and the content-free launcher summary. Keep authentication files, tokens, and full chat logs private.

OpenAI's CLI guide explains `/model` and `/exit`. Its subagent guide explains on-demand delegation and `/subagents`. The Headroom integration needs its own test evidence. [CLI commands](https://learn.chatgpt.com/docs/developer-commands) · [Subagent guide](https://learn.chatgpt.com/docs/agent-configuration/subagents).

<a name="hazırlık-kabulü"></a>

## Preparation acceptance

The preparation included a source and security review. Windows PowerShell 5.1 and pwsh 7.6.5 each passed 88 focused assertions. With a registered PATH like File Explorer's, the launcher stopped safely when the engine was absent or stopped. After **Start → Ready** in the native UI, CheckOnly passed in PowerShell and through the CMD wrapper. These checks started no Codex process and made no model requests. [Detailed evidence](proofs/manual-codex-launcher.md).

On 23 October, the user's interactive CLI test ended normally. The user selected two models. Two real subagents completed and returned to the main agent. This trial left the link between request routes and subagent identities open. The product's automatic Verify, general Desktop, model, and agent compatibility, and main H019 acceptance remained open at the time. [User test evidence](proofs/codex-user-manual-test.md).

The preparation left the Headroom test window in **Ready**. Start at the first step if the window is no longer open.
