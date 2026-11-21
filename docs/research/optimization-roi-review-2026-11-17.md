<a name="optimizasyon-roi-ve-kapsam-değerlendirmesi"></a>

# Optimization ROI and scope review

17 November 2026. The team reviewed the recommendations in the user's Gemini assessment. Measurements and task acceptance require their own evidence.

<a name="uygulanan-kararlar"></a>

## Decisions applied

- The team recorded separate stop criteria before reading the result of the first live comparison for the JSON candidate. The run kept the selected account, model, and reasoning effort. It also kept the current task, data, protections, and eight-submission limit. Reject the candidate if quality fails. Stop if any turn, counter, or cache condition is missing. Freeze R&D if the first valid signal shows less than 5% total usage reduction. The decision applies to this pair only. A positive result still requires the independent test matrix.
- The run recorded only the fixed test's `sent` / `retry` integers and normal, known tool order/states. It did not store unexpected string fields, prompt, private reasoning, or raw model code. The new observation layer did not change the model request, response, instruction, or budget. The same bounded observation ran in both arms.
- The run ended at the limit with five control and three candidate submissions. The control returned the correct result. The candidate's CLI record showed two completed `inspect_result` calls and an incomplete final turn. The candidate's full usage total is unknown. The pair's cold condition remained unverified. Quality and savings acceptance remain open. It will not start another repeat or a third rule candidate automatically. [Record](../proofs/rules-json-live-pilot.json).
- The next existing H041 work priority is a safe targeted query. Show signals such as `total_lines`, `matched_count`, and `truncated` only when the system can actually calculate them. Integrity or match guarantees depend on snapshot/ownership and full-scan/budget evidence. Adding a field name alone does not provide a guarantee. The total includes readback, retries, and extra model turns.

<a name="düzeltilen-yorumlar"></a>

## Interpretations corrected

1. **The current test keeps its original size criteria.** The proposed 1,500–2,000 estimated tokens and 30% local reduction are hypotheses. Leaving a small task passive after the fact changes the A/B test. Not touching small output can reduce added transformation risk. General model errors are still possible. The current candidate already returns the original stdout, which avoids growing the JSON. Net benefit requires evidence across all provider turns.
2. **Cache loss requires more evidence than a change in the Anthropic marker's position.** In the official automatic cache flow, the marker advances to new blocks. The old prefix stays readable, and the added part can be written. The changing prefix, TTL, and lookback limit also matter. It is correct to assess multi-turn reads/writes and total cost together. Do not derive a bill from the local marker position alone. [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#how-automatic-caching-works-in-multi-turn-conversations).
3. **Large targeted-query numbers are not provider measurements.** The full input JSON in the fixed local example had 60,295 tokenizer tokens. The targeted query used 693–709. Including a later full read increased the total to 61,200–61,228. Real client permissions, task quality with the same model, and provider cost remain open. Do not make a product promise from the 36,000 → 41 ratio reported in a different context. [Local source](../proofs/targeted-query-pilot.json).
4. **H039 keeps its completed scope.** The team previously completed H039's local Windows 10 acceptance. H048's earlier macOS development acceptance covers the earlier version. The new 0.44.0 pin needs its own native acceptance. H041/H042, Windows 11, the new native pin, and production/API gaps keep their own open/not_tested limits. Those items stay open while optional R&D is frozen. Keep current free-quota and account limits. Do not start new CI, buy an API key, or begin distribution.

<a name="çekirdek-ve-ar-ge-ayrımı"></a>

## Core and R&D scopes

The core records reliable usage mapping for an allowed route, process/settings ownership, package/OS compatibility, and release acceptance. The Learn benefit pilot, experimental rules, and Serena are separate, default-off R&D acceptance areas. They all remain open in the current task list. Core compatibility requires its own evidence. Keep e3 frozen. Acceptance of this stage is required before e4. H041 stays open. The independent targeted query preparation within this task can continue.
