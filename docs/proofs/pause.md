# H011-a — Pause control source assessment

**Historical source assessment, 19 October 2026.** Pause was `not_implemented/not_tested` at that checkpoint. On 5 November, the H011 and H028 implementation and local engine and native checks passed. See the [safe pause evidence](safe-pause.md). This record describes the first source assessment. Current UI and live provider support have separate evidence.

## What the pinned source provides

`CompressionDecision.decide()` checks the inbound `x-headroom-bypass: true` or `x-headroom-mode: passthrough` header before the configured optimization flag. Either header directs that request to passthrough. The signal applies to one request. App-wide control for future requests needs a separate operation. The signal leaves an active stream on its existing path.

The proxy uses `config.optimize=False` to turn optimization off. The CLI's `--no-optimize` and startup environment feed that config. The process's `/admin/runtime-env` endpoint accepts only names in the live runtime setting registry. The registry contains output shaper, verbosity, and Read-threshold controls. `HEADROOM_OPTIMIZE` is absent. The endpoint ignores unknown keys.

`/settings/apply` saves settings and restarts the proxy or returns a manual restart action for that deployment. An in-place pause needs its own operation.

The product pause contract still needs evidence. Finish active streams. Stop new compression decisions after acknowledgement. Preserve earlier compressed-prefix bytes. Keep the CCR store and access to original content. H007 proves local CCR retrieval. Active streams, prefix preservation, and store access during pause require their own checks.

## Decision and next proof

Return a successful `paused` or `enabled` result only after verification of the pause contract. The control must belong to the app and bind to the engine generation. H011 and H028 must check the actual client-to-proxy path.

Check acknowledgement against the policy for active streams. Check the later passthrough path. Check byte equality of the earlier prefix and retrieval of existing CCR content. Resume applies only to subsequent requests. It preserves the selected model, reasoning, auth route, and agent scope.

## Pinned source fingerprints

On 2026-10-19, the read-only source review used the H006 packaged runtime copy. The assessment did not rerun H005-H010 proofs.

| Source | SHA-256 |
| --- | --- |
| `headroom/proxy/compression_decision.py` | `75d460be69986c9432592ccdbad7a00a378961f509f7f91cb82e7ddf8fc573329` |
| `headroom/proxy/helpers.py` | `44022bff9d08b234b7a4c8f48ae1910c812ca6130f27ba73da1b9829c7677815` |
| `headroom/proxy/runtime_env.py` | `38454fa8331c81158fa7c8e34ccadc0ea0499980ab0d37e6eab44d4d98e95392` |
| `headroom/proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |

[H007](protocol.md) records engine CCR retrieval. [H006-a](windows-engine.md) records engine ownership and helper lifecycle with a GUI-like owner. Pause requires separate evidence.
