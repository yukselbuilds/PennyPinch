# H011-a — Pause control source assessment

**Status: `not_implemented` / `not_tested`.** The production UI must keep compression pause disabled until H011/H028 proves an app-owned control path. This is a source assessment of the pinned `headroom-ai==0.41.0` package, not a runtime test.

## What the pinned source provides

`CompressionDecision.decide()` checks the inbound `x-headroom-bypass: true` or `x-headroom-mode: passthrough` header before the configured optimization flag. Either header makes that request pass through without compression. This is a request-level client signal; it does not provide a control operation that stops future compression for every connected client, and it cannot change a request already being streamed.

The proxy also has a `config.optimize=False` kill switch. The CLI's `--no-optimize` and startup environment feed that configuration. However, the process's `/admin/runtime-env` endpoint only accepts names from the live runtime-knob registry. That registry currently contains output-shaper, verbosity, and Read-threshold controls; it does not contain `HEADROOM_OPTIMIZE`. Unknown keys are ignored. `/settings/apply` persists settings and restarts the proxy (or returns a manual/restart action for the deployment), so it is not an in-place pause operation.

None of these source paths proves the product pause contract: finish current streams, prevent new compression decisions after acknowledgement, preserve earlier compressed-prefix bytes, retain the CCR store, and allow the original content to remain retrievable. H007 proves a separate local CCR retrieval path; it is not evidence that pausing preserves an active stream, prefix, or store.

## Decision and next proof

No pause or resume UI action may return a successful `paused` / `enabled` result on this evidence. The future control layer must be application-owned and generation-bound. H011/H028 should verify, through the actual client-to-proxy path, that pause acknowledgement follows the defined in-flight stream policy, later requests take the intended passthrough path, the pre-pause prefix remains byte-stable, and pre-existing CCR content remains retrievable. Resume must affect only subsequent requests and retain the selected model, reasoning, authentication route, and agent scope.

## Pinned source fingerprints

Read-only source inspection on 2026-10-19 used the H006 packaged runtime copy, not a system Python install. No H005-H010 proof was rerun.

| Source | SHA-256 |
| --- | --- |
| `headroom/proxy/compression_decision.py` | `75d460be69986c9432592ccdbad7a00a378961f509f7f91cb82e7ddf8fc573329` |
| `headroom/proxy/helpers.py` | `44022bff9d08b234b7a4c8f48ae1910c812ca6130f27ba73da1b9829c7677815` |
| `headroom/proxy/runtime_env.py` | `38454fa8331c81158fa7c8e34ccadc0ea0499980ab0d37e6eab44d4d98e95392` |
| `headroom/proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |

CCR engine retrieval evidence remains in [H007](protocol.md); engine ownership and GUI-like helper lifecycle remain in [H006-a](windows-engine.md). Neither closes the pause gap.
