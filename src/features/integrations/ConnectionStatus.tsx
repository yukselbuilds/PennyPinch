import { useCallback, useEffect, useRef, useState } from "react";
import {
  codexStatusClient,
  codexVerifyClient,
  codexVerifyGenerationChanged,
  CodexStatusRequestSequence,
  diagnoseCodexNextAction,
  isCodexStatusAvailable,
  isCodexVerifySelectionSupported,
  presentCodexSetting,
  visibleRuntimeStatus,
  type CodexStatusErrorCode,
  type CodexStatusResult,
  type CodexStatusSnapshot,
  type CodexVerifyErrorCode,
  type CodexVerifyFailureCode,
  type CodexVerifyResult,
  type CodexSettingPresentationKey,
  type CodexNextActionCode,
} from "../../lib/codexStatus";
import type { InstallationSummary } from "../../lib/integrations";
import type { PreferenceLanguage } from "../../lib/preferences";

type Copy = {
  title: string;
  choose: string;
  chooseInstallation: string;
  update: string;
  updating: string;
  verify: string;
  verifying: string;
  verifyScope: string;
  verifyUsage: string;
  verifyNeedsReady: string;
  verifyUnsupported: string;
  verifyPassed: string;
  verifyCompletedWithErrors: string;
  verifyBlocked: string;
  verifyFailed: string;
  verifyRouteUnsupported: string;
  verifyRuntimeUnavailable: string;
  verifyConfigUnavailable: string;
  verifyStale: string;
  verifyProcessFailed: string;
  verifyError: Record<CodexVerifyErrorCode, string>;
  desktopRequired: string;
  scanFirst: string;
  loading: string;
  noSelection: string;
  readFailure: string;
  invalidResponse: string;
  currentInstall: string;
  compatibility: string;
  ownership: string;
  endpointBinding: string;
  ingress: string;
  engine: string;
  traffic: string;
  liveCoverage: string;
  more: string;
  currentValue: string;
  diskSetting: string;
  diskState: Record<CodexSettingPresentationKey, string>;
  diskObservationLimit: string;
  nextStep: string;
  nextAction: Record<CodexNextActionCode, string>;
  configScope: string;
  userConfig: string;
  policy: string;
  auth: string;
  agents: string;
  agentsValue: string;
  currentMethodNotRead: string;
  active: string;
  released: string;
  none: string;
  unavailable: string;
  same: string;
  different: string;
  notOwned: string;
  unknown: string;
  ready: string;
  stopped: string;
  starting: string;
  draining: string;
  failed: string;
  notVerified: string;
  fixtureOnly: string;
  notTested: string;
  updateRequired: string;
  apiKeyFixture: string;
  apiKeyUntested: string;
  chatgptUntested: string;
  error: Record<CodexStatusErrorCode, string>;
};

const copy: Record<PreferenceLanguage, Copy> = {
  tr: {
    title: "Codex bağlantı durumu", choose: "Kurulum seç",
    chooseInstallation: "Bir Codex kurulumu seç", update: "Durumu güncelle", updating: "Güncelleniyor…",
    verify: "Yeni süreçte doğrula", verifying: "Test isteği sürüyor…",
    verifyScope: "Bu kontrol yalnızca yeni ve salt okunur bir Codex süreci için geçerlidir. Kayıtlı ayarların kullanıldığını veya ana/alt ajan kapsamını doğrulamaz.",
    verifyUsage: "Geçerli Codex oturum açma yöntemini ve normal model seçimini kullanarak gerçek bir test isteği gönderir; hesap kullanımına yansıyabilir. Yalnız bu yeni sürecin endpoint'i değiştirilir; kayıtlı ayarlar değişmez.",
    verifyNeedsReady: "Doğrulama için desteklenen CLI kurulumu ve hazır Headroom motoru gerekir.",
    verifyUnsupported: "Bu Codex yüzeyi veya sürümü bu yerel testte desteklenmiyor.",
    verifyPassed: "Yeni Codex süreci test isteğini Headroom üzerinden tamamladı. Kayıtlı ayarların benimsenmesi ve ajan kapsamı doğrulanmadı.",
    verifyCompletedWithErrors: "Bu yanıt Headroom üzerinden tamamlandı; {count} ek, sınıflandırılmamış CLI hatası bildirildi. Yalnız bu yanıtın rotası doğrulandı; genel protokol ve araç durumu bilinmiyor.",
    verifyBlocked: "Yerel test başlamadı. Durumu güncelle ve gösterilen yerel gereksinimleri kontrol et.",
    verifyFailed: "Yerel test doğrulanamadı. Durumu güncelleyip motoru kontrol edebilirsin.",
    verifyRouteUnsupported: "Mevcut Codex rota ayarları bu test kapsamının dışında; hiçbir ayar değiştirilmedi.",
    verifyRuntimeUnavailable: "Headroom motoru hazır değil. Genel Bakış'ta motoru başlatıp durumu güncelle.",
    verifyConfigUnavailable: "Codex rota ayarı güvenle okunamadı; dosya değiştirilmedi.",
    verifyStale: "Kurulum veya motor değişti. Durumu güncellemeden sonucu kullanma.",
    verifyProcessFailed: "Yeni Codex süreci tamamlanmadı. Tekrar denemeden önce durumu güncelle.",
    verifyError: {
      validation_failed: "Doğrulama isteği geçersiz.", stale_selection: "Kurulum listesi değişti; yeniden tara.",
      not_found: "Seçilen kurulum artık listede değil.", busy: "Başka bir yerel işlem sürüyor; durum güncellendikten sonra yeniden dene.",
      unavailable: "Yerel doğrulama şu anda kullanılamıyor.", unsupported_platform: "Bu işletim sisteminde doğrulama desteklenmiyor.",
    },
    desktopRequired: "Bu yerel durum masaüstü uygulamasında kullanılabilir.",
    scanFirst: "Önce yerel kurulumları tara.", loading: "Seçilen kurulumun yerel durumu okunuyor…",
    noSelection: "Durum görmek için bir Codex kurulumu seç.",
    readFailure: "Yerel durum okunamadı. Yeniden deneyebilirsin.",
    invalidResponse: "Yerel durum yanıtı doğrulanamadı.", currentInstall: "Kurulum",
    compatibility: "Sürüm kapsamı", ownership: "Headroom kaydı", endpointBinding: "Endpoint eşleşmesi",
    ingress: "Yerel giriş", engine: "Motor", traffic: "İstemci trafiği",
    liveCoverage: "Canlı auth ve ajan kapsamı", more: "Diğer ayrıntılar",
    currentValue: "Codex ayarındaki değer", diskSetting: "Diskteki Codex ayarı",
    diskState: {
      matches: "Kayıtlı adresle eşleşiyor", missing: "Codex ayarında Headroom adresi yok",
      different: "Codex ayarında farklı bir adres var",
      unsupported_route: "Codex ayar yapısı mevcut yerel rota kapsamının dışında. Sağlayıcı, model veya profil değiştirilmedi.",
      unavailable: "Codex ayarı güvenle okunamadı",
      not_owned: "Headroom'a ait bir ayar kaydı yok", not_rechecked: "Disk ayarı yeniden kontrol edilmedi",
    },
    diskObservationLimit: "Bu, dosyanın anlık gözlemidir; çalışan Codex sürecinin bu ayarı kullandığını doğrulamaz.",
    nextStep: "Sonraki adım",
    nextAction: {
      unsupported_installation: "Bu Codex yüzeyi veya sürümü yerel test kapsamına alınmadı; canlı doğrulama yapılamıyor.",
      refresh_status: "Yerel durumu yeniden oku; kayıt şu anda güvenle okunamadı.",
      manage_connection: "Bağlantıyı yönet üzerinden önizlemeyi açıp ayarları gözden geçir.",
      inspect_codex_setting: "Bağlantıyı yönet üzerinden Codex ayarını ve kaydı incele; otomatik düzeltme yapılmadı.",
      review_connection: "Bağlantıyı yönet üzerinden mevcut eşleşmeyi incele; ayarı otomatik değiştirme.",
      inspect_ingress: "Durumu güncelle; sorun sürerse Genel Bakış'taki motor durumunu incele.",
      start_engine: "Genel Bakış'tan Headroom motorunu başlat, sonra durumu yeniden oku.",
      wait_for_engine: "Motor işlemi tamamlanana kadar bekle, ardından durumu yeniden oku.",
      inspect_runtime: "Genel Bakış'taki motor durumunu incele, sonra bu panelde durumu güncelle.",
      local_prerequisites_ready: "Yerel önkoşullar hazır. Bu durum okuması istemci trafiğini doğrulamaz.",
    },
    policy: "Ayar kapsamı", auth: "Kimlik doğrulama kapsamı",
    configScope: "Kapsam", userConfig: "Kullanıcı ayarları",
    agents: "Ajan kapsamı", agentsValue: "Ana ajan: bilinmiyor; alt ajan: sınanmadı.",
    currentMethodNotRead: "Mevcut yöntem okunmadı.", active: "Etkin kayıt", released: "Kayıt geri alınmış", none: "Kayıt yok",
    unavailable: "Kayıt okunamadı", same: "Aynı endpoint", different: "Farklı endpoint",
    notOwned: "Headroom sahipliğinde değil", unknown: "Bilinmiyor", ready: "Hazır", stopped: "Durdurulmuş",
    starting: "Başlıyor", draining: "Kapanıyor", failed: "Hata", notVerified: "Doğrulanmadı",
    fixtureOnly: "Yalnız yerel test", notTested: "Sınanmadı",
    updateRequired: "Headroom model, akıl yürütme ve kimlik doğrulama ayarlarını değiştirmedi. Bir ayar değişikliğini kullanmak için yeni Codex süreci gerekir; mevcut etkin ayar bilinmiyor.",
    apiKeyFixture: "API anahtarı rotası yalnız yerel testte sınandı.",
    apiKeyUntested: "API anahtarı rotası sınanmadı.", chatgptUntested: "ChatGPT girişi sınanmadı.",
    error: {
      validation_failed: "Durum isteği doğrulanamadı.", stale_selection: "Kurulum listesi değişti; yeniden tara ve seçim yap.",
      not_found: "Seçilen kurulum artık listede değil.", client_mismatch: "Seçim Codex kurulumu değil.",
      unavailable: "Yerel durum geçici olarak kullanılamıyor.",
    },
  },
  en: {
    title: "Codex connection status", choose: "Choose installation",
    chooseInstallation: "Choose a Codex installation", update: "Update status", updating: "Updating…",
    verify: "Test in a new process", verifying: "Test request running…",
    verifyScope: "This check applies only to a new read-only Codex process. It does not verify saved-setting adoption or main/subagent coverage.",
    verifyUsage: "Sends a real test request using your current Codex sign-in and normal model selection; it may count toward account usage. Only this new process gets an endpoint override; saved settings are not changed.",
    verifyNeedsReady: "Verification needs a supported CLI installation and a ready Headroom engine.",
    verifyUnsupported: "This Codex surface or version is not supported by this local test.",
    verifyPassed: "The new Codex process completed the test request through Headroom. Saved-setting adoption and agent coverage remain unverified.",
    verifyCompletedWithErrors: "This response completed through Headroom; {count} additional, unclassified CLI errors were reported. Only this response route was verified; general protocol and tool status remain unknown.",
    verifyBlocked: "The local test did not start. Update status and check the local requirements shown here.",
    verifyFailed: "The local test could not be verified. You can update status and inspect the engine.",
    verifyRouteUnsupported: "The current Codex route settings are outside this test's scope; no settings were changed.",
    verifyRuntimeUnavailable: "The Headroom engine is not ready. Start it from Overview, then update status.",
    verifyConfigUnavailable: "Codex route settings could not be read safely; no file was changed.",
    verifyStale: "The installation or engine changed. Refresh status before relying on this result.",
    verifyProcessFailed: "The new Codex process did not complete. Update status before trying again.",
    verifyError: {
      validation_failed: "The verification request is invalid.", stale_selection: "The installation list changed; scan again.",
      not_found: "The selected installation is no longer listed.", busy: "Another local operation is running; refresh status before trying again.",
      unavailable: "Local verification is temporarily unavailable.", unsupported_platform: "Verification is not supported on this operating system.",
    },
    desktopRequired: "This local status is available in the desktop app.",
    scanFirst: "Scan for local installations first.", loading: "Reading local status for the selected installation…",
    noSelection: "Choose a Codex installation to view its status.",
    readFailure: "Local status could not be read. You can try again.",
    invalidResponse: "The local status response could not be validated.", currentInstall: "Installation",
    compatibility: "Version scope", ownership: "Headroom record", endpointBinding: "Endpoint binding",
    ingress: "Local ingress", engine: "Engine", traffic: "Client traffic",
    liveCoverage: "Live auth and agent coverage", more: "More details",
    currentValue: "Value in Codex settings", diskSetting: "Codex setting on disk",
    diskState: {
      matches: "Matches saved address", missing: "No Headroom address in Codex settings",
      different: "Codex settings contain a different address",
      unsupported_route: "The Codex settings structure is outside current local route coverage. No provider, model, or profile was changed.",
      unavailable: "Codex settings could not be read safely",
      not_owned: "No setting record is owned by Headroom", not_rechecked: "Disk setting was not rechecked",
    },
    diskObservationLimit: "This is a point-in-time file observation; it does not verify that the running Codex process uses this setting.",
    nextStep: "Next step",
    nextAction: {
      unsupported_installation: "This Codex surface or version is outside local test coverage; live verification is unavailable.",
      refresh_status: "Refresh local status; the record could not be read safely.",
      manage_connection: "Open Manage connection to review the preview and settings.",
      inspect_codex_setting: "Use Manage connection to review the Codex setting and record; no automatic repair was made.",
      review_connection: "Use Manage connection to review the current match; the setting was not changed automatically.",
      inspect_ingress: "Update status; if the issue remains, inspect the engine state on Overview.",
      start_engine: "Start the Headroom engine from Overview, then refresh status.",
      wait_for_engine: "Wait for the engine operation to finish, then refresh status.",
      inspect_runtime: "Inspect the engine state on Overview, then update status in this panel.",
      local_prerequisites_ready: "Local prerequisites are ready. This status read does not verify client traffic.",
    },
    configScope: "Scope", userConfig: "User config",
    policy: "Settings scope", auth: "Authentication scope", agents: "Agent coverage",
    agentsValue: "Main agent: unknown; subagent: not tested.", currentMethodNotRead: "Current method was not read.",
    active: "Active record", released: "Record released", none: "No record",
    unavailable: "Record unavailable", same: "Same endpoint", different: "Different endpoint",
    notOwned: "Not owned by Headroom", unknown: "Unknown", ready: "Ready", stopped: "Stopped",
    starting: "Starting", draining: "Stopping", failed: "Failed", notVerified: "Not verified",
    fixtureOnly: "Local test only", notTested: "Not tested",
    updateRequired: "Headroom did not change model, reasoning, or authentication settings. A new Codex process is required to use a setting change; current effective settings are unknown.",
    apiKeyFixture: "The API-key route was tested only with a local test.",
    apiKeyUntested: "The API-key route was not tested.", chatgptUntested: "ChatGPT sign-in was not tested.",
    error: {
      validation_failed: "The status request could not be validated.", stale_selection: "The installation list changed; scan and select again.",
      not_found: "The selected installation is no longer listed.", client_mismatch: "The selection is not a Codex installation.",
      unavailable: "Local status is temporarily unavailable.",
    },
  },
};

type LocalState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "snapshot"; value: CodexStatusSnapshot }
  | { kind: "failed"; result: CodexStatusResult };

type VerificationState = { kind: "idle" } | { kind: "running" } | { kind: "stale" } | { kind: "result"; result: CodexVerifyResult };

export function ConnectionStatus({
  language,
  installations,
  scanGeneration,
}: {
  language: PreferenceLanguage;
  installations: InstallationSummary[];
  scanGeneration: number;
}) {
  const text = copy[language];
  const [selectedId, setSelectedId] = useState(() => installations.length === 1 ? installations[0].installation_id : "");
  const [state, setState] = useState<LocalState>({ kind: "idle" });
  const [verification, setVerification] = useState<VerificationState>({ kind: "idle" });
  const sequence = useRef(new CodexStatusRequestSequence());
  const verifySequence = useRef(new CodexStatusRequestSequence());
  const verifyInFlight = useRef(false);
  const mounted = useRef(false);
  const selected = installations.find((item) => item.installation_id === selectedId);
  const available = isCodexStatusAvailable();
  const runtimeGeneration = state.kind === "snapshot" ? state.value.runtime.runtime_generation : null;
  const verifyContext = `${selectedId}:${scanGeneration}:${runtimeGeneration ?? "unavailable"}`;
  const currentVerifyContext = useRef(verifyContext);
  currentVerifyContext.current = verifyContext;

  const load = useCallback(async (installation: InstallationSummary) => {
    verifySequence.current.invalidate();
    setVerification({ kind: "idle" });
    const token = sequence.current.next();
    setState({ kind: "loading" });
    const result = await codexStatusClient.load(installation, scanGeneration);
    if (!mounted.current || !sequence.current.isCurrent(token)) return;
    setState(result.kind === "snapshot" ? { kind: "snapshot", value: result.snapshot } : { kind: "failed", result });
  }, [scanGeneration]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      sequence.current.invalidate();
      verifySequence.current.invalidate();
    };
  }, []);

  useEffect(() => {
    if (selected) void load(selected);
    else {
      sequence.current.invalidate();
      verifySequence.current.invalidate();
      setVerification({ kind: "idle" });
      setState({ kind: "idle" });
    }
  }, [selectedId, scanGeneration, load]);

  function chooseInstallation(installationId: string) {
    sequence.current.invalidate();
    verifySequence.current.invalidate();
    setVerification({ kind: "idle" });
    setState({ kind: "idle" });
    setSelectedId(installationId);
  }

  async function verifySelectedInstallation() {
    if (!selected || selected.client_kind !== "codex" || state.kind !== "snapshot"
      || !isCodexVerifySelectionSupported({ ...selected, client_kind: "codex" })
      || !state.value.runtime.engine_ready || !state.value.runtime.stable_ingress_ready
      || verifyInFlight.current) return;
    verifyInFlight.current = true;
    const token = verifySequence.current.next();
    const context = verifyContext;
    const runtimeGenerationAtStart = state.value.runtime.runtime_generation;
    setVerification({ kind: "running" });
    try {
      const result = await codexVerifyClient.verify({
        installation_id: selected.installation_id,
        client_kind: "codex",
        surface: selected.surface,
        version: selected.version,
      }, scanGeneration);
      if (!mounted.current || !verifySequence.current.isCurrent(token) || currentVerifyContext.current !== context) return;
      if (result.kind === "snapshot"
        && codexVerifyGenerationChanged(result.snapshot.runtime_generation, runtimeGenerationAtStart)) {
        setVerification({ kind: "stale" });
        return;
      }
      setVerification({ kind: "result", result });
    } finally {
      verifyInFlight.current = false;
    }
  }

  const busy = state.kind === "loading";
  const verificationBusy = verification.kind === "running";
  const supportedSelection = Boolean(selected && selected.client_kind === "codex"
    && isCodexVerifySelectionSupported({ ...selected, client_kind: "codex" }));
  const runtimeReady = state.kind === "snapshot" && state.value.runtime.engine_ready && state.value.runtime.stable_ingress_ready;
  const canVerify = available && supportedSelection && runtimeReady && verification.kind !== "stale";
  const failure = state.kind === "failed" ? failureText(state.result, text) : null;
  return <section className="connection-status-panel" aria-label={text.title}>
    <div className="connection-status-panel__header">
      <h2>{text.title}</h2>
      <div className="connection-status-panel__actions">
        <button className="button" type="button" disabled={!available || !selected || busy || verificationBusy}
          onClick={() => selected && void load(selected)}>{busy ? text.updating : text.update}</button>
        {selected && <button className="button button-primary" type="button" disabled={!canVerify || busy || verificationBusy}
          onClick={() => void verifySelectedInstallation()}>{verificationBusy ? text.verifying : text.verify}</button>}
      </div>
    </div>
    {installations.length > 1 && <label className="connection-dialog__field">
      {text.choose}
      <select disabled={busy || verificationBusy} value={selectedId} onChange={(event) => chooseInstallation(event.currentTarget.value)}>
        <option value="">{text.chooseInstallation}</option>
        {installations.map((installation) => <option key={installation.installation_id} value={installation.installation_id}>
          {installation.surface === "desktop" ? "Desktop" : "CLI"} · {installation.version ?? text.unknown}
        </option>)}
      </select>
    </label>}
    {!available && <p className="applications-notice" role="status">{text.desktopRequired}</p>}
    {available && installations.length === 0 && <p className="applications-notice" role="status">{text.scanFirst}</p>}
    {available && installations.length > 1 && !selected && <p className="applications-notice" role="status">{text.noSelection}</p>}
    {available && selected && busy && <p className="applications-notice" role="status" aria-live="polite">
      <span className="applications-spinner" aria-hidden="true" />{text.loading}
    </p>}
    {available && failure && <p className="applications-notice applications-notice--error" role="alert">{failure}</p>}
    {available && state.kind === "snapshot" && <StatusDetails snapshot={state.value} text={text} />}
    {selected && <>
      <p className="applications-scope" role="note">{supportedSelection ? text.verifyScope : text.verifyUnsupported}</p>
      {supportedSelection && <p className="applications-scope" role="note">{text.verifyUsage}</p>}
      {supportedSelection && state.kind === "snapshot" && !runtimeReady && <p className="applications-notice" role="status">{text.verifyNeedsReady}</p>}
      {verificationBusy && <p className="applications-notice" role="status" aria-live="polite">
        <span className="applications-spinner" aria-hidden="true" />{text.verifying}
      </p>}
      {verification.kind === "stale" && <p className="applications-notice applications-notice--error" role="status">{text.verifyStale}</p>}
      {verification.kind === "result" && <p className={`applications-notice ${verification.result.kind === "snapshot"
        ? verification.result.snapshot.probe_status === "response_verified_cli_errors"
          ? "applications-notice--warning"
          : verification.result.snapshot.probe_status === "passed" ? "" : "applications-notice--error"
        : ""}`} role="status" aria-live="polite">
        {verificationMessage(verification.result, text)}
      </p>}
    </>}
  </section>;
}
function StatusDetails({ snapshot, text }: { snapshot: CodexStatusSnapshot; text: Copy }) {
  const installation = snapshot.installation;
  const ownership = snapshot.ownership;
  const runtime = snapshot.runtime;
  const nextAction = diagnoseCodexNextAction(snapshot);
  const compatibility = installation.compatibility === "fixture_only" ? text.fixtureOnly :
    installation.compatibility === "not_tested" ? text.notTested : text.notVerified;
  const diskSetting = presentCodexSetting(ownership.current_config_value);
  const ownerState = ownership.state === "active" ? text.active : ownership.state === "released" ? text.released :
    ownership.state === "none" ? text.none : text.unavailable;
  const binding = ownership.endpoint_binding === "same" ? text.same : ownership.endpoint_binding === "different" ? text.different :
    ownership.endpoint_binding === "not_owned" ? text.notOwned : text.unknown;
  const ingress = statusLabel(visibleRuntimeStatus(
    runtime.ingress_status,
    runtime.stable_ingress_ready,
  ), text);
  const engine = statusLabel(visibleRuntimeStatus(
    runtime.engine_status,
    runtime.engine_ready,
  ), text);
  const summaryRows: [string, string][] = [
    [text.ownership, ownerState],
    [text.ingress, ingress],
    [text.engine, engine],
    [text.traffic, text.notVerified],
  ];
  const apiKey = snapshot.auth_scope.api_key_auth === "fixture_only" ? text.apiKeyFixture : text.apiKeyUntested;
  const detailRows: [string, string][] = [
    [text.currentInstall, `${installation.surface === "desktop" ? "Desktop" : "CLI"} · ${installation.version ?? text.unknown}`],
    [text.compatibility, compatibility],
    [text.endpointBinding, binding],
    [text.ingress, ingress],
    [text.configScope, text.userConfig],
    [text.currentValue, text.diskState[diskSetting.key]],
    [text.auth, `${text.currentMethodNotRead} · ${text.chatgptUntested} · ${apiKey}`],
  ];
  return <>
    <dl className="connection-status-panel__facts">
      {summaryRows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}
    </dl>
    <p className={`connection-dialog__detail-note connection-status__setting connection-status__setting--${diskSetting.tone}`} role="status">
      <strong>{text.diskSetting}:</strong> {text.diskState[diskSetting.key]}
    </p>
    {ownership.state === "active" && <p className="connection-dialog__detail-note">{text.diskObservationLimit}</p>}
    <p className="connection-dialog__detail-note">
      {text.liveCoverage}: {text.chatgptUntested} · {apiKey} · {text.agentsValue}
    </p>
    <p className="connection-dialog__detail-note" role="status">
      <strong>{text.nextStep}:</strong> {text.nextAction[nextAction.code]}
    </p>
    <details className="connection-status__details">
      <summary>{text.more}</summary>
      <dl className="connection-dialog__details">
        {detailRows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}
      </dl>
      <p className="connection-dialog__detail-note">{text.updateRequired}</p>
    </details>
  </>;
}

function statusLabel(status: string, text: Copy): string {
  switch (status) {
    case "ready": return text.ready;
    case "stopped": return text.stopped;
    case "starting": return text.starting;
    case "draining": return text.draining;
    case "failed": return text.failed;
    case "unknown": return text.unknown;
    default: return text.unavailable;
  }
}

function failureText(result: CodexStatusResult, text: Copy): string {
  if (result.kind === "codex_error") return text.error[result.error.error.code];
  if (result.kind === "desktop_unavailable") return text.desktopRequired;
  if (result.kind === "invalid_response") return text.invalidResponse;
  return text.readFailure;
}

function verificationMessage(result: CodexVerifyResult, text: Copy): string {
  if (result.kind === "verify_error") return text.verifyError[result.error.error.code];
  if (result.kind === "desktop_unavailable") return text.desktopRequired;
  if (result.kind === "unsupported_installation") return text.verifyUnsupported;
  if (result.kind !== "snapshot") return text.verifyFailed;
  const snapshot = result.snapshot;
  if (snapshot.probe_status === "passed") return text.verifyPassed;
  if (snapshot.probe_status === "response_verified_cli_errors") {
    return text.verifyCompletedWithErrors.replace("{count}", String(snapshot.cli_error_count));
  }
  let message: string;
  switch (snapshot.failure_code as CodexVerifyFailureCode | null) {
    case "unsupported_installation": message = text.verifyUnsupported; break;
    case "runtime_unavailable": message = text.verifyRuntimeUnavailable; break;
    case "route_unsupported": message = text.verifyRouteUnsupported; break;
    case "config_unavailable":
    case "config_changed": message = text.verifyConfigUnavailable; break;
    case "selection_changed":
    case "runtime_changed":
    case "state_changed": message = text.verifyStale; break;
    case "process_start_failed":
    case "process_cancelled":
    case "process_timeout":
    case "process_wait_failed":
    case "process_failed": message = text.verifyProcessFailed; break;
    case "process_ownership_unavailable":
    case "process_ownership_lost":
    case "process_output_invalid":
    case "transport_unverified":
    case "workspace_unavailable": message = text.verifyFailed; break;
    default: message = snapshot.probe_status === "blocked" ? text.verifyBlocked : text.verifyFailed;
  }
  const safeReasons = [snapshot.failure_code, snapshot.output_diagnostic]
    .flatMap((value) => value === null ? [] : [value]);
  return safeReasons.length > 0 ? `${message} (${safeReasons.join(" · ")})` : message;
}
