import { invoke, isTauri } from "@tauri-apps/api/core";
import { decodeDiscoveryReply, type ClientKind, type DiscoveryErrorEnvelope, type InstallationSummary } from "./integrations.ts";

export type InstallationChoiceStatus = "not_selected" | "unavailable" | "not_scanned" | "available" | "saved";

export interface InstallationChoiceSnapshot {
  schema_version: 1;
  request_id: string;
  status: InstallationChoiceStatus;
  client_kind: ClientKind;
  scan_generation: number | null;
  installation_id: string | null;
}

export type InstallationChoiceResult =
  | { kind: "snapshot"; snapshot: InstallationChoiceSnapshot }
  | { kind: "discovery_error"; error: DiscoveryErrorEnvelope }
  | { kind: "desktop_unavailable" | "request_failed" | "invalid_request" | "invalid_response" };

type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type UnknownRecord = Record<string, unknown>;

const clientKinds = new Set<ClientKind>(["codex", "claude_code", "opencode"]);
const requestKeys = ["schema_version", "request_id", "status", "client_kind", "scan_generation", "installation_id"] as const;
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isClientKind(value: unknown): value is ClientKind {
  return typeof value === "string" && clientKinds.has(value as ClientKind);
}

function isGeneration(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function decodeSnapshot(
  value: unknown,
  requestId: string,
  clientKind: ClientKind,
  operation: "read" | "remember",
): InstallationChoiceSnapshot | null {
  if (!isRecord(value) || !hasExactKeys(value, requestKeys) ||
      value.schema_version !== 1 || value.request_id !== requestId || value.client_kind !== clientKind) return null;
  if (typeof value.status !== "string" ||
      (operation === "read" && !["not_selected", "unavailable", "not_scanned", "available"].includes(value.status)) ||
      (operation === "remember" && value.status !== "saved")) return null;

  const generation = value.scan_generation;
  const installationId = value.installation_id;
  if (!(generation === null || isGeneration(generation)) ||
      !(installationId === null || isUuid(installationId))) return null;

  switch (value.status as InstallationChoiceStatus) {
    case "not_selected":
    case "unavailable":
      if (installationId !== null) return null;
      break;
    case "not_scanned":
      if (generation !== null || installationId !== null) return null;
      break;
    case "available":
    case "saved":
      if (!isGeneration(generation) || !isUuid(installationId)) return null;
      break;
    default:
      return null;
  }

  return {
    schema_version: 1,
    request_id: requestId,
    status: value.status as InstallationChoiceStatus,
    client_kind: clientKind,
    scan_generation: generation as number | null,
    installation_id: installationId as string | null,
  };
}

interface InstallationChoiceClientOptions {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}

export function createInstallationChoiceClient(options: InstallationChoiceClientOptions) {
  async function run(
    command: "get_saved_installation_choice" | "remember_installation_choice",
    request: UnknownRecord,
    requestId: string,
    clientKind: ClientKind,
    operation: "read" | "remember",
    expectedSaved?: { scanGeneration: number; installationId: string },
  ): Promise<InstallationChoiceResult> {
    try {
      const raw = await options.invoke(command, { request });
      const discovery = decodeDiscoveryReply(raw, requestId);
      if (discovery?.kind === "discovery_error") return discovery;
      const snapshot = decodeSnapshot(raw, requestId, clientKind, operation);
      if (snapshot && expectedSaved &&
          (snapshot.scan_generation !== expectedSaved.scanGeneration ||
           snapshot.installation_id !== expectedSaved.installationId)) return { kind: "invalid_response" };
      return snapshot ? { kind: "snapshot", snapshot } : { kind: "invalid_response" };
    } catch {
      return { kind: "request_failed" };
    }
  }

  async function get(clientKind: ClientKind): Promise<InstallationChoiceResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    if (!isClientKind(clientKind)) return { kind: "invalid_request" };
    const requestId = options.createRequestId();
    if (!isUuid(requestId)) return { kind: "request_failed" };
    return run("get_saved_installation_choice", {
      schema_version: 1,
      request_id: requestId,
      client_kind: clientKind,
    }, requestId, clientKind, "read");
  }

  async function remember(input: {
    clientKind: ClientKind;
    scanGeneration: number;
    installationId: string;
  }): Promise<InstallationChoiceResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    if (!isRecord(input) || !hasExactKeys(input, ["clientKind", "scanGeneration", "installationId"]) ||
        !isClientKind(input.clientKind) || !isGeneration(input.scanGeneration) || !isUuid(input.installationId)) {
      return { kind: "invalid_request" };
    }
    const requestId = options.createRequestId();
    if (!isUuid(requestId)) return { kind: "request_failed" };
    return run("remember_installation_choice", {
      schema_version: 1,
      request_id: requestId,
      scan_generation: input.scanGeneration,
      installation_id: input.installationId,
    }, requestId, input.clientKind, "remember", {
      scanGeneration: input.scanGeneration,
      installationId: input.installationId,
    });
  }

  return { get, remember };
}

export function resolvePreferredInstallation(
  result: InstallationChoiceResult,
  candidates: readonly Pick<InstallationSummary, "installation_id" | "client_kind">[],
  clientKind: ClientKind,
  scanGeneration: number,
): string | null {
  if (result.kind !== "snapshot" || !isClientKind(clientKind) || !isGeneration(scanGeneration)) return null;
  const { snapshot } = result;
  if (snapshot.client_kind !== clientKind || snapshot.scan_generation !== scanGeneration) return null;

  if (snapshot.status === "not_selected") {
    const matching = candidates.filter((candidate) => candidate.client_kind === clientKind);
    if (matching.length !== 1 || !isUuid(matching[0].installation_id)) return null;
    return candidates.filter((candidate) => candidate.installation_id === matching[0].installation_id).length === 1
      ? matching[0].installation_id
      : null;
  }
  if (snapshot.status !== "available" || !isUuid(snapshot.installation_id)) return null;
  const matches = candidates.filter((candidate) => candidate.installation_id === snapshot.installation_id);
  return matches.length === 1 && matches[0].client_kind === clientKind ? snapshot.installation_id : null;
}

function newRequestId(): string | null {
  const value = globalThis.crypto?.randomUUID?.();
  return value && isUuid(value) ? value : null;
}

export function createNativeInstallationChoiceClient(): ReturnType<typeof createInstallationChoiceClient> {
  return createInstallationChoiceClient({
    isNative: () => isTauri(),
    createRequestId: newRequestId,
    invoke: (command, args) => invoke(command, args),
  });
}
