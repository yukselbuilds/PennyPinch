# PennyPinch user guide

This guide covers the local development preview. The [platform validation matrix](validation/desktop-platforms.md) records the tested platforms and the remaining release checks.

## Connect a coding agent

1. Open PennyPinch and go to **Applications**. Choose **Scan for clients** to look in known local installation locations. Use **Add from file** if PennyPinch cannot find your installation. The selected file is only recorded permanently when you save the installation choice or connect.
2. Open **Manage connection** for the client. Confirm the installation and surface, such as Codex CLI or Codex Desktop, and review the proposed settings change. **Connect** saves your choice, starts PennyPinch's engine if needed and applies the proposed route when the configuration can be changed safely. The dialog explains any conflict that blocks the change.
3. Start a new CLI process or fully quit and reopen the desktop client. Existing chats can continue to use their previous route. Check **Activity** after sending a request. A saved connection and observed traffic are separate states.

**Start** in **Overview** runs the local Headroom engine. Use **Manage connection** to set up a client route. Sign in to your provider through the client. The client controls your model, reasoning level and authentication method.

## Accounts and sessions

**Sessions → Accounts** shows the local sign-in record for a supported client and selected CLI profile. Choose another installation if needed. A local record reflects the state saved by that client. Provider access, observed PennyPinch traffic and the account in another profile or desktop session are verified separately.

**Open official sign-in** first shows the CLI profile it will affect. Codex opens ChatGPT sign-in, Claude Code opens Claude sign-in, and OpenCode lets you choose the provider and method. Enter passwords and codes in the official client flow. Sign-in can replace an existing session or API-key record. Canceling the flow may leave that record changed. **Stop** ends the sign-in process started by PennyPinch. Signing in can continue while you switch screens or minimize to the tray. Quitting PennyPinch stops that process.

An account appears as **Unknown** when its local sign-in state cannot be read reliably. For the tested Claude Code version, the account-status command can write settings, so PennyPinch skips it.

**Sessions** can list supported Codex conversation metadata. When you confirm opening a conversation, the official Codex chat center opens for you to select it. Traffic and token savings are reported for observed requests through the local endpoint.

## Read token savings

PennyPinch measures requests sent through its local endpoint. Main agents and subagents that use this endpoint can follow the same route. When an agent uses a separate endpoint or provider, that traffic follows its own connection. Route evidence has a specific client, surface, authentication method and version. See the [compatibility records](../compatibility/clients.json).

**Overview** compares the input tokens before and after local optimization. When both values use a comparable basis and the original input is greater than zero, savings are `(before − after) / before × 100`. An increase appears as added input. The panel shows unavailable or non-comparable measurements as such. These are local input-token measurements. Your provider reports its own usage and billing.

Choose **Session**, **Today** or **7 days** in **Overview**. In **Activity**, filter observations by client or model. Unidentified models and agents remain **Unknown**.

## Pause, stop and use the tray

**Pause** stops compression for incoming requests while active responses and access to previously stored content continue. **Resume** enables compression for new requests. **Stop** shuts down the engine. Connected clients can report a connection error until the engine is running or the route is removed.

Open **Manage connection** to remove the route added by PennyPinch. The operation preserves unrelated client settings and the engine state. Reopen the client process to pick up the configuration change if required.

If the close-to-tray preference is enabled, the window close button hides PennyPinch while the engine keeps running. **Quit PennyPinch** closes the engine and removes the tray icon. A client with a saved route can report connection errors and responses in progress can be interrupted. When launch at Windows sign-in is enabled, PennyPinch opens its window. Start the engine from the app when needed.

## Local data and diagnostics

Activity and usage exports describe observed requests using metadata such as the client, model and token counts. Diagnostic exports use selected fields and leave out credential values and raw request content. The engine can temporarily keep original compressed content in its local retrieval store so the client can recover it. See the [local application trust boundaries](SECURITY_BOUNDARIES.md).

## Explore savings options

Enable **Savings tips** in **Settings** to show suggestions in **Overview** when enough local evidence is available. A tip might flag growing input, slower compression, fewer recorded cache hits or an installation that needs another scan. Each tip includes its observation time and scope. You decide what to change.

In **Settings → Learn from a session**, choose one Codex or Claude Code session file (.jsonl) saved on your device. Learn reviews repeated failures locally. For example, it can suggest checking the working directory before retrying the same file operation. You choose how to use the advice.

Enable **Rule trials (experimental)** inside Learn to choose a recording and try shortening repeated log lines. PennyPinch compares the output size in bytes and checks that it can restore the original exactly. The trial applies to the selected recording.

Open **Tool output protection** under **Operating preferences** to keep selected results in full. Enter one exact client tool name per line. For example, enter `exec_command` to protect its matching output when that is the name your client sends. Matching results on the PennyPinch route pass through without compression, which can reduce savings. Save the list and start the engine manually for the change to take effect.

## Preferences and updates

Reducing the retention period can permanently delete older measurement records. Increasing it applies to the retained and future data.

**Updates** shows signed releases when this build has a trusted update source. Choose **Check for updates** to check your channel. The app shows the verified version and lets you start the download. Local Windows 10 update and rollback tests passed. Production hosting, signing and delivery are tracked in the [Windows update validation](validation/windows-update.md).

## Verified coverage

Use **Applications → Tested versions and connections** to compare past tests by application version and sign-in method when troubleshooting. Check the application card for your current connection status.

- Windows 10 22H2 x64 is the first release target. Windows 11 verification is pending. See the [platform validation matrix](validation/desktop-platforms.md) for the scope of the macOS and Linux checks.
- Selected Windows 10 Codex CLI and Desktop routes were verified with ChatGPT sign-in. Live Codex API-key verification is still open.
- Claude Code and OpenCode connection tests used synthetic credentials and local providers. The [compatibility records](../compatibility/clients.json) bind evidence to specific client versions and surfaces.
- Token comparisons are specific to the recorded requests and optimization settings. See the [optimization validation](validation/optimization-benefit.md) for the workloads and results.
