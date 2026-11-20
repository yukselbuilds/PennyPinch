<a name="011--güncel-codex-keşfi-ve-ortak-kullanıcı-bağlantısı"></a>

# 011 — Current Codex discovery and shared user connection

3 November 2026. H008 usage correction. The lead agent owned auth/routing/process decisions. The single Luna worker wrote only independent synthetic CLI/app-server fixture helpers.

<a name="sorun"></a>

## Problem

Discovery was tied to the sizes/hashes of two old binaries and did not recognize the current CLI 0.166.0 or Desktop backend 0.166.0-alpha.5.1. Plan, Apply, status reader, and frontend also used old CLI version/surface lists. The standalone installation uses a local junction. The Desktop package could not be found because the WindowsApps root could not be enumerated. The package's Codex.exe is a small launcher, while the actual window is ChatGPT.exe and product metadata identifies Codex. A long PATH also exhausted the candidate-probe budget before the Desktop check.

<a name="karar"></a>

## Decision

- Legacy full-hash evidence is retained. Other bounded binaries are checked with WinVerifyTrust for the OS trust chain, cached revocation, and exact OpenAI OpCo, LLC signer O/CN. The version command is not run until chain/publisher verification succeeds. New binary hashes do not need to be added to an allowlist. A publisher change requires review.
- Local Codex junction targets are resolved through at most 8 steps. Network/device/relative/parent targets and any remaining reparse component at the final target are rejected. Canonical target, file identity, size, and SHA256 are checked. A read-only handle prevents writes/deletes, and parent pins are retained during verification. Path rules for other clients are not relaxed.
- The signed file's --version command runs without a shell, without inherited auth/config environment, with stdin/stderr closed, 256-byte stdout, a 5-second timeout, and an owned JobObject. A small digest→version cache only avoids repeating the version process. The signature and file are revalidated. Offline trust is not a new online revocation check.
- Windows GetPackagesByPackageFamily/GetPackagePathByFullName finds the OpenAI.Codex_2p2nqsd0c76g0 package registered to the current user, read-only. It requires neither admin permission nor enumeration of the WindowsApps root. The executable with suitable signed GUI metadata is selected. Version is reported from package identity. Registered Desktop is checked before consuming the PATH probe budget. Count/size/path limits are not increased.
- A valid bounded CLI 0.x version with minor >= 163 may use the existing openai_base_url user-config contract for an experiment. This is not a live protocol/model guarantee. Other major versions require review. The Desktop's 3/4-component app version is not compared with CLI version. Version/surface limits for other clients remain unchanged.
- Codex CLI and Desktop user connections use the same atomic Plan→Apply→Undo contract. Model/reasoning/provider/auth and user additions are preserved. Ownership, fingerprint, runtime lease, stale selection, and conflict-preserving undo are unchanged. The frontend does not reject metadata using a second list of old binary versions.
- New-process verification is for CLI only. Desktop selection explains the same connection scope and need to reopen. A CLI response is not evidence that an open Desktop chat adopted the route. No model-name or subagent-model allowlist is added.
- The verifier workspace is under the application's local state root in both debug and release. CodeGraph/project config inside the checkout does not block testing. Ancestor-config checks remain. Existing user settings are read-only. Temporary endpoint override is supplied only to the new CLI process. A missing app-state root cannot be used safely.
- Debug scans of large binaries are accelerated through sha2 dependency optimization. Release is already optimized. This is not a token-savings measurement.

<a name="kabul-sınırı-ve-kaynaklar"></a>

## Acceptance boundary and sources

Discovery/config capability, fixture transport, the new CLI process's actual response, and open Desktop/subagent traffic are separate evidence. Generic CLI error notices are kept separate from successful terminal completion. Local success does not stand in for Windows 11/other OS or every model's full feature set. The historical manual-codex launcher is evidence for an old pinned binary and is not presented as current product verification.

- [Current evidence](../proofs/codex-current.md), [review](../reviews/H008-current.md).
- OpenAI [app-server](https://learn.chatgpt.com/docs/app-server), [settings layers](https://learn.chatgpt.com/docs/developer-settings), [built-in provider endpoint](https://learn.chatgpt.com/docs/config-file/config-advanced).
- Microsoft [WinTrust offline options](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data), [packages registered to the current user](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackagesbypackagefamily).
