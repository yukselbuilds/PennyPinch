import type { InstallationSummary } from "./integrations.ts";
import type { ConnectionPlan } from "./contracts.ts";

function key(plan: ConnectionPlan): string {
  const { request_id: _r, plan_id: _p, installation_id: _i, expires_at_unix_seconds: _e, ...effect } = plan;
  return JSON.stringify(effect);
}

/** Native validated plans prove equivalence; binary names or versions alone cannot. */
export function equivalentCodexInstallation(
  candidates: readonly InstallationSummary[], plans: readonly ConnectionPlan[], generation: number, nowMs: number,
): string | null {
  if (candidates.length < 2 || candidates.length > 4 || plans.length !== candidates.length
    || new Set(candidates.map(item => item.installation_id)).size !== candidates.length) return null;
  for (let i = 0; i < candidates.length; i += 1) {
    const plan = plans[i];
    if (candidates[i].client_kind !== "codex" || plan.client_kind !== "codex" || plan.scope !== "user_config"
      || plan.installation_id !== candidates[i].installation_id || plan.scan_generation !== generation
      || plan.status !== "preview" || plan.blockers.length !== 0 || plan.expires_at_unix_seconds * 1000 <= nowMs) return null;
    if (i > 0 && key(plan) !== key(plans[0])) return null;
  }
  // The representative is temporary; explicit Connect still remembers and freshly previews it.
  return candidates[0].installation_id;
}
