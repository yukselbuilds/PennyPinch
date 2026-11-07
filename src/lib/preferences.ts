import { invoke, isTauri } from "@tauri-apps/api/core";

export type PreferenceLanguage = "tr" | "en";
export type PreferenceTheme = "system" | "light" | "dark";

export interface UserPreferences {
  language: PreferenceLanguage;
  theme: PreferenceTheme;
  operating: OperatingPreferences;
}

export interface OperatingPreferences {
  close_to_tray: boolean;
  retention_days: 7 | 30 | 90;
  protected_tools: string[];
}
export const defaultOperatingPreferences = (): OperatingPreferences => ({close_to_tray:true,retention_days:30,protected_tools:[]});
export function decodeOperatingPreferences(value: unknown): OperatingPreferences | null {
  if (!isRecord(value) || !hasExactKeys(value,["close_to_tray","retention_days","protected_tools"])
    ||typeof value.close_to_tray!=="boolean"||![7,30,90].includes(value.retention_days as number)||!Array.isArray(value.protected_tools)
    ||value.protected_tools.length>32||!value.protected_tools.every((name,index,all)=>typeof name==="string"&&/^[a-zA-Z0-9_.:/-]{1,96}$/.test(name)&&(index===0||all[index-1]<name)))return null;
  return {close_to_tray:value.close_to_tray,retention_days:value.retention_days as 7|30|90,protected_tools:[...value.protected_tools]};
}

export interface RuntimeManifestReference {
  engine_version: "0.41.0";
  manifest_sha256: "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908";
  mode: "cache";
  savings_profile: "coding";
  ccr_ttl_seconds: 7200;
}

export interface PreferencesSnapshot {
  schema_version: 1;
  request_id: string;
  status: "ok";
  revision: number;
  state_fingerprint: string | null;
  preferences: UserPreferences;
  runtime_manifest: RuntimeManifestReference;
}

export type PreferenceErrorCode =
  | "validation_failed"
  | "unsupported_schema"
  | "environment_blocked"
  | "busy"
  | "config_conflict";

export const PREFERENCE_ERROR_MESSAGES = {
  validation_failed: { retryable: false, messageKey: "preferences.validation_failed" },
  unsupported_schema: { retryable: false, messageKey: "preferences.unsupported_schema" },
  environment_blocked: { retryable: false, messageKey: "preferences.environment_blocked" },
  busy: { retryable: true, messageKey: "preferences.busy" },
  config_conflict: { retryable: false, messageKey: "preferences.config_conflict" },
} as const;

export interface PreferencesErrorEnvelope {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: PreferenceErrorCode;
    retryable: (typeof PREFERENCE_ERROR_MESSAGES)[PreferenceErrorCode]["retryable"];
    message_key: (typeof PREFERENCE_ERROR_MESSAGES)[PreferenceErrorCode]["messageKey"];
    diagnostic_id: null;
  };
}

export type PreferencesResult =
  | { kind: "desktop_unavailable" }
  | { kind: "snapshot"; snapshot: PreferencesSnapshot }
  | { kind: "preferences_error"; error: PreferencesErrorEnvelope }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

export class PreferencesRequestSequence {
  private read = 0;
  private write = 0;

  nextRead(): number { return ++this.read; }
  isReadCurrent(sequence: number): boolean { return this.read === sequence; }
  nextWrite(): number { return ++this.write; }
  isWriteCurrent(sequence: number): boolean { return this.write === sequence; }
  invalidate(): void {
    this.read += 1;
    this.write += 1;
  }
}

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

interface ClientOptions {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length && actual.every((key, index) => key === required[index]);
}

function isRequestId(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function isFingerprint(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}

function decodePreferences(value: unknown): UserPreferences | null {
  if (!isRecord(value) || !(hasExactKeys(value, ["language", "theme"]) || hasExactKeys(value,["language","theme","operating"]))) return null;
  if ((value.language !== "tr" && value.language !== "en") ||
      (value.theme !== "system" && value.theme !== "light" && value.theme !== "dark")) return null;
  const operating=Object.hasOwn(value,"operating")?decodeOperatingPreferences(value.operating):defaultOperatingPreferences();
  return operating?{ language: value.language, theme: value.theme, operating }:null;
}

function decodeManifest(value: unknown): RuntimeManifestReference | null {
  if (!isRecord(value) || !hasExactKeys(value, [
    "engine_version", "manifest_sha256", "mode", "savings_profile", "ccr_ttl_seconds",
  ])) return null;
  if (value.engine_version !== "0.41.0" ||
      value.manifest_sha256 !== "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908" ||
      value.mode !== "cache" || value.savings_profile !== "coding" || value.ccr_ttl_seconds !== 7200) return null;
  return {
    engine_version: "0.41.0",
    manifest_sha256: "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908",
    mode: "cache",
    savings_profile: "coding",
    ccr_ttl_seconds: 7200,
  };
}

export function decodePreferencesSnapshot(value: unknown, expectedRequestId: string): PreferencesSnapshot | null {
  if (!isRecord(value) || !isRequestId(expectedRequestId) ||
      !hasExactKeys(value, [
        "schema_version", "request_id", "status", "revision", "state_fingerprint", "preferences", "runtime_manifest",
      ])) return null;
  const preferences = decodePreferences(value.preferences);
  const runtimeManifest = decodeManifest(value.runtime_manifest);
  if (value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "ok" ||
      typeof value.revision !== "number" || !Number.isSafeInteger(value.revision) || value.revision < 0 ||
      !(value.state_fingerprint === null || isFingerprint(value.state_fingerprint)) ||
      !preferences || !runtimeManifest) return null;
  return {
    schema_version: 1,
    request_id: expectedRequestId,
    status: "ok",
    revision: value.revision,
    state_fingerprint: value.state_fingerprint,
    preferences,
    runtime_manifest: runtimeManifest,
  };
}

export function decodePreferencesError(value: unknown, expectedRequestId: string): PreferencesErrorEnvelope | null {
  if (!isRecord(value) || !isRecord(value.error) || !isRequestId(expectedRequestId) ||
      !hasExactKeys(value, ["schema_version", "request_id", "status", "error"]) ||
      !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(PREFERENCE_ERROR_MESSAGES, code)) return null;
  const contract = PREFERENCE_ERROR_MESSAGES[code as PreferenceErrorCode];
  if (value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "error" ||
      value.error.retryable !== contract.retryable || value.error.message_key !== contract.messageKey ||
      value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: expectedRequestId,
    status: "error",
    error: {
      code: code as PreferenceErrorCode,
      retryable: contract.retryable,
      message_key: contract.messageKey,
      diagnostic_id: null,
    },
  };
}

export function createPreferencesClient(options: ClientOptions) {
  async function invokePreferences(command: "get_preferences" | "save_preferences", args: Record<string, unknown> = {}): Promise<PreferencesResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    const requestId = options.createRequestId();
    if (!requestId || !isRequestId(requestId)) return { kind: "request_failed" };
    try {
      const raw = await options.invoke(command, { requestId, ...args });
      const snapshot = decodePreferencesSnapshot(raw, requestId);
      if (snapshot) return { kind: "snapshot", snapshot };
      const error = decodePreferencesError(raw, requestId);
      return error ? { kind: "preferences_error", error } : { kind: "invalid_response" };
    } catch {
      return { kind: "request_failed" };
    }
  }

  return {
    readPreferences: () => invokePreferences("get_preferences"),
    savePreferences: (preferences: UserPreferences, expectedRevision: number, expectedFingerprint: string | null) => {
      if (!Number.isSafeInteger(expectedRevision) || expectedRevision < 0 ||
          !(expectedFingerprint === null || isFingerprint(expectedFingerprint)) || !decodePreferences(preferences)) {
        return Promise.resolve<PreferencesResult>({ kind: "invalid_response" });
      }
      return invokePreferences("save_preferences", {
        language: preferences.language,
        theme: preferences.theme,
        ...(Object.hasOwn(preferences,"operating")?{operating:preferences.operating}:{}),
        expectedRevision,
        expectedFingerprint,
      });
    },
  };
}

function newRequestId(): string | null {
  const requestId = globalThis.crypto?.randomUUID?.();
  return requestId && isRequestId(requestId) ? requestId : null;
}

const nativePreferencesClient = createPreferencesClient({
  isNative: () => isTauri(),
  createRequestId: newRequestId,
  invoke: (command, args) => invoke(command, args),
});

export const readPreferences = nativePreferencesClient.readPreferences;
export const savePreferences = nativePreferencesClient.savePreferences;
