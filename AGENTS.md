# PennyPinch repository instructions

## Purpose and sources

- The canonical workspace is `D:\Projeler\Headroom`.
- Product scope lives in `HEADROOM_DESKTOP_SPEC.md`. Task order and acceptance criteria live in `TASKS.md`.
- Keep decisions in `docs/decisions/`, verification summaries in `docs/proofs/` and the current handoff in `docs/IMPLEMENTATION_STATUS.md`.
- Target Windows 10 22H2 x64 and Windows 11 x64. Verify each operating system independently.
- The finance, file and workspace rules from the an unrelated project reference project apply to that project alone.

## Task planning and context

- Work from the active task, the latest checkpoint and the relevant specification and decisions. Reuse current source and documents already in context. Refresh the relevant parts when they have changed, context is missing or an uncertainty needs resolving. Avoid copying the full task list or conversation into each handoff.
- Specify the goal, out-of-scope work, contracts to preserve, critical assumptions, normal, error and boundary scenarios, and the relevant source and verification paths. The lead agent checks the plan against current source and updates missing or conflicting decisions. Create a plan file or request a separate model review when the task warrants it.
- Workflow maintenance leaves product-task status unchanged. Check the user's existing authorization before publishing, using paid resources, accessing accounts or expanding scope. Proceed with authorized routine work.

## Lead agent and single worker

- The lead agent uses the user's selected model and reasoning level and owns architecture, critical assumptions, scope, acceptance, implementation ownership, review and completion. The lead agent can implement directly. Briefly explain whether the lead agent or Luna owns the implementation. Keep the selected lead model settings.
- Delegate to Luna when behavior and verification are clear, architectural decisions are settled and the work justifies a handoff. Keep small tasks together. Treat sequential handoffs as sequential work. Report savings only when measured.
- The lead agent implements changes that require decisions about authentication, model or route safety, process or configuration ownership, token or cost measurement, persistent state or recovery, or an unresolved performance cause. Helpers with clear contracts can be delegated.
- When delegating, use one **GPT-6 Luna, xhigh** worker with `model="gpt-6-luna"`, `reasoning_effort="xhigh"` and `fork_turns="none"`. Report unavailable model or reasoning settings. Keep the requested settings and a single worker without nested delegation.
- The worker configuration applies to development in this repository. PennyPinch handles model names independently of the development agent. Product verification covers different main and subagent models and model changes, without treating the development worker's model as a required test model.
- Assign one writer to each implementation area at a time. Before taking over, the lead agent stops the worker, verifies it has stopped and preserves its changes and evidence. Keep the worker and lead agent from writing the same implementation in parallel.
- Reuse the worker for concrete sequential assignments. Open one replacement Luna from the checkpoint only when the worker session actually ends or its handle is lost. Keep the same worker and model through delays, timeouts and initial test failures.
- Include the task ID, goal, file ownership and bounds, acceptance criteria, critical assumptions, relevant sources and evidence, and focused verification commands in each handoff. Summarize the relevant interfaces and decisions and let the worker verify assumptions against the source.
- Keep each slice to one behavior and acceptance scenarios that can be verified together. Roughly one to five source files is a handoff guide. Split sequential work by independent goals.
- Before editing, the implementer checks the affected interfaces, storage, measurements and runtime or client contracts. Include the real engine or client path behind the UI action and its error behavior.
- The worker stops decision-dependent edits and reports the observation, attempted approach and decision needed when a contract is unclear or contradictory, a retry produces no new evidence, acceptance would need to be weakened or scope expands without a reason. The lead agent sends one set of corrections. If the problem persists, the lead agent takes over or narrows the slice while preserving acceptance.
- The worker reports changed files, actual behavior, verification results and fingerprints, limitations and the next concrete action. The lead agent reviews the diff, the critical connected path and the evidence. A worker summary or passing worker tests alone is insufficient for acceptance. Send corrections together and review the changed area and its impact. Resolve cosmetic preferences without another review cycle.

## Efficiency and evidence

- Compaction continues the same work and preserves valid evidence.
- Reuse a PASS only when the workspace, relevant source, test and configuration inputs, dependencies and toolchain, command and environment fingerprints match. Refresh live connection, CI, deployment, migration, database and production checks at their respective stages.
- Before repeating a successful check, state the concrete reason it was invalidated. Reuse valid results.
- Use the verification command in the plan, or the TASKS criteria and the gate in `docs/development/WORKFLOW.md`. Verify the script or command exists before relying on it.
- Run focused tests during implementation. After review and corrections, run the full local quality gate once on the final candidate of the main task. Instruction-only maintenance calls for checks of the affected instructions.
- Capture the result and relevant before-and-after source and environment fingerprints programmatically in the same verification step. Avoid hand-copying hashes. If inputs changed, the PASS is evidence for the earlier candidate. State any limits in the available measurement tooling.
- Keep successful tool output short. Keep raw failures out of persistent evidence. Show a bounded, redacted excerpt when diagnosing a problem.
- Limit success claims to the evidence. Long-term reliability and live provider routing require their own evidence beyond a short run or isolated fixture. Define the data, operating state, load, duration and threshold before performance verification. Keep the agreed threshold when a check fails. Record `not_tested` or unknown as a verification state.
- Limit internet research and source reading to the relevant material. Verify changing technical claims against current primary sources or the pinned source code.
- Report only measured token usage and savings. State when development-agent token savings are unmeasured.
- Do not copy base instructions from `models_cache.json` into `model_instructions_file`. Leave unrelated global Codex model and subagent settings outside this project.

## Implementation boundaries

- Verify the canonical root before writing. Use the exact path and stop if a required read fails or returns unexpected empty content. Prefer a contextual patch. For a full write, preserve the original bytes, validate the temporary file and replace it atomically. Recover an accidentally emptied file by validating its original content and intended additions before continuing.
- Verify the real engine, packaging and protocol before implementing connected product slices. Label mock results with their actual test scope.
- Discovery is read-only. Engine experiments preserve existing client settings and active sessions in other projects.
- Connection changes record ownership and fingerprints, write atomically and preserve conflicts during rollback. Keep authentication secrets out of persistent logs and state.
- The runtime manages only app-owned processes and loopback ports. Leave other proxies under their own ownership.
- Make model, reasoning, authentication method and agent-count changes explicit. Verify agent identity and routing separately from the model name.
- Enable local measurements and disable anonymous beacon collection by default. Retain the minimum metadata needed, with message contents and secrets excluded.
- Pausing preserves active streams and access to existing CCR retrieval data. Require evidence for fail-open behavior, model support and Windows support.
- Defaults are the coding/cache profile, source and numeric-content protection and a CCR TTL of 7200. Automatic model or reasoning changes, memory/learn and the output shaper are off.
- Preserve the user's existing changes and keep unrelated projects' `.env` files and keys in their own projects. Create worktrees or chats when needed. Keep product source and project-downloaded development artifacts under this project or the explicitly designated app state root.
- Follow applicable skills and proceed with routine reversible work under existing authorization. State unavailable account or second-OS checks in the support status and continue independent work.

## Completion and handoff

- Complete the active main task before moving to the next. Update `TASKS.md` as work progresses. Tell the user when each main task is complete, stop and wait for the user to continue. Report subtask acceptance at its own scope.
- Mark a task complete once its acceptance criteria have concrete evidence. Leave partial work and external verification open.
- Review product behavior through the path from UI action to manager to engine or client. Check the connected path in addition to any button or mock tests.
- At interruption or close, record the last verified slice, evidence, open items and next concrete action in the checkpoint. Include the implementer or takeover rationale and any measured duration, correction cycles or model use. Keep the checkpoint consistent with TASKS.
- Record confirmed compatibility, local development success and release-candidate readiness separately.

## Project CodeGraph

- Use `scripts/codegraph.ps1` for CodeGraph development support. Use `rg` or direct reads if the local runtime is unavailable. The lead agent and single worker can share the project index with any model. See `docs/development/codegraph.md` for supported commands and verified limits.
- Use narrow graph queries for callers, dependencies, impact and related symbols. Read known files or small line ranges directly. Ask the graph for the smallest relevant result set.
- The MCP server is registered in the project's `.codex/config.toml`. If its tool is unavailable in the chat, use the same launcher from the CLI. Keep global Codex settings and other chats outside this setup.
- Run the launcher at the project root. Telemetry and update-check opt-outs and the `.codegraph` directory are process-scoped. `codegraph.json` defines the source, test and script scope. The index excludes work and caches, real authentication and state, machine-specific settings, indexes and build artifacts. Keep the graph database out of Git.
- Use graph results for discovery. Verify dynamic Tauri IPC, live client traffic, subagent ownership, runtime behavior and test passes independently. Update the index as documented when source or index state changes. Fall back to direct source and `rg` if the graph is broken or incomplete. Report a token savings percentage only from actual measurements.

## Ponytail and task acceptance

- Apply the project `.agents/skills/emil-design-eng/SKILL.md` and `DESIGN.md` for UI design and visual review. They cover visual hierarchy, typography and interaction. Keep engine, measurement, security and existing runtime acceptance contracts in effect. Use the project-local skills and settings.
- Follow `docs/development/WORKFLOW.md` for tool selection, short worker handoffs, the actual local quality gate and main-task completion criteria. Apply the task's own criteria as well.
- Use the project `.agents/skills/ponytail/SKILL.md` at `full` level for implementation and fixes. The lead agent reads `.agents/skills/ponytail-review/SKILL.md` at acceptance and folds it into the same quality review. Use `.agents/skills/ponytail-audit/SKILL.md` for scoped historical code audits.
- Keep repository reports in English and communicate with the repository owner in Turkish. Ponytail provides development guidance within existing model, agent, publishing and settings authorization. The smallest complete change preserves ownership, atomic writes, conflict-safe rollback, security, error handling, accessibility and required tests.
- Give each finding a concrete condition, file and line, user impact and small fix. A reduction in lines of code is separate from acceptance. Report savings only when measured in the same scope.
- Start audits read-only and state what the audit covered. Treat an actual fix as a separate scoped implementation and choose the implementer as above. Resolve critical correctness or data-loss findings before dependent work. Keep prospective features and cosmetic suggestions outside the defect gate. Skills and tools operate within the product scope and the user's authorization.

## Simplified Technical English

- Use the project [ASD-STE100 skill](.agents/skills/asd-ste100/SKILL.md) when you write or revise English technical text.
- Use the same sentence rules in replies to the repository owner in Turkish. Use short, active sentences and consistent terms. Put one action in each instruction.
- Use Strict mode for procedures, error messages, tool descriptions, and agent instructions. Use STE-flavored mode for reports and explanations.
- Keep technical names, code literals, measurements, exceptions, and the strength of each claim. Compare the revised text with the source.
- Use the skill's linter to check sentence structure. Review its findings against the source.
