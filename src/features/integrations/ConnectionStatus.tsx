import { useCallback, useEffect, useRef, useState } from "react";
import {
  codexStatusClient,
  CodexStatusRequestSequence,
  isCodexStatusAvailable,
  presentCodexSetting,
  visibleRuntimeStatus,
  type CodexStatusErrorCode,
  type CodexStatusResult,
  type CodexStatusSnapshot,
  type CodexSettingPresentationKey,
} from "../../lib/codexStatus";
import type { InstallationSummary } from "../../lib/integrations";
import type { PreferenceLanguage } from "../../lib/preferences";

type Copy = {
  title: string;
  choose: string;
  chooseInstallation: string;
  update: string;
  updating: string;
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
      different: "Codex ayarında farklı bir adres var", unavailable: "Codex ayarı güvenle okunamadı",
      not_owned: "Headroom'a ait bir ayar kaydı yok", not_rechecked: "Disk ayarı yeniden kontrol edilmedi",
    },
    diskObservationLimit: "Bu, dosyanın anlık gözlemidir; çalışan Codex sürecinin bu ayarı kullandığını doğrulamaz.",
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
      different: "Codex settings contain a different address", unavailable: "Codex settings could not be read safely",
      not_owned: "No setting record is owned by Headroom", not_rechecked: "Disk setting was not rechecked",
    },
    diskObservationLimit: "This is a point-in-time file observation; it does not verify that the running Codex process uses this setting.",
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
  const sequence = useRef(new CodexStatusRequestSequence());
  const mounted = useRef(false);
  const selected = installations.find((item) => item.installation_id === selectedId);
  const available = isCodexStatusAvailable();

  const load = useCallback(async (installation: InstallationSummary) => {
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
    };
  }, []);

  useEffect(() => {
    if (selected) void load(selected);
    else {
      sequence.current.invalidate();
      setState({ kind: "idle" });
    }
  }, [selectedId, scanGeneration, load]);

  function chooseInstallation(installationId: string) {
    sequence.current.invalidate();
    setState({ kind: "idle" });
    setSelectedId(installationId);
  }

  const busy = state.kind === "loading";
  const failure = state.kind === "failed" ? failureText(state.result, text) : null;
  return <section className="connection-status-panel" aria-label={text.title}>
    <div className="connection-status-panel__header">
      <h2>{text.title}</h2>
      <button className="button" type="button" disabled={!available || !selected || busy}
        onClick={() => selected && void load(selected)}>{busy ? text.updating : text.update}</button>
    </div>
    {installations.length > 1 && <label className="connection-dialog__field">
      {text.choose}
      <select value={selectedId} onChange={(event) => chooseInstallation(event.currentTarget.value)}>
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
  </section>;
}

function StatusDetails({ snapshot, text }: { snapshot: CodexStatusSnapshot; text: Copy }) {
  const installation = snapshot.installation;
  const ownership = snapshot.ownership;
  const runtime = snapshot.runtime;
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
