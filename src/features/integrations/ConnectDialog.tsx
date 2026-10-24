import { useEffect, useRef, useState } from "react";
import { connectionPlanClient } from "../../lib/connectionPlan";
import { connectionActionClient } from "../../lib/connectionActions";
import {
  createNativeClaudeContextClient,
  type ClaudeContextScope,
  type ClaudeContextState,
  type ClaudeSettingsSnapshot,
} from "../../lib/claudeContext";
import { ClaudeScopeSection } from "./ClaudeScopeSection";
import {
  canApplyForConnectionScope,
  candidateSelectionKey,
  initialInstallationSelection,
  isConnectionPlanExpired,
} from "../../lib/connectionPlanView";
import type {
  ConnectionPlan,
  ConnectionActionErrorCode,
  ConnectionOwnership,
  ConnectionPlanErrorCode,
  ConnectionPlanResult,
} from "../../lib/contracts";
import type { PreferenceLanguage } from "../../lib/preferences";
import { isDiscoveryAvailable, type ClientKind, type InstallationSummary, type InstallationSource } from "../../lib/integrations";

type Copy = {
  title: string;
  close: string;
  scope: string;
  scopeNote: string;
  installation: string;
  chooseInstallation: string;
  noInstallations: string;
  versionUnknown: string;
  cli: string;
  desktop: string;
  preview: string;
  refresh: string;
  refreshing: string;
  idle: string;
  previewAvailable: string;
  blocked: string;
  expired: string;
  expires: string;
  endpoint: string;
  current: string;
  proposed: string;
  source: string;
  presence: string;
  present: string;
  missing: string;
  newFile: string;
  model: string;
  reasoning: string;
  provider: string;
  auth: string;
  agentCount: string;
  unchanged: string;
  declared: string;
  notDeclared: string;
  unknown: string;
  authUnknown: string;
  authMethod: string;
  authSecret: string;
  authHelper: string;
  runtimeEffective: string;
  hookShell: string;
  hookAbsent: string;
  hookPresent: string;
  sessionEffects: string;
  sessionRestart: string;
  routeBoundary: string;
  apply: string;
  applyUnavailable: string;
  ownershipTitle: string;
  ownershipLoading: string;
  ownershipNone: string;
  ownershipReleased: string;
  ownershipActive: string;
  ownershipNotVerified: string;
  ownershipLimits: string;
  ownershipSource: string;
  ownershipPort: string;
  ownershipReadError: string;
  ownershipRefreshRequired: string;
  reloadOwnership: string;
  reloadingOwnership: string;
  undo: string;
  undoing: string;
  applySuccess: string;
  unchangedSuccess: string;
  undoSuccess: string;
  applying: string;
  actionUnavailable: string;
  actionFailure: string;
  actionUncertain: string;
  actionErrors: Record<ConnectionActionErrorCode, string>;
  loading: string;
  desktopUnavailable: string;
  invalidRequest: string;
  invalidResponse: string;
  requestFailed: string;
  errors: Record<ConnectionPlanErrorCode, string>;
  blockers: Record<ConnectionPlan["blockers"][number], string>;
  sources: Record<ConnectionPlan["config_source"], string>;
  installationSources: Record<InstallationSource, string>;
  endpointBefores: Record<ConnectionPlan["endpoint_field"]["before"], string>;
  opencodeEndpointLabel: string;
  advancedDetails: string;
  routeCoverageTitle: string;
  routeEligible: string;
  routeExcluded: string;
  routeResponses: string;
  routeChatCompletions: string;
  routeConfiguredDefault: string;
  routeDefaultCovered: string;
  routeDefaultExcluded: string;
  routeDefaultUnknown: string;
  routeDefaultExcludedNote: string;
  routePartialNote: string;
  routeRuntimeUnknown: string;
  targetModel: string;
  targetReasoning: string;
  targetAuth: string;
  claudeUserScope: string;
  claudeProjectScope: string;
  refreshClaude: string;
};

const copy: Record<PreferenceLanguage, Copy> = {
  tr: {
    title: "Bağlantı önizlemesi", close: "Kapat", scope: "Kapsam: kullanıcı yapılandırması",
    scopeNote: "Yalnız kullanıcı yapılandırması incelenir; diğer kapsamlar ve açık oturumlar bilinmiyor.",
    installation: "Kurulum", chooseInstallation: "Bir kurulum seç", noInstallations: "Bu istemci için seçilebilir kurulum kalmadı.",
    versionUnknown: "Sürüm bilinmiyor", cli: "Komut satırı", desktop: "Masaüstü",
    preview: "Önizle", refresh: "Önizlemeyi yenile", refreshing: "Önizleme hazırlanıyor…", idle: "Henüz önizleme oluşturulmadı.",
    previewAvailable: "Önizleme hazır; bağlantı kurulmadı.", blocked: "Önizleme engellendi", expired: "Önizleme süresi doldu. Devam etmeden önce yenile.",
    expires: "Geçerlilik sonu", endpoint: "Gerekli endpoint alanı", current: "Mevcut durum", proposed: "Önerilen değer",
    source: "Yapılandırma türü", presence: "Dosya durumu", present: "Mevcut", missing: "Eksik", newFile: "Yeni kullanıcı yapılandırma dosyası gerekir.",
    model: "Model bildirimi", reasoning: "Reasoning bildirimi", provider: "Sağlayıcı", auth: "Kimlik doğrulama bildirimi",
    agentCount: "Ajan sayısı", unchanged: "Değişmeden kalır", declared: "Bildirilmiş", notDeclared: "Bildirilmemiş", unknown: "Bilinmiyor",
    authUnknown: "Bilinmiyor", authMethod: "Yöntem bildirimi", authSecret: "Kimlik bilgisi tanımlı", authHelper: "Yardımcı bildirimi",
    runtimeEffective: "Çalışma anında etkin değer", hookShell: "Hook / shell etkisi", hookAbsent: "Yok", hookPresent: "Var; değiştirilmeden kalır",
    sessionEffects: "Oturum etkisi", sessionRestart: "Yeni bir istemci süreci gerekir; mevcut oturumun bu ayarı kullandığı doğrulanmadı.",
    routeBoundary: "Canlı bağlantı doğrulanmadı; ana/alt ajanların veya diğer oturumların aynı rotayı kullandığı bilinmiyor.",
    apply: "Uygula", applyUnavailable: "Güncel sahiplik kontrolü ve geçerli önizleme gerekir.",
    ownershipTitle: "Yönlendirme kaydı", ownershipLoading: "Sahiplik kaydı okunuyor…",
    ownershipNone: "Bu istemci için etkin bir Headroom sahiplik kaydı yok.",
    ownershipReleased: "Yönlendirme kaydı geri alınmış.", ownershipActive: "Yönlendirme kaydı etkin.",
    ownershipNotVerified: "Bu kayıt trafik kullanıldığını doğrulamaz.", ownershipSource: "Kaynak", ownershipPort: "Yerel port",
    ownershipLimits: "Headroom ve motor açık kalmalı. Uygulama kapanınca kayıt otomatik geri alınmaz.",
    ownershipReadError: "Sahiplik kaydı okunamadı. Önizleme salt okunur olarak kullanılabilir.",
    ownershipRefreshRequired: "İşlem sonucu kesinleşmedi. Yeni bir geri alma işleminden önce kaydı yeniden oku.",
    reloadOwnership: "Kaydı yeniden oku", reloadingOwnership: "Kayıt okunuyor…", undo: "Geri al", undoing: "Geri alınıyor…",
    applySuccess: "Yönlendirme kaydedildi.", unchangedSuccess: "Yönlendirme zaten aynıydı; yeni sahiplik kaydı oluşturulmadı.",
    undoSuccess: "Yönlendirme geri alındı.", applying: "Yönlendirme kaydediliyor…",
    actionUnavailable: "İşlem bu ortamda kullanılamıyor.", actionFailure: "İşlem tamamlanamadı. Kaydı yeniden oku.",
    actionUncertain: "Sonuç belirsiz. Kaydı yeniden oku; otomatik tekrar yapılmadı.",
    actionErrors: {
      validation_failed: "İşlem isteği doğrulanamadı.", unsupported_schema: "İşlem sürümü desteklenmiyor.",
      busy: "Başka bir bağlantı işlemi sürüyor.", not_found: "Önizleme veya sahiplik kaydı artık bulunamıyor.",
      plan_expired: "Önizlemenin süresi doldu. Yeniden önizle.", config_conflict: "Yapılandırma değişti. Güncel kaydı ve önizlemeyi yenile.",
      config_invalid: "Yapılandırma artık geçerli değil.", config_unsafe: "Yapılandırma güvenli biçimde değiştirilemiyor.",
      config_unavailable: "Yapılandırma okunamıyor.", unsupported_filesystem: "Bu dosya sistemi güvenli bağlantı değişikliğini desteklemiyor.",
      commit_unknown: "Yazım sonucu belirsiz. Kaydı yeniden oku.",
    },
    loading: "Kullanıcı yapılandırması inceleniyor…", desktopUnavailable: "Bu önizleme için Headroom masaüstü bağlantısı gerekir.",
    invalidRequest: "Önizleme isteği doğrulanamadı.", invalidResponse: "Önizleme yanıtı doğrulanamadı.", requestFailed: "Önizleme tamamlanamadı. Yenileyip tekrar deneyebilirsin.",
    errors: {
      validation_failed: "Önizleme isteği doğrulanamadı.", unsupported_scope: "Bu yapılandırma kapsamı desteklenmiyor.",
      stale_selection: "Kurulum veya tarama değişti. Önizlemeyi yeni seçimle yenile.", not_found: "Seçilen kurulum artık bulunamıyor.",
      endpoint_unavailable: "Headroom yerel endpoint'i hazır değil.", config_unavailable: "Kullanıcı yapılandırması okunamıyor.",
      config_unsafe: "Yapılandırma güvenli biçimde okunamadı.", config_too_large: "Yapılandırma izin verilen boyutu aşıyor.",
      config_conflict: "Yapılandırma değişti. Güncel durumu yeniden önizle.", config_invalid: "Yapılandırma çözümlenemedi.",
      busy: "Başka bir önizleme işlemi sürüyor. Biraz sonra yeniden dene.",
    },
    blockers: {
      unsupported_schema: "Yapılandırma şeması desteklenmiyor.", custom_gateway: "Özel gateway rotası bu önizlemede desteklenmiyor.",
      unsupported_route: "Bu istemci rotası henüz doğrulanmadı.", client_version_unknown: "İstemci sürümü bilinmiyor; plan uygulanamaz.",
      unsupported_client_version: "Bu istemci sürümü henüz doğrulanmadı.", unsupported_client_surface: "Bu istemci yüzeyi henüz doğrulanmadı.",
    },
    sources: { codex_toml: "Codex TOML", claude_json: "Claude JSON", opencode_json: "OpenCode JSON", opencode_jsonc: "OpenCode JSONC", opencode_missing: "OpenCode yapılandırması eksik" },
    installationSources: { path: "PATH", user_local_npm: "Kullanıcıya ait npm", user_local_native: "Kullanıcıya ait kurulum", scoop: "Scoop", windows_app_package: "Windows uygulama paketi", manual_file: "Seçilen dosya" },
    endpointBefores: { missing: "Eksik", same_stable_endpoint: "Aynı yerel endpoint", other_configured_redacted: "Başka bir değer yapılandırılmış (gizli)", unknown: "Bilinmiyor", provider_routes: "Uygun yapılandırılmış sağlayıcı rotaları" },
    opencodeEndpointLabel: "Uygun sağlayıcıların options.baseURL alanları",
    advancedDetails: "Gelişmiş ayrıntılar",
    routeCoverageTitle: "Bu önizlemenin kapsadığı rotalar",
    routeEligible: "Uygun rota sayısı",
    routeExcluded: "Dışarıda kalan rota sayısı",
    routeResponses: "Responses API",
    routeChatCompletions: "Chat Completions",
    routeConfiguredDefault: "Yapılandırılmış varsayılan rota",
    routeDefaultCovered: "Kapsanıyor",
    routeDefaultExcluded: "Dışarıda",
    routeDefaultUnknown: "Bilinmiyor",
    routeDefaultExcludedNote: "Yapılandırılmış varsayılan modelin trafiği bu dosya değişikliğiyle Headroom üzerinden yönlendirilmez.",
    routePartialNote: "Kapsam kısmi; dışarıda kalan veya gözlenmeyen rotalar değiştirilmez.",
    routeRuntimeUnknown: "Dosya gözlemi, mevcut sürecin etkin rotasını doğrulamaz.",
    targetModel: "Hedef dosyadaki model bildirimi", targetReasoning: "Hedef dosyadaki reasoning bildirimi", targetAuth: "Hedef dosyadaki kimlik doğrulama bildirimi",
    claudeUserScope: "Kapsam: kullanıcı ayarları", claudeProjectScope: "Kapsam: proje ayarları", refreshClaude: "Bağlamı ve kayıtları yenile",
  },
  en: {
    title: "Connection preview", close: "Close", scope: "Scope: user configuration",
    scopeNote: "Only user configuration is inspected. Other scopes and effective settings in open sessions are unknown.",
    installation: "Installation", chooseInstallation: "Choose an installation", noInstallations: "No selectable installation remains for this client.",
    versionUnknown: "Version unknown", cli: "Command line", desktop: "Desktop",
    preview: "Preview", refresh: "Refresh preview", refreshing: "Preparing preview…", idle: "No preview has been created.",
    previewAvailable: "Preview available; no connection was made.", blocked: "Preview blocked", expired: "This preview expired. Refresh before continuing.",
    expires: "Expires", endpoint: "Required endpoint field", current: "Current state", proposed: "Proposed value",
    source: "Configuration type", presence: "File state", present: "Present", missing: "Missing", newFile: "A new user configuration file will be needed.",
    model: "Model declaration", reasoning: "Reasoning declaration", provider: "Provider", auth: "Authentication declaration",
    agentCount: "Agent count", unchanged: "Preserved", declared: "Declared", notDeclared: "Not declared", unknown: "Unknown",
    authUnknown: "Unknown", authMethod: "Method declaration", authSecret: "Credential declared", authHelper: "Helper declaration",
    runtimeEffective: "Effective at runtime", hookShell: "Hook / shell effect", hookAbsent: "Absent", hookPresent: "Present; unchanged",
    sessionEffects: "Session effect", sessionRestart: "A new client process is required; adoption by the current session is unknown.",
    routeBoundary: "A live connection was not verified; use of the same route by main/subagents or other sessions is unknown.",
    apply: "Apply", applyUnavailable: "A fresh ownership check and valid preview are required.",
    ownershipTitle: "Routing record", ownershipLoading: "Reading ownership record…",
    ownershipNone: "There is no active Headroom ownership record for this client.",
    ownershipReleased: "The routing record has been released.", ownershipActive: "The routing record is active.",
    ownershipNotVerified: "This record does not confirm that traffic uses it.", ownershipSource: "Source", ownershipPort: "Local port",
    ownershipLimits: "Keep Headroom and its engine running. The record is not automatically undone when the app closes.",
    ownershipReadError: "The ownership record could not be read. Read-only preview remains available.",
    ownershipRefreshRequired: "The operation outcome is uncertain. Reload the record before another undo.",
    reloadOwnership: "Reload record", reloadingOwnership: "Reading record…", undo: "Undo", undoing: "Undoing…",
    applySuccess: "Routing saved.", unchangedSuccess: "Routing was already the same; no new ownership record was created.",
    undoSuccess: "Routing released.", applying: "Saving routing…",
    actionUnavailable: "This action is unavailable in this environment.", actionFailure: "The action could not be completed. Reload the record.",
    actionUncertain: "The outcome is uncertain. Reload the record; no automatic retry was made.",
    actionErrors: {
      validation_failed: "The action request could not be validated.", unsupported_schema: "This action version is unsupported.",
      busy: "Another connection action is in progress.", not_found: "The preview or ownership record is no longer available.",
      plan_expired: "The preview expired. Create a new preview.", config_conflict: "Configuration changed. Reload the record and preview current state.",
      config_invalid: "The configuration is no longer valid.", config_unsafe: "The configuration cannot be changed safely.",
      config_unavailable: "The configuration could not be read.", unsupported_filesystem: "This file system does not support a safe connection change.",
      commit_unknown: "The write outcome is uncertain. Reload the record.",
    },
    loading: "Inspecting user configuration…", desktopUnavailable: "The Headroom desktop bridge is required for this preview.",
    invalidRequest: "The preview request could not be validated.", invalidResponse: "The preview response could not be validated.", requestFailed: "The preview could not be completed. Refresh and try again.",
    errors: {
      validation_failed: "The preview request could not be validated.", unsupported_scope: "This configuration scope is unsupported.",
      stale_selection: "The installation or scan changed. Refresh the preview for the current selection.", not_found: "The selected installation is no longer available.",
      endpoint_unavailable: "The Headroom local endpoint is not ready.", config_unavailable: "User configuration could not be read.",
      config_unsafe: "Configuration could not be read safely.", config_too_large: "Configuration exceeds the allowed size.",
      config_conflict: "Configuration changed. Preview the current state again.", config_invalid: "Configuration could not be parsed.",
      busy: "Another preview operation is in progress. Try again shortly.",
    },
    blockers: {
      unsupported_schema: "The configuration schema is unsupported.", custom_gateway: "A custom gateway route is unsupported in this preview.",
      unsupported_route: "This client route has not been verified.", client_version_unknown: "Client version is unknown; the plan cannot be applied.",
      unsupported_client_version: "This client version has not been verified.", unsupported_client_surface: "This client surface has not been verified.",
    },
    sources: { codex_toml: "Codex TOML", claude_json: "Claude JSON", opencode_json: "OpenCode JSON", opencode_jsonc: "OpenCode JSONC", opencode_missing: "OpenCode configuration missing" },
    installationSources: { path: "PATH", user_local_npm: "User-local npm", user_local_native: "User-local install", scoop: "Scoop", windows_app_package: "Windows app package", manual_file: "Selected file" },
    endpointBefores: { missing: "Missing", same_stable_endpoint: "Same local endpoint", other_configured_redacted: "Another value is configured (redacted)", unknown: "Unknown", provider_routes: "Eligible configured provider routes" },
    opencodeEndpointLabel: "Eligible provider options.baseURL fields",
    advancedDetails: "Advanced details",
    routeCoverageTitle: "Routes covered by this preview",
    routeEligible: "Eligible routes",
    routeExcluded: "Excluded routes",
    routeResponses: "Responses API",
    routeChatCompletions: "Chat Completions",
    routeConfiguredDefault: "Configured default route",
    routeDefaultCovered: "Covered",
    routeDefaultExcluded: "Excluded",
    routeDefaultUnknown: "Unknown",
    routeDefaultExcludedNote: "Traffic for the configured default model will not use Headroom through this file change.",
    routePartialNote: "Coverage is partial; excluded or unobserved routes are left unchanged.",
    routeRuntimeUnknown: "File observations do not verify the route used by a running process.",
    targetModel: "Target-file model declaration", targetReasoning: "Target-file reasoning declaration", targetAuth: "Target-file authentication declaration",
    claudeUserScope: "Scope: user settings", claudeProjectScope: "Scope: project settings", refreshClaude: "Refresh context and records",
  },
};

const clientTitles: Record<ClientKind, string> = { codex: "Codex", claude_code: "Claude Code", opencode: "OpenCode" };
const endpointLabels: Record<ConnectionPlan["endpoint_field"]["field"], string> = {
  codex_openai_base_url: "openai_base_url",
  claude_anthropic_base_url: "env.ANTHROPIC_BASE_URL",
  opencode_v1_openai_base_url: "provider options.baseURL",
};

type PreviewContext = { scanGeneration: number; installationIdsKey: string; scope: ConnectionPlan["scope"] };
type PreviewState = { context: PreviewContext; result: ConnectionPlanResult };
type OwnershipView =
  | { kind: "loading" }
  | { kind: "ready"; ownership: ConnectionOwnership | null }
  | { kind: "error"; message: string }
  | { kind: "refresh_required" };
type ActionFeedback = { kind: "success" | "error" | "uncertain"; message: string };

function actionClientKind(clientKind: ClientKind): ConnectionPlan["client_kind"] {
  return clientKind === "opencode" ? "opencode_v1" : clientKind;
}

function actionResultMessage(
  result: Awaited<ReturnType<typeof connectionActionClient.snapshot>>,
  text: Copy,
): { message: string; uncertain: boolean } {
  if (result.kind === "action_error") {
    return {
      message: text.actionErrors[result.error.code],
      uncertain: result.error.code === "commit_unknown",
    };
  }
  if (result.kind === "desktop_unavailable") return { message: text.actionUnavailable, uncertain: false };
  if (result.kind === "invalid_request") return { message: text.actionFailure, uncertain: false };
  if (result.kind === "invalid_response" || result.kind === "request_failed") {
    return { message: text.actionUncertain, uncertain: true };
  }
  return { message: text.actionFailure, uncertain: true };
}

function declarationLabel(value: ConnectionPlan["model"], text: Copy): string {
  return value === "present" ? text.declared : value === "absent" ? text.notDeclared : text.unknown;
}

function authLabel(value: ConnectionPlan["auth"], text: Copy): string {
  const labels: Record<ConnectionPlan["auth"], string> = {
    unknown: text.authUnknown,
    method_declared: text.authMethod,
    secret_configured: text.authSecret,
    helper_declared: text.authHelper,
  };
  return labels[value];
}

function planResultMessage(result: ConnectionPlanResult, text: Copy): string {
  switch (result.kind) {
    case "desktop_unavailable": return text.desktopUnavailable;
    case "invalid_request": return text.invalidRequest;
    case "invalid_response": return text.invalidResponse;
    case "request_failed": return text.requestFailed;
    case "plan_error": return text.errors[result.error.code];
    case "preview": return result.plan.status === "blocked" ? text.blocked : text.previewAvailable;
  }
}

const claudeContextClient = createNativeClaudeContextClient();

export function ConnectDialog({
  language,
  clientKind,
  installations,
  scanGeneration,
  onClose,
}: {
  language: PreferenceLanguage;
  clientKind: ClientKind;
  installations: InstallationSummary[];
  scanGeneration: number;
  onClose: () => void;
}) {
  const text = copy[language];
  const dialogRef = useRef<HTMLDialogElement>(null);
  const selectRef = useRef<HTMLSelectElement>(null);
  const previewRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef<HTMLElement | null>(null);
  const mounted = useRef(false);
  const requestSequence = useRef(0);
  const ownershipSequence = useRef(0);
  const claudeSequence = useRef(0);
  const bridgeLock = useRef(false);
  const mutationLock = useRef(false);
  const scopePickerLock = useRef(false);
  const pendingAction = useRef<"apply" | "undo" | null>(null);
  const idsKey = candidateSelectionKey(installations);
  const [selectedId, setSelectedId] = useState(() => initialInstallationSelection(installations));
  const [previewState, setPreviewState] = useState<PreviewState | null>(null);
  const [busyContext, setBusyContext] = useState<PreviewContext | null>(null);
  const [ownershipView, setOwnershipView] = useState<OwnershipView>({ kind: "loading" });
  const [ownershipLoading, setOwnershipLoading] = useState(false);
  const [bridgePending, setBridgePending] = useState(false);
  const [claudeContext, setClaudeContext] = useState<ClaudeContextState | null>(null);
  const claudeContextRef = useRef<ClaudeContextState | null>(null);
  const [claudeSettings, setClaudeSettings] = useState<ClaudeSettingsSnapshot | null>(null);
  const [claudeLoading, setClaudeLoading] = useState(false);
  const [claudePickerPending, setClaudePickerPending] = useState(false);
  const [claudeError, setClaudeError] = useState<string | null>(null);
  const [mutationPending, setMutationPending] = useState(false);
  const [actionFeedback, setActionFeedback] = useState<ActionFeedback | null>(null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const currentContext = { scanGeneration, installationIdsKey: idsKey };
  const isClaude = clientKind === "claude_code";
  const isOpenCode = clientKind === "opencode";
  const currentScope: ConnectionPlan["scope"] = isClaude
    ? claudeContext?.scope ?? "user_config"
    : "user_config";
  const result = previewState
    && previewState.context.scanGeneration === currentContext.scanGeneration
    && previewState.context.installationIdsKey === currentContext.installationIdsKey
    && previewState.context.scope === currentScope
    ? previewState.result : null;
  const busy = busyContext !== null
    && busyContext.scanGeneration === currentContext.scanGeneration
    && busyContext.installationIdsKey === currentContext.installationIdsKey;
  const selected = installations.find((item) => item.installation_id === selectedId);
  const plan = result?.kind === "preview" ? result.plan : null;
  const openCodeCoverage = plan?.client_kind === "opencode_v1" ? plan.opencode_coverage : undefined;
  const expired = plan !== null && isConnectionPlanExpired(plan, nowMs);
  const previewAvailable = installations.length > 0;
  const ownership = ownershipView.kind === "ready" ? ownershipView.ownership : null;
  const activeOwnership = ownership?.state === "active" ? ownership : null;
  const nativeAvailable = isDiscoveryAvailable();
  const canApply = Boolean(plan && plan.status === "preview" && !expired
    && plan.scope === currentScope
    && canApplyForConnectionScope(plan.scope, ownership)
    && (!isClaude || (claudeContext !== null && claudeError === null && claudeSettings !== null))
    && ownershipView.kind === "ready" && !ownershipLoading && nativeAvailable && !mutationPending && !bridgePending);

  const planTechnicalDetails = plan && <>
    <section className="connection-dialog__section" aria-label={text.endpoint}>
      <h3>{text.endpoint}</h3>
      <dl className="connection-dialog__endpoint">
        <div><dt>{text.current}</dt><dd>{text.endpointBefores[plan.endpoint_field.before]}</dd></div>
        <div><dt>{text.proposed}</dt><dd><code>{isOpenCode ? text.opencodeEndpointLabel : endpointLabels[plan.endpoint_field.field]}</code><code>{plan.endpoint_field.proposed_value}</code></dd></div>
      </dl>
    </section>
    <section className="connection-dialog__section">
      <h3>{text.source}</h3>
      <dl className="connection-dialog__details">
        <div><dt>{text.source}</dt><dd>{text.sources[plan.config_source]}</dd></div>
        <div><dt>{text.presence}</dt><dd>{plan.config_presence === "present" ? text.present : text.missing}</dd></div>
        <div><dt>{isClaude ? text.targetModel : text.model}</dt><dd>{declarationLabel(plan.model, text)} · {text.unchanged}</dd></div>
        <div><dt>{isClaude ? text.targetReasoning : text.reasoning}</dt><dd>{declarationLabel(plan.reasoning, text)} · {text.unchanged}</dd></div>
        <div><dt>{text.provider}</dt><dd>{text.unchanged}</dd></div>
        <div><dt>{isClaude ? text.targetAuth : text.auth}</dt><dd>{authLabel(plan.auth, text)} · {text.unchanged}</dd></div>
        <div><dt>{text.agentCount}</dt><dd>{text.unchanged}</dd></div>
        <div><dt>{text.hookShell}</dt><dd>{plan.hook_shell === "absent" ? text.hookAbsent : plan.hook_shell === "present_unchanged" ? text.hookPresent : text.unknown}</dd></div>
        <div><dt>{text.runtimeEffective}</dt><dd>{text.unknown}</dd></div>
      </dl>
      {plan.config_presence === "missing" && <p className="connection-dialog__detail-note">{text.newFile}</p>}
    </section>
  </>;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    mounted.current = true;
    restoreFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!dialog.open) dialog.showModal();
    (installations.length > 1 ? selectRef.current : selected ? previewRef.current : closeRef.current)?.focus();
    if (isClaude) void refreshClaudeState();
    else void readOwnership();
    const keepTabInside = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(
        'button:not([disabled]), select:not([disabled]), input:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
      )).filter((element) => {
        if (element.getAttribute("aria-hidden") === "true" || element.tabIndex < 0) return false;
        if (window.getComputedStyle(element).visibility === "hidden") return false;
        const rect = element.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0;
      });
      const first = focusable[0];
      const last = focusable.at(-1);
      const active = document.activeElement;
      if (!first || !last) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      if (!dialog.contains(active)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
        return;
      }
      if (event.shiftKey && (active === first || active === dialog)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || active === dialog)) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", keepTabInside, true);
    const timer = window.setInterval(() => setNowMs(Date.now()), 15_000);
    return () => {
      mounted.current = false;
      requestSequence.current += 1;
      ownershipSequence.current += 1;
      claudeSequence.current += 1;
      window.clearInterval(timer);
      document.removeEventListener("keydown", keepTabInside, true);
      if (dialog.open) dialog.close();
      if (restoreFocus.current?.isConnected) restoreFocus.current.focus();
    };
  }, []);

  useEffect(() => {
    requestSequence.current += 1;
    setSelectedId(initialInstallationSelection(installations));
    setPreviewState(null);
    setBusyContext(null);
  }, [scanGeneration, idsKey]);

  async function createPreview() {
    if (bridgeLock.current || mutationLock.current || busy || !selected || selected.client_kind !== clientKind
      || (isClaude && (!claudeContextRef.current || claudeError !== null))) return;
    bridgeLock.current = true;
    setBridgePending(true);
    const sequence = ++requestSequence.current;
    const context = { scanGeneration, installationIdsKey: idsKey, scope: currentScope };
    setBusyContext(context);
    setPreviewState(null);
    let next: ConnectionPlanResult;
    try {
      next = await connectionPlanClient.preview({
        installationId: selected.installation_id,
        scanGeneration,
        clientKind,
        scope: currentScope,
      });
    } catch {
      next = { kind: "request_failed" };
    }
    if (mounted.current && requestSequence.current === sequence) {
      setPreviewState({ context, result: next });
      setBusyContext(null);
      setActionFeedback(null);
    }
    bridgeLock.current = false;
    if (mounted.current) setBridgePending(false);
  }

  async function readOwnershipUnlocked(sequence: number, clearFeedback = true) {
    setOwnershipLoading(true);
    setOwnershipView({ kind: "loading" });
    if (clearFeedback) setActionFeedback(null);
    try {
      const next = await connectionActionClient.snapshot(actionClientKind(clientKind));
      if (!mounted.current || ownershipSequence.current !== sequence) return;
      if (next.kind === "success" && next.reply.status === "snapshot") {
        setOwnershipView({ kind: "ready", ownership: next.reply.ownership });
      } else {
        const message = next.kind === "action_error" ? text.actionErrors[next.error.code]
          : next.kind === "desktop_unavailable" ? text.desktopUnavailable : text.ownershipReadError;
        setOwnershipView({ kind: "error", message });
      }
    } catch {
      if (mounted.current && ownershipSequence.current === sequence) {
        setOwnershipView({ kind: "error", message: text.ownershipReadError });
      }
    } finally {
      if (mounted.current && ownershipSequence.current === sequence) setOwnershipLoading(false);
    }
  }

  async function loadClaudeLayersUnlocked(scope: ClaudeContextScope, sequence: number) {
    const next = await claudeContextClient.settingsSnapshot(scope);
    if (!mounted.current || claudeSequence.current !== sequence) return;
    if (next.kind === "snapshot" && next.snapshot.scope === scope) {
      setClaudeSettings(next.snapshot);
      setClaudeError(null);
    } else {
      setClaudeSettings(null);
      setClaudeError(next.kind === "action_error" ? text.actionErrors[next.error.code]
        : next.kind === "desktop_unavailable" ? text.desktopUnavailable : text.ownershipReadError);
    }
  }

  async function loadClaudeStateUnlocked(sequence: number, preserveFeedback = false) {
    setClaudeLoading(true);
    const contextResult = await claudeContextClient.context();
    if (!mounted.current || claudeSequence.current !== sequence) return;
    let nextContext: ClaudeContextState | null = null;
    if (contextResult.kind === "state") {
      nextContext = contextResult.state;
      const previous = claudeContextRef.current;
      const changed = previous !== null && (previous.scope !== nextContext.scope
        || previous.project_directory !== nextContext.project_directory);
      claudeContextRef.current = nextContext;
      setClaudeContext(nextContext);
      setClaudeError(null);
      if (changed) {
        requestSequence.current += 1;
        setPreviewState(null);
        setBusyContext(null);
        if (!preserveFeedback) setActionFeedback(null);
      }
    } else {
      setClaudeContext(null);
      claudeContextRef.current = null;
      setClaudeSettings(null);
      setClaudeError(contextResult.kind === "context_error" ? text.errors[contextResult.error.code as ConnectionPlanErrorCode] ?? text.ownershipReadError
        : contextResult.kind === "desktop_unavailable" ? text.desktopUnavailable : text.ownershipReadError);
      requestSequence.current += 1;
      setPreviewState(null);
      setBusyContext(null);
    }

    const ownerSequence = ++ownershipSequence.current;
    await readOwnershipUnlocked(ownerSequence, !preserveFeedback);
    if (!mounted.current || claudeSequence.current !== sequence) return;
    if (nextContext) await loadClaudeLayersUnlocked(nextContext.scope, sequence);
    if (mounted.current && claudeSequence.current === sequence) setClaudeLoading(false);
  }

  async function refreshClaudeState(preserveFeedback = false) {
    if (bridgeLock.current || mutationLock.current) return;
    bridgeLock.current = true;
    setBridgePending(true);
    const sequence = ++claudeSequence.current;
    try {
      await loadClaudeStateUnlocked(sequence, preserveFeedback);
    } catch {
      if (mounted.current && claudeSequence.current === sequence) {
        setClaudeSettings(null);
        setClaudeError(text.ownershipReadError);
        setClaudeLoading(false);
      }
    } finally {
      bridgeLock.current = false;
      if (mounted.current) setBridgePending(false);
    }
  }

  async function selectClaudeScope(scope: ClaudeContextScope) {
    if (!isClaude || bridgeLock.current || mutationLock.current || !nativeAvailable) return;
    bridgeLock.current = true;
    scopePickerLock.current = scope === "project_local_config";
    setBridgePending(true);
    setClaudePickerPending(scopePickerLock.current);
    setClaudeLoading(true);
    const sequence = ++claudeSequence.current;
    try {
      const selectedScope = await claudeContextClient.selectScope(scope);
      if (!mounted.current || claudeSequence.current !== sequence) return;
      if (selectedScope.kind === "state" && selectedScope.state.status === "cancelled") {
        setClaudeLoading(false);
        return;
      }
      if (selectedScope.kind !== "state" || selectedScope.state.status !== "selected") {
        setClaudeError(selectedScope.kind === "context_error" ? text.ownershipReadError
          : selectedScope.kind === "desktop_unavailable" ? text.desktopUnavailable : text.ownershipReadError);
        setClaudeLoading(false);
        return;
      }
      const nextContext = selectedScope.state;
      claudeContextRef.current = nextContext;
      setClaudeContext(nextContext);
      setClaudeError(null);
      requestSequence.current += 1;
      setPreviewState(null);
      setBusyContext(null);
      setActionFeedback(null);
      const ownerSequence = ++ownershipSequence.current;
      await readOwnershipUnlocked(ownerSequence, true);
      if (!mounted.current || claudeSequence.current !== sequence) return;
      await loadClaudeLayersUnlocked(nextContext.scope, sequence);
      if (mounted.current && claudeSequence.current === sequence) setClaudeLoading(false);
    } catch {
      if (mounted.current && claudeSequence.current === sequence) {
        setClaudeError(text.ownershipReadError);
        setClaudeLoading(false);
      }
    } finally {
      scopePickerLock.current = false;
      if (mounted.current) {
        setClaudePickerPending(false);
        setBridgePending(false);
      }
      bridgeLock.current = false;
    }
  }

  async function readOwnership() {
    if (isClaude) {
      await refreshClaudeState();
      return;
    }
    if (bridgeLock.current || mutationLock.current || ownershipLoading || !nativeAvailable) {
      if (!nativeAvailable) setOwnershipView({ kind: "error", message: text.desktopUnavailable });
      return;
    }
    bridgeLock.current = true;
    setBridgePending(true);
    const sequence = ++ownershipSequence.current;
    try {
      await readOwnershipUnlocked(sequence);
    } finally {
      bridgeLock.current = false;
      if (mounted.current) setBridgePending(false);
    }
  }

  async function applyPlan() {
    if (bridgeLock.current || mutationLock.current || !canApply || !plan) return;
    bridgeLock.current = true;
    mutationLock.current = true;
    setBridgePending(true);
    pendingAction.current = "apply";
    setMutationPending(true);
    setActionFeedback(null);
    requestSequence.current += 1;
    setPreviewState(null);
    setBusyContext(null);
    const applyingPlan = plan;
    try {
      const next = await connectionActionClient.apply(applyingPlan);
      if (!mounted.current) return;
      if (next.kind === "success" && (next.reply.status === "applied" || next.reply.status === "unchanged")) {
        setOwnershipView({ kind: "ready", ownership: next.reply.ownership });
        setActionFeedback({
          kind: "success",
          message: next.reply.status === "applied" ? text.applySuccess
            : next.reply.ownership?.state === "active" ? text.ownershipActive : text.unchangedSuccess,
        });
        if (isClaude) await loadClaudeStateUnlocked(++claudeSequence.current, true);
      } else {
        const outcome = actionResultMessage(next, text);
        setOwnershipView({ kind: "refresh_required" });
        setActionFeedback({ kind: outcome.uncertain ? "uncertain" : "error", message: outcome.message });
      }
    } catch {
      if (mounted.current) {
        setOwnershipView({ kind: "refresh_required" });
        setActionFeedback({ kind: "uncertain", message: text.actionUncertain });
      }
    } finally {
      mutationLock.current = false;
      bridgeLock.current = false;
      pendingAction.current = null;
      if (mounted.current) {
        setMutationPending(false);
        setBridgePending(false);
      }
    }
  }

  async function undoOwnership() {
    if (bridgeLock.current || mutationLock.current || mutationPending || !activeOwnership || !nativeAvailable
      || (isClaude && (!claudeContextRef.current || activeOwnership.scope !== claudeContextRef.current.scope))) return;
    bridgeLock.current = true;
    mutationLock.current = true;
    setBridgePending(true);
    pendingAction.current = "undo";
    setMutationPending(true);
    setActionFeedback(null);
    requestSequence.current += 1;
    setPreviewState(null);
    setBusyContext(null);
    try {
      const next = await connectionActionClient.undo(actionClientKind(clientKind), activeOwnership);
      if (!mounted.current) return;
      if (next.kind === "success" && next.reply.status === "released") {
        setOwnershipView({ kind: "ready", ownership: next.reply.ownership });
        setActionFeedback({ kind: "success", message: text.undoSuccess });
        if (isClaude) await loadClaudeStateUnlocked(++claudeSequence.current, true);
      } else {
        const outcome = actionResultMessage(next, text);
        setOwnershipView({ kind: "refresh_required" });
        setActionFeedback({ kind: outcome.uncertain ? "uncertain" : "error", message: outcome.message });
      }
    } catch {
      if (mounted.current) {
        setOwnershipView({ kind: "refresh_required" });
        setActionFeedback({ kind: "uncertain", message: text.actionUncertain });
      }
    } finally {
      mutationLock.current = false;
      bridgeLock.current = false;
      pendingAction.current = null;
      if (mounted.current) {
        setMutationPending(false);
        setBridgePending(false);
      }
    }
  }

  function close() {
    if (mutationLock.current || scopePickerLock.current) return;
    requestSequence.current += 1;
    ownershipSequence.current += 1;
    claudeSequence.current += 1;
    onClose();
  }

  return <dialog
    ref={dialogRef}
    className="connection-dialog"
    aria-labelledby="connection-dialog-title"
    tabIndex={-1}
    onCancel={(event) => { event.preventDefault(); if (!mutationLock.current && !scopePickerLock.current) close(); }}
    onClick={(event) => { if (event.target === event.currentTarget && !mutationLock.current && !scopePickerLock.current) close(); }}
  >
    <div className="connection-dialog__frame">
      <header className="connection-dialog__header">
        <div>
          <div className="panel-kicker">{isClaude
            ? currentScope === "project_local_config" ? text.claudeProjectScope : text.claudeUserScope
            : text.scope}</div>
          <h2 id="connection-dialog-title">{text.title} · {clientTitles[clientKind]}</h2>
        </div>
        <button ref={closeRef} className="connection-dialog__close" type="button" disabled={mutationPending || claudePickerPending} onClick={close} aria-label={text.close}>×</button>
      </header>
      <div className="connection-dialog__body">
        {isClaude
          ? <ClaudeScopeSection
            language={language}
            context={claudeContext}
            settings={claudeSettings}
            loading={claudeLoading}
            pickerPending={claudePickerPending}
            disabled={!nativeAvailable || bridgePending || mutationPending}
            error={claudeError}
            onSelectScope={(scope) => void selectClaudeScope(scope)}
          />
          : <p className="connection-dialog__scope-note">{text.scopeNote}</p>}
        <section className="connection-dialog__ownership" aria-labelledby="connection-ownership-title">
          <div className="connection-dialog__ownership-head">
            <h3 id="connection-ownership-title">{text.ownershipTitle}</h3>
            <button className="button" type="button" disabled={!nativeAvailable || ownershipLoading || mutationPending || bridgePending} onClick={() => void readOwnership()}>
              {ownershipLoading ? text.reloadingOwnership : isClaude ? text.refreshClaude : text.reloadOwnership}
            </button>
          </div>
          {ownershipView.kind === "loading" && <p className="connection-dialog__notice" role="status"><span className="applications-spinner" aria-hidden="true" />{text.ownershipLoading}</p>}
          {ownershipView.kind === "error" && <p className="connection-dialog__notice connection-dialog__notice--error" role="alert">{ownershipView.message}</p>}
          {ownershipView.kind === "refresh_required" && <p className="connection-dialog__notice connection-dialog__notice--error" role="alert">{text.ownershipRefreshRequired}</p>}
          {ownershipView.kind === "ready" && !ownership && <p className="connection-dialog__notice" role="status">{text.ownershipNone}</p>}
          {ownershipView.kind === "ready" && ownership?.state === "released" && <p className="connection-dialog__notice" role="status">{text.ownershipReleased}</p>}
          {ownershipView.kind === "ready" && activeOwnership && <>
            <p className="connection-dialog__notice" role="status">{text.ownershipActive}{isClaude && activeOwnership.scope === "project_local_config" ? ` ${language === "tr" ? "Bu kayıt başka bir proje klasörüne ait olabilir; geri almak için ilk seçilen klasörü yeniden seçin." : "This record may belong to another project folder; reselect the original folder before undoing."}` : ""}</p>
            <dl className="connection-dialog__ownership-meta">
              <div><dt>{text.ownershipSource}</dt><dd>{text.sources[activeOwnership.config_source]}</dd></div>
              {isClaude && <div><dt>{text.scope}</dt><dd>{activeOwnership.scope === "project_local_config" ? text.claudeProjectScope : text.claudeUserScope}</dd></div>}
              <div><dt>{text.ownershipPort}</dt><dd>127.0.0.1:{activeOwnership.endpoint_port}</dd></div>
            </dl>
            <button className="button connection-dialog__undo" type="button" disabled={mutationPending || bridgePending || !nativeAvailable || (isClaude && (!claudeContext || activeOwnership.scope !== currentScope))} onClick={() => void undoOwnership()}>
              {mutationPending ? (pendingAction.current === "undo" ? text.undoing : text.applying) : text.undo}
            </button>
          </>}
          <p className="connection-dialog__ownership-note">{text.ownershipNotVerified} {text.ownershipLimits}</p>
          {!plan && <p className="connection-dialog__ownership-note">{text.sessionRestart} {text.routeBoundary}</p>}
          {actionFeedback && <p className={"connection-dialog__action-feedback is-" + actionFeedback.kind} role={actionFeedback.kind === "success" ? "status" : "alert"} aria-live="polite">{mutationPending ? (pendingAction.current === "undo" ? text.undoing : text.applying) : actionFeedback.message}</p>}
          {mutationPending && !actionFeedback && <p className="connection-dialog__action-feedback" role="status" aria-live="polite">{pendingAction.current === "undo" ? text.undoing : text.applying}</p>}
        </section>
        <label className="connection-dialog__field" htmlFor="connection-installation">{text.installation}
            <select
            ref={selectRef}
            id="connection-installation"
            value={selectedId}
            disabled={busy || bridgePending || mutationPending || installations.length <= 1}
            onChange={(event) => {
              if (bridgeLock.current || mutationLock.current) return;
              requestSequence.current += 1;
              setSelectedId(event.currentTarget.value);
              setPreviewState(null);
              setBusyContext(null);
              setActionFeedback(null);
            }}
          >
            {installations.length !== 1 && <option value="">{text.chooseInstallation}</option>}
            {installations.map((item) => <option key={item.installation_id} value={item.installation_id}>
              {item.surface === "desktop" ? text.desktop : text.cli} · {item.version ?? text.versionUnknown} · {text.installationSources[item.source]}
            </option>)}
          </select>
        </label>
        {!previewAvailable && <p className="connection-dialog__notice" role="status">{text.noInstallations}</p>}
        {!busy && !result && previewAvailable && <p className="connection-dialog__notice" role="status">{text.idle}</p>}
        {busy && <p className="connection-dialog__notice" role="status"><span className="applications-spinner" aria-hidden="true" />{text.loading}</p>}
        {result && <div className={`connection-dialog__result ${result.kind === "preview" && result.plan.status === "preview" ? "is-preview" : "is-blocked"}`} role={result.kind === "preview" && result.plan.status === "preview" ? "status" : "alert"}>
          <strong>{planResultMessage(result, text)}</strong>
          {result.kind === "preview" && result.plan.status === "blocked" && <ul className="connection-dialog__blockers">
            {result.plan.blockers.map((blocker) => <li key={blocker}>{text.blockers[blocker]}</li>)}
          </ul>}
          {result.kind === "plan_error" && result.error.retryable && <p>{text.requestFailed}</p>}
        </div>}
        {plan && <>
          {expired && <p className="connection-dialog__expired" role="alert">{text.expired}</p>}
          {isOpenCode && openCodeCoverage && <section className="connection-dialog__coverage" aria-labelledby="connection-dialog-coverage-title">
            <h3 id="connection-dialog-coverage-title">{text.routeCoverageTitle}</h3>
            <dl className="connection-dialog__coverage-grid">
              <div><dt>{text.routeEligible}</dt><dd>{openCodeCoverage.eligible_count}</dd></div>
              <div><dt>{text.routeExcluded}</dt><dd>{openCodeCoverage.excluded_count}</dd></div>
              <div><dt>{text.routeResponses}</dt><dd>{openCodeCoverage.responses_count}</dd></div>
              <div><dt>{text.routeChatCompletions}</dt><dd>{openCodeCoverage.chat_completions_count}</dd></div>
              <div><dt>{text.routeConfiguredDefault}</dt><dd>{text[
                openCodeCoverage.configured_default_route === "covered" ? "routeDefaultCovered"
                  : openCodeCoverage.configured_default_route === "excluded" ? "routeDefaultExcluded" : "routeDefaultUnknown"
              ]}</dd></div>
            </dl>
            {openCodeCoverage.configured_default_route === "excluded" && <p className="connection-dialog__coverage-note connection-dialog__coverage-note--warning">{text.routeDefaultExcludedNote}</p>}
            {openCodeCoverage.partial && <p className="connection-dialog__coverage-note">{text.routePartialNote}</p>}
            <p className="connection-dialog__coverage-note">{text.routeRuntimeUnknown}</p>
          </section>}
          {isOpenCode
            ? <details className="connection-dialog__advanced">
              <summary>{text.advancedDetails}</summary>
              <div className="connection-dialog__advanced-body">{planTechnicalDetails}</div>
            </details>
            : planTechnicalDetails}
          <section className="connection-dialog__section connection-dialog__limits">
            <h3>{text.sessionEffects}</h3>
            <p>{text.sessionRestart}</p>
            <p>{text.routeBoundary}</p>
          </section>
          <p className="connection-dialog__expiry">{text.expires}: <time dateTime={new Date(plan.expires_at_unix_seconds * 1_000).toISOString()}>{new Date(plan.expires_at_unix_seconds * 1_000).toLocaleTimeString(language === "tr" ? "tr-TR" : "en-US", { hour: "2-digit", minute: "2-digit" })}</time></p>
        </>}
      </div>
      <footer className="connection-dialog__footer">
        <div className="connection-dialog__apply-note"><span>{text.applyUnavailable}</span></div>
        <div className="connection-dialog__actions">
          <button className="button" type="button" disabled={mutationPending || claudePickerPending} onClick={close}>{text.close}</button>
          <button ref={previewRef} className="button" type="button" disabled={busy || bridgePending || mutationPending || !selected || !previewAvailable || !nativeAvailable || (isClaude && (!claudeContext || claudeError !== null))} onClick={() => void createPreview()}>
            {busy ? text.refreshing : plan ? text.refresh : text.preview}
          </button>
          <button className="button button-primary" type="button" disabled={!canApply} onClick={() => void applyPlan()}>{mutationPending && pendingAction.current === "apply" ? text.applying : text.apply}</button>
        </div>
      </footer>
    </div>
  </dialog>;
}
