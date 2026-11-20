<a name="004--salt-okunur-bağlantı-önizlemesi"></a>

# 004 — Read-only connection preview

21 October 2026. Local H017 preview acceptance on Windows 10 is complete. [Evidence](../proofs/connection-plan.md), [review](../reviews/H017.md). Actual config writing and client connection are separate gates.

<a name="kullanıcının-gördüğü-kapsam"></a>

## User-visible scope

Finding an installation does not authorize a connection. The user requests a plan by selecting a specific installation and an explicitly named scope. The initial preview scope is the user config. Project, command-line, shell, managed settings, and active settings in open sessions are shown as unknown unless separately verified. Project or open-chat databases are not scanned or changed. This preview does not send a paid model request.

The plan proposes only the required endpoint field. Model, reasoning, provider identity, authentication method, and agent count are not changed. A declaration in config and the setting effective at runtime are evaluated separately. Main/subagents are in scope only if they use the same verified route. Unknown catalog statuses remain unchanged. Hook/shell contents, if present, are neither executed nor displayed. Their effects and uncertainty are summarized with fixed statuses.

H017 does not write config or treat the connection as verified. H018 covers ownership/atomic writing/undo. H019–H021 are the actual client-route gates. An action that was not applied in preview does not return success.

<a name="güven-sınırı"></a>

## Trust boundary

The renderer supplies only an opaque installation ID, expected scan generation, and permitted scope. Arbitrary file, port, URL, shell, or auth values are not accepted. The installation comes from the backend discovery record. The backend derives the config location from permitted user roots. File reads are bounded by size, schema, fixed-disk, and reparse checks. An ambiguous, malformed, or changed document is not treated as a successful plan.

TOML is parsed with a standard parser. JSON/JSONC is parsed with a parser that understands strings, comments, escapes, and trailing commas. Duplicate fields are treated as ambiguous. Parser errors, free text, and config contents are not copied to UI/logs. Auth files and chat databases are not read. Any secrets encountered in config remain within the short in-memory read boundary. Preview carries only fixed labels and necessary non-secret metadata. Original URL/header/command/path literals are not exposed.

The persistent target comes only from a fresh, owned ingress record from the H040 runtime manager. The engine's temporary port or a target supplied by the UI is not used. Preview reports clearly when the endpoint is not ready. When the application exits completely, the connection port is released. This preview is not evidence for background/tray or disconnect behavior.

The time-limited plan ID is bound to the config fingerprint, installation/scan generation, selected scope, and persistent endpoint identity/address. H018 must reread and check the fingerprint when applying. The existence of an H017 result is not write authorization.

<a name="kaynak-ve-destek-sınırı"></a>

## Sources and support boundary

- The Codex configuration reference defines `openai_base_url` for the built-in provider and `model_providers.<id>.base_url` for custom providers. [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference). Provider/auth keys are not treated as effective in project config. Current documentation is not evidence that an installed binary behaves the same. [H008](../proofs/codex.md) is limited to the recorded CLI fixture scope.
- Claude distinguishes user, project, local and managed layers from command-line and environment effects. [Claude settings and precedence](https://code.claude.com/docs/en/settings). The [H009](../proofs/claude.md) API fixture does not prove OAuth or all-session behavior.
- OpenCode merges JSON/JSONC layers. V1 uses `provider.<id>.options.baseURL` and V2 uses the plural provider schema. [OpenCode config](https://docs.opencode.ai/docs/config/) · [Provider settings](https://opencode.ai/docs/providers/). [H010](../proofs/opencode.md) is limited to the pinned V1 tools-off fixture.

These sources were read in the relevant sections on 21 October 2026. Unknown versions, custom gateways, auth, transport, and separate subagent routes remain open. The final candidate's actual native preview path for three formats was verified with temporary config roots. This result does not raise the live compatibility status of installed clients.
