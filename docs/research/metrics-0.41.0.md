<a name="tasarruf-ölçümü--pinned-kaynak-incelemesi"></a>

# Savings measurement in the pinned source

19 October 2026. The review examined the packaged `headroom-ai==0.41.0` source. Its results apply to source inspection. H012 and H023–H025 use it as design input. These tasks need their own runtime and product tests.

<a name="ürün-kararı"></a>

## Measurement decision

The main Overview metric is **input token reduction**. Use input pairs with the same tokenizer and scope. For each pair, calculate `S=B-A`. For the period, calculate `rate=100×ΣS/ΣB`.

Update the amount, percentage, period, and measurement scope together. Calculate the rate from the totals. Keep missing data unknown. Show `0%` for a valid zero result with a positive baseline. Show an increase when `A>B`. Show `—` when the baseline is absent.

This metric measures forwarded input. Later requests can send the same chat history again. The total counts removed tokens across requests. Unique content and billing reductions need separate evidence. Cache and new-content rates use separate bases. Keep each numerator with its own denominator.

The coding profile in `agent_savings.py` defines `target_savings=0.50` as a nominal display value. It defines `target_ratio` as `None`. The value supplies no measurement, quality guarantee, or expected reduction for every request. An empty panel stays empty.

<a name="motordaki-kaynaklar"></a>

## Engine sources

The source root was `work/proofs/packaging/artifacts/headroom-engine/runtime/Lib/site-packages/headroom/`.

| Source or field | Meaning in the source | Desktop implication |
| --- | --- | --- |
| `proxy/outcome.py`, `RequestLog.input_tokens_original/optimized` | Local counts before and after input optimization for a request. The aggregate novel correction is separate. | The pair can be comparable when both counts use the same basis. A fixture must check completeness, lifecycle behavior, and negative values. |
| `/stats.tokens.input` | Uses the provider input count when available. Otherwise it uses a local estimate. Separate counters identify the source. | The provider and local tokenizer bases can differ. Adding local `saved` cannot establish the actual local `B`. |
| `/stats.tokens.active_savings_percent` | Uses `all_layers_saved / (attempted_input_tokens + tool_search_saved_total)`. It covers content for which the engine attempted compression. | Show its own name and denominator in Advanced. Keep the attempted-input scope explicit. |
| `/stats.tokens.proxy_savings_percent` | Uses `proxy_compression_saved / proxy_total_before_compression`. | Check novel behavior in the aggregate numerator and where the input count came from before using it in the product metric. |
| `/stats.tokens.savings_percent`, `all_layers_savings_percent` | Uses `(proxy_compression_saved + tool_search_saved_total) / total_before_compression`. | The value includes the tool schema layer. Adding that layer again would count it twice. |
| `/stats.tokens.new_input_savings_percent` | Uses `new_input_saved / (new_input + new_input_saved)` for the same set of eligible requests. It requires the provider breakdown for uncached input and cache writes. | Use a separate new-content rate. When the breakdown is missing, the engine's `0` leaves the desktop rate unknown. A measured `0%` requires evidence. |
| `proxy/savings_tracker.py`, `display_session` | The persistent tracker defines its session using its own inactivity period. | Carry the tracker's start and end source. The app launch, client chat, and day filter have separate boundaries. |
| `savings_ledger.py` | Persistent events use separate cost and cache bases for each source. The `saved` field uses `saved=max(before-after,0)`. | This field cannot recover a negative difference. The ledger may skip an event with no savings and no `new_input`. Full traffic coverage needs its own denominator evidence. |

For Responses requests that resend the full history, `proxy/conversation_savings.py` and `proxy/outcome.py` can convert cumulative savings to novel savings in the running totals. This requires a reliable conversation identity. The request log totals can therefore differ from the running totals. Incremental requests with `previous_response_id` or server-side `conversation` use a separate path.

Conversation matching requires an explicit identity. A model name, a system instruction, or `prompt_cache_key` alone leaves chat or agent identity unverified. A missing identity stays unknown. Each actual client needs evidence that it supplies a reliable identity.

<a name="h012h023-uygulama-gereklilikleri"></a>

## Requirements for H012 and H023

1. Record the measurement source, tokenizer basis, runtime generation, request ID, time, scope, and measurement state. Keep provider usage, the wire pair, novel savings, tool schema savings, and cost on their own bases.
2. Verify a content-free event adapter before H023. A `RequestLog` can include `request_messages`, `compressed_messages`, and free-form tags. Select fields with an allowlist. Keep raw logs and snapshots out of disk storage and the UI. Exclude authentication, content, and personal paths.
3. Count each event once when polling supplies the data. Show ring buffer gaps, restart or counter reset, and missing intervals. Keep snapshot differences separate from event totals. Label the measured subset if completeness is unknown.
4. The proxy, MCP, and subagent can report the same event. Use ownership and event origin to deduplicate. Show separate scopes when the source cannot establish a reliable match.
5. Include valid requests with zero savings in each period's denominator. Label a rate based only on requests with savings with its own scope. Round after aggregation.
6. Preserve the model and reasoning level. Cache hits, net cost, and subscription quota need separate evidence. Observe added retrieval and retry input separately. Net savings require evidence of those added inputs.
7. Verify which components the `RequestLog` pair counts. Check the message body, system text, and tool schema separately. In `outcome.py`, tags carry tool schema savings separately. They leave the `tok_before/after` pair unchanged. Add another layer only with comparable, non-overlapping before and after counts. Label narrower measurements with their actual scope. Keep output savings from the disabled output shaper outside the main input metric.

Focused fixtures cover these conditions.

- Different tokenizers, a request with savings followed by one with zero savings, a negative difference, `B=0`, and missing usage.
- Event replay, runtime restart, and a buffer gap.
- Two conversations with explicit identities, traffic without an identity, and full-history and incremental Responses requests.
- Duplicate proxy and MCP reports and double counting across layers.
- Consistent Turkish and English display and period selection.

<a name="inceleme-fingerprintleri"></a>

## Source fingerprints

These SHA-256 values identify the inspected version. Behavior requires separate tests. After an engine update, review only the affected metric definitions.

| Relative source | SHA-256 |
| --- | --- |
| `proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| `proxy/conversation_savings.py` | `31ab5aa5c0a0c71f5b3941b36d28162505cfd1f9f84153cb5859edad39c396f5` |
| `proxy/outcome.py` | `5d7ad234e0caab9f7adc305de6e1fe5c89ba5e99ebad2d991c75d910ef8b43b6` |
| `proxy/prometheus_metrics.py` | `8a25b94a0a4c3fe0657810d16326ff74d32538928e07ae82bb4c0290cecfc4b8` |
| `proxy/savings_tracker.py` | `ae6a75c99b98388e44670ee2b57bfab25fc6bada1c963b83baa94e940feb6daa` |
| `savings_ledger.py` | `361b011c256839ed4ddbbabae0f141c3aa735ca82688724775424b1f3be5e899` |
