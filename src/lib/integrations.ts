import { invoke, isTauri } from "@tauri-apps/api/core";

export type ClientKind = "codex" | "claude_code" | "opencode";
export type ClientSurface = "cli" | "desktop";
export type InstallationSource = "path" | "user_local_npm" | "user_local_native" | "scoop" | "windows_app_package" | "manual_file";
export type DiscoveryWarning =
  | "scan_not_run"
  | "scope_partial"
  | "absence_is_not_unsupported"
  | "path_entry_limit_reached"
  | "candidate_limit_reached"
  | "package_metadata_untrusted"
  | "package_version_unknown"
  | "network_location_skipped"
  | "candidate_location_unavailable"
  | "scoop_entry_limit_reached"
  | "scoop_version_limit_reached"
  | "windows_app_entry_limit_reached"
  | "desktop_metadata_untrusted";

export interface InstallationSummary {
  installation_id: string;
  client_kind: ClientKind;
  surface: ClientSurface;
  version: string | null;
  source: InstallationSource;
  location_label: string;
  status: "detected";
  connection_status: "not_verified";
}

export interface DiscoverySnapshot {
  schema_version: 1;
  request_id: string;
  status: "not_scanned" | "partial";
  scan_generation: number;
  installations: InstallationSummary[];
  warnings: DiscoveryWarning[];
}

export type DiscoveryErrorCode = "validation_failed" | "busy" | "environment_blocked";

export interface DiscoveryErrorEnvelope {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: DiscoveryErrorCode;
    retryable: boolean;
    message_key: string;
    diagnostic_id: null;
  };
}

export type DiscoveryResult =
  | { kind: "desktop_unavailable" }
  | { kind: "snapshot"; snapshot: DiscoverySnapshot }
  | { kind: "discovery_error"; error: DiscoveryErrorEnvelope }
  | { kind: "busy" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

export type ClientFileSelectionResult = DiscoveryResult | { kind: "cancelled"; request_id: string };

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const sourceLabels: Record<InstallationSource, string> = {
  path: "PATH",
  user_local_npm: "User-local npm",
  user_local_native: "User-local install",
  scoop: "Scoop",
  windows_app_package: "Windows app package",
  manual_file: "Selected file",
};

const warningCodes = new Set<DiscoveryWarning>([
  "scan_not_run", "scope_partial", "absence_is_not_unsupported", "path_entry_limit_reached",
  "candidate_limit_reached", "package_metadata_untrusted", "package_version_unknown",
  "network_location_skipped", "candidate_location_unavailable", "scoop_entry_limit_reached",
  "scoop_version_limit_reached", "windows_app_entry_limit_reached", "desktop_metadata_untrusted",
]);

const discoveryErrors: Record<DiscoveryErrorCode, { retryable: boolean; messageKey: string }> = {
  validation_failed: { retryable: false, messageKey: "discovery.validation_failed" },
  busy: { retryable: true, messageKey: "discovery.busy" },
  environment_blocked: { retryable: false, messageKey: "discovery.environment_blocked" },
};

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isRequestId(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function validVersion(value: unknown): value is string {
  if (typeof value !== "string" || value.length === 0 || value.length > 64 || /\s|[^\x00-\x7f]/.test(value)) return false;
  const buildSeparator = value.indexOf("+");
  const withoutBuild = buildSeparator === -1 ? value : value.slice(0, buildSeparator);
  const build = buildSeparator === -1 ? null : value.slice(buildSeparator + 1);
  if (buildSeparator !== -1 && (build === null || build.includes("+") || !validIdentifiers(build))) return false;
  const prereleaseSeparator = withoutBuild.indexOf("-");
  const core = prereleaseSeparator === -1 ? withoutBuild : withoutBuild.slice(0, prereleaseSeparator);
  const prerelease = prereleaseSeparator === -1 ? null : withoutBuild.slice(prereleaseSeparator + 1);
  if (prerelease !== null && !validIdentifiers(prerelease)) return false;
  const components = core.split(".");
  return components.length >= 2 && components.length <= 4 && components.every((part) =>
    part.length > 0 && part.length <= 10 && /^\d+$/.test(part) && (part.length === 1 || !part.startsWith("0")));
}

function validIdentifiers(value: string): boolean {
  return value.length > 0 && value.length <= 32 && value.split(".").every((part) =>
    part.length > 0 && /^[A-Za-z0-9-]+$/.test(part));
}

function decodeInstallation(value: unknown): InstallationSummary | null {
  if (!isRecord(value) || !hasExactKeys(value, [
    "installation_id", "client_kind", "surface", "version", "source", "location_label", "status", "connection_status",
  ])) return null;

  const { installation_id, client_kind, surface, version, source, location_label, status, connection_status } = value;
  if (!isRequestId(installation_id) ||
      !(["codex", "claude_code", "opencode"] as unknown[]).includes(client_kind) ||
      !(["cli", "desktop"] as unknown[]).includes(surface) ||
      !(version === null || validVersion(version)) ||
      !(["path", "user_local_npm", "user_local_native", "scoop", "windows_app_package", "manual_file"] as unknown[]).includes(source) ||
      typeof location_label !== "string" || typeof source !== "string" ||
      location_label !== sourceLabels[source as InstallationSource] ||
      status !== "detected" || connection_status !== "not_verified") return null;
  if (source === "windows_app_package") {
    if (client_kind !== "codex" || surface !== "desktop") return null;
  } else if (surface !== "cli") return null;
  if (source === "manual_file" && version !== null) return null;

  return {
    installation_id,
    client_kind: client_kind as ClientKind,
    surface: surface as ClientSurface,
    version: version as string | null,
    source: source as InstallationSource,
    location_label,
    status: "detected",
    connection_status: "not_verified",
  };
}

function decodeSnapshot(value: unknown, expectedRequestId: string): DiscoverySnapshot | null {
  if (!isRecord(value) || !isRequestId(expectedRequestId) || !hasExactKeys(value, [
    "schema_version", "request_id", "status", "scan_generation", "installations", "warnings",
  ])) return null;
  if (value.schema_version !== 1 || value.request_id !== expectedRequestId ||
      !["not_scanned", "partial"].includes(value.status as string) ||
      !Number.isSafeInteger(value.scan_generation) || (value.scan_generation as number) < 0 ||
      !Array.isArray(value.installations) || value.installations.length > 512 ||
      !Array.isArray(value.warnings)) return null;

  const installations = value.installations.map(decodeInstallation);
  if (installations.some((item) => item === null)) return null;
  const typedInstallations = installations as InstallationSummary[];
  if (typedInstallations.filter((item) => item.source === "manual_file").length > 32) return null;
  const ids = typedInstallations.map((item) => item.installation_id);
  if (new Set(ids).size !== ids.length) return null;

  const warnings: DiscoveryWarning[] = [];
  for (const warning of value.warnings) {
    if (typeof warning !== "string" || !warningCodes.has(warning as DiscoveryWarning) || warnings.includes(warning as DiscoveryWarning)) return null;
    warnings.push(warning as DiscoveryWarning);
  }

  const scanGeneration = value.scan_generation as number;
  if (value.status === "not_scanned") {
    if (scanGeneration !== 0 || typedInstallations.length !== 0 || warnings.length !== 1 || warnings[0] !== "scan_not_run") return null;
  } else if (scanGeneration < 1 || warnings.includes("scan_not_run") ||
      !warnings.includes("scope_partial") || !warnings.includes("absence_is_not_unsupported")) return null;

  return {
    schema_version: 1,
    request_id: expectedRequestId,
    status: value.status as DiscoverySnapshot["status"],
    scan_generation: scanGeneration,
    installations: typedInstallations,
    warnings,
  };
}

function decodeError(value: unknown, expectedRequestId: string): DiscoveryErrorEnvelope | null {
  if (!isRecord(value) || !isRecord(value.error) || !isRequestId(expectedRequestId) ||
      !hasExactKeys(value, ["schema_version", "request_id", "status", "error"]) ||
      !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(discoveryErrors, code)) return null;
  const contract = discoveryErrors[code as DiscoveryErrorCode];
  if (value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "error" ||
      value.error.retryable !== contract.retryable || value.error.message_key !== contract.messageKey ||
      value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: expectedRequestId,
    status: "error",
    error: { code: code as DiscoveryErrorCode, retryable: contract.retryable, message_key: contract.messageKey, diagnostic_id: null },
  };
}

export function decodeDiscoveryReply(value: unknown, expectedRequestId: string):
  | { kind: "snapshot"; snapshot: DiscoverySnapshot }
  | { kind: "discovery_error"; error: DiscoveryErrorEnvelope }
  | null {
  const snapshot = decodeSnapshot(value, expectedRequestId);
  if (snapshot) return { kind: "snapshot", snapshot };
  const error = decodeError(value, expectedRequestId);
  return error ? { kind: "discovery_error", error } : null;
}

export function decodeSelectClientFileReply(value: unknown, expectedRequestId: string):
  | { kind: "snapshot"; snapshot: DiscoverySnapshot }
  | { kind: "discovery_error"; error: DiscoveryErrorEnvelope }
  | { kind: "cancelled"; request_id: string }
  | null {
  const discoveryReply = decodeDiscoveryReply(value, expectedRequestId);
  if (discoveryReply) return discoveryReply;
  if (!isRecord(value) || !isRequestId(expectedRequestId) ||
      !hasExactKeys(value, ["schema_version", "request_id", "status"]) ||
      value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "cancelled") return null;
  return { kind: "cancelled", request_id: expectedRequestId };
}

interface DiscoveryClientOptions {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}

export function createDiscoveryClient(options: DiscoveryClientOptions) {
  async function run(command: "get_discovery_snapshot" | "scan_discovery"): Promise<DiscoveryResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    const requestId = options.createRequestId();
    if (!requestId || !isRequestId(requestId)) return { kind: "request_failed" };
    try {
      const raw = await options.invoke(command, { requestId });
      const reply = decodeDiscoveryReply(raw, requestId);
      if (!reply) return { kind: "invalid_response" };
      return reply.kind === "snapshot" ? { kind: "snapshot", snapshot: reply.snapshot } : reply;
    } catch {
      return { kind: "request_failed" };
    }
  }

  async function selectClientFile(clientKind: ClientKind): Promise<ClientFileSelectionResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    const requestId = options.createRequestId();
    if (!requestId || !isRequestId(requestId)) return { kind: "request_failed" };
    try {
      const raw = await options.invoke("select_client_file", { requestId, clientKind });
      const reply = decodeSelectClientFileReply(raw, requestId);
      if (!reply) return { kind: "invalid_response" };
      return reply;
    } catch {
      return { kind: "request_failed" };
    }
  }

  return {
    isAvailable: () => options.isNative(),
    getDiscoverySnapshot: () => run("get_discovery_snapshot"),
    scanDiscovery: () => run("scan_discovery"),
    selectClientFile,
  };
}

type DiscoveryClient = ReturnType<typeof createDiscoveryClient>;

type DiscoverySessionClient = Pick<DiscoveryClient, "getDiscoverySnapshot" | "scanDiscovery"> &
  Partial<Pick<DiscoveryClient, "selectClientFile">>;

export function createDiscoverySession(client: DiscoverySessionClient) {
  let pendingScan: Promise<DiscoveryResult> | null = null;
  let pendingPicker: Promise<ClientFileSelectionResult> | null = null;
  let lastScan: Promise<DiscoveryResult> | null = null;
  let scanAttempt = 0;

  function scanDiscovery(): Promise<DiscoveryResult> {
    if (pendingScan) return pendingScan;
    if (pendingPicker) return Promise.resolve({ kind: "busy" });
    const current = Promise.resolve().then(() => client.scanDiscovery());
    const wrapped = current.finally(() => {
      if (pendingScan === wrapped) pendingScan = null;
    });
    pendingScan = wrapped;
    lastScan = wrapped;
    scanAttempt += 1;
    return wrapped;
  }

  function selectClientFile(clientKind: ClientKind): Promise<ClientFileSelectionResult> {
    if (pendingScan || pendingPicker) return Promise.resolve({ kind: "busy" });
    if (!client.selectClientFile) return Promise.resolve({ kind: "request_failed" });
    const current = Promise.resolve().then(() => client.selectClientFile!(clientKind));
    const wrapped = current.finally(() => {
      if (pendingPicker === wrapped) pendingPicker = null;
    });
    pendingPicker = wrapped;
    return wrapped;
  }

  async function loadInitial(): Promise<DiscoveryResult> {
    if (pendingScan) return pendingScan;
    if (pendingPicker) {
      await pendingPicker;
      return loadInitial();
    }
    const attemptAtRead = scanAttempt;
    const result = await client.getDiscoverySnapshot();
    if (pendingScan) return pendingScan;
    if (pendingPicker) {
      await pendingPicker;
      return loadInitial();
    }
    if (scanAttempt !== attemptAtRead && lastScan) return lastScan;
    if (result.kind !== "snapshot" || result.snapshot.status !== "not_scanned") return result;
    if (lastScan) return lastScan;
    return scanDiscovery();
  }

  return {
    loadInitial,
    scanDiscovery,
    selectClientFile,
    isScanPending: () => pendingScan !== null,
    isMutationPending: () => pendingScan !== null || pendingPicker !== null,
  };
}

export class DiscoveryRequestSequence {
  private current = 0;
  next(): number { return ++this.current; }
  isCurrent(value: number): boolean { return value === this.current; }
  invalidate(): void { this.current += 1; }
}

function newRequestId(): string | null {
  const requestId = globalThis.crypto?.randomUUID?.();
  return requestId && isRequestId(requestId) ? requestId : null;
}

const nativeDiscoveryClient = createDiscoveryClient({
  isNative: () => isTauri(),
  createRequestId: newRequestId,
  invoke: (command, args) => invoke(command, args),
});

export const isDiscoveryAvailable = nativeDiscoveryClient.isAvailable;
export const getDiscoverySnapshot = nativeDiscoveryClient.getDiscoverySnapshot;
export const scanDiscovery = nativeDiscoveryClient.scanDiscovery;
export const discoverySession = createDiscoverySession(nativeDiscoveryClient);
