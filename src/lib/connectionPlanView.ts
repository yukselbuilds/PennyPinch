export function candidateSelectionKey(items: readonly { installation_id: string }[]): string {
  return items.map((item) => item.installation_id).sort().join("\0");
}

export function initialInstallationSelection(items: readonly { installation_id: string }[]): string {
  return items.length === 1 ? items[0]!.installation_id : "";
}

export function isConnectionPlanExpired(
  plan: { expires_at_unix_seconds: number },
  nowMilliseconds: number,
): boolean {
  if (!Number.isSafeInteger(nowMilliseconds)) return false;
  const deadline = plan.expires_at_unix_seconds * 1_000;
  return Number.isSafeInteger(deadline) && deadline <= nowMilliseconds;
}
