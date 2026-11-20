import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PreferenceLanguage } from "../lib/preferences";
import { recoveryClient, type RecoveryClientSnapshot, type RecoveryResult, type RecoverySnapshot } from "../lib/recovery";
import "./problem-notice.css";

type Props = {
  language: PreferenceLanguage;
  contextKey: string;
  disabled: boolean;
  onOpenApplications: () => void;
  onOpenOverview: () => void;
};

const copy = {
  tr: {
    title: "Bağlantıyı denetle",
    intro: "Motoru, kayıtlı ayarları ve son yedi gündeki yerel trafik işaretini salt okunur olarak kontrol eder.",
    scope: "Sağlayıcı, hesap veya oturum başarısını doğrulamaz.",
    check: "Denetle",
    checking: "Denetleniyor…",
    disabled: "Devam eden işlem bitince denetleyebilirsiniz.",
    none: "Henüz denetim yapılmadı.",
    failed: "Denetim tamamlanamadı. Yeniden deneyebilirsiniz.",
    nativeOnly: "Bu denetim PennyPinch masaüstü uygulamasında kullanılabilir.",
    requestBusy: "Başka bir denetim sürüyor. Bitince yeniden deneyin.",
    stale: "Bu sonuç eski bağlamdan geldi. Yeniden tarayın ve sürümü doğrulayın; ayar değiştiyse istemciyi yeniden açın.",
    runtime: "Motor",
    ingress: "Yerel giriş",
    clients: "Uygulama ayarları",
    traffic: "Son 7 günün yerel trafik işareti",
    observed: "Kayıt bulundu",
    noTraffic: "Kayıt yok; bu, kaçak veya sıfır tasarruf kanıtı değildir.",
    trafficUnavailable: "Kayıt okunamadı; trafik sonucu bilinmiyor.",
    noError: "Hata bildirilmedi; bu, başarılı istek kanıtı değildir.",
    errorReported: "Kaydedilen istekte hata bildirildi; kaynak bilinmiyor. İstemcide 401/403 veya giriş hatası varsa orada oturum açın.",
    outcomeUnknown: "Hata sonucu bilinmiyor.",
    configSupported: "ayar biçimi tanınıyor",
    configNotVerified: "ayar biçimi doğrulanmadı",
    current: "güncel dosya",
    staleInstall: "dosya değişmiş",
    notFound: "kurulum bulunamadı",
    installationUnknown: "kurulum bilinmiyor",
    matched: "PennyPinch adresi kayıtlı",
    conflict: "ayar çakışması",
    notOwned: "PennyPinch ayar kaydı yok",
    configUnavailable: "ayar okunamadı",
    busy: "ayar denetimi meşgul",
    notRechecked: "ayar yeniden denetlenmedi",
    userScope: "kullanıcı ayarı",
    projectScope: "proje ayarı",
    scopeUnknown: "kapsam bilinmiyor",
    bindingSame: "adres eşleşiyor",
    bindingDifferent: "adres farklı",
    bindingNotOwned: "adres sahipliği yok",
    bindingUnknown: "adres bilinmiyor",
    runtimeEnvironment: "Motor ortamını veya uygulama paketini denetleyin.",
    runtimeOwnership: "Yabancı sürece dokunmadan sahiplik ve port durumunu denetleyin; ardından durumu yenileyin.",
    runtimeConfig: "Uygulamalar ekranında ayar önizlemesini inceleyin.",
    runtimeLifecycle: "Genel Bakış ekranında motor durumunu denetleyin.",
    runtimeRetry: "Motor yanıt vermedi. Durumu yenileyin; sorun sürerse motor durumunu Genel Bakış’ta inceleyin.",
    runtimeProtocol: "Motor yanıtını ve uygulama sürümünü Genel Bakış’ta denetleyin.",
    runtimeUnavailable: "Motor şu anda kullanılamıyor. Genel Bakış’ta motor durumunu denetleyin.",
    staleInstallation: "Kurulum veya sürüm kanıtı güncel değil. Uygulamalar’da yeniden tarayın; yeni sürümün ayar ve protokol yolunu doğrulayın. Ayar değiştiyse istemciyi yeniden açın.",
    configReview: "Kayıtlı ayar veya adres değişmiş olabilir. Uygulamalar’da önizlemeyi gözden geçirin.",
    configUnknown: "Ayar veya adres bu denetimde doğrulanamadı. Uygulamalar’da tekrar inceleyin.",
    bindingUnknownNext: "Kayıtlı adres çalışan motorla eşleştirilemedi. Genel Bakış’ta motor durumunu denetleyin.",
    openApplications: "Uygulamaları aç",
    openOverview: "Genel Bakış’ı aç",
    refresh: "Durumu yenile",
    checkedAt: "Gözlem zamanı",
    lastRequest: "Son kaydedilen istek",
  },
  en: {
    title: "Check connection",
    intro: "Read-only check of the engine, saved settings, and local traffic signals from the last seven days.",
    scope: "It does not verify provider, account, or session success.",
    check: "Check now",
    checking: "Checking…",
    disabled: "You can check after the current operation finishes.",
    none: "No check has run yet.",
    failed: "The check could not finish. You can try again.",
    nativeOnly: "This check is available in the PennyPinch desktop app.",
    requestBusy: "Another check is running. Try again when it finishes.",
    stale: "This result belongs to an earlier context. Rescan and verify the version; reopen the client if settings changed.",
    runtime: "Engine",
    ingress: "Local ingress",
    clients: "Application settings",
    traffic: "Local traffic signal · last 7 days",
    observed: "Records found",
    noTraffic: "No records; this does not prove a leak or zero savings.",
    trafficUnavailable: "Records could not be read; traffic outcome is unknown.",
    noError: "No error was reported; this does not prove a successful request.",
    errorReported: "An error was reported for a recorded request; its source is unknown. If the client shows a 401/403 or sign-in error, sign in there.",
    outcomeUnknown: "Error outcome is unknown.",
    configSupported: "settings format recognized",
    configNotVerified: "settings format not verified",
    current: "current file",
    staleInstall: "file changed",
    notFound: "installation not found",
    installationUnknown: "installation unknown",
    matched: "PennyPinch address recorded",
    conflict: "settings conflict",
    notOwned: "no PennyPinch settings record",
    configUnavailable: "settings unavailable",
    busy: "settings check busy",
    notRechecked: "settings not rechecked",
    userScope: "user settings",
    projectScope: "project settings",
    scopeUnknown: "scope unknown",
    bindingSame: "address matches",
    bindingDifferent: "address differs",
    bindingNotOwned: "address not owned",
    bindingUnknown: "address unknown",
    runtimeEnvironment: "Check the runtime environment or application package.",
    runtimeOwnership: "Inspect ownership and port state without touching another process, then refresh status.",
    runtimeConfig: "Review the settings preview in Applications.",
    runtimeLifecycle: "Check the engine state in Overview.",
    runtimeRetry: "The engine did not respond. Refresh status; if the issue continues, inspect the engine in Overview.",
    runtimeProtocol: "Check the engine response and application version in Overview.",
    runtimeUnavailable: "The engine is unavailable. Check its state in Overview.",
    staleInstallation: "Installation or version evidence is not current. Rescan in Applications and verify the new version’s settings and protocol path. Reopen the client if settings changed.",
    configReview: "Saved settings or the address may have changed. Review the preview in Applications.",
    configUnknown: "Settings or the address could not be verified. Review them in Applications.",
    bindingUnknownNext: "The saved address could not be matched to the running engine. Check engine status in Overview.",
    openApplications: "Open Applications",
    openOverview: "Open Overview",
    refresh: "Refresh status",
    checkedAt: "Observed",
    lastRequest: "Last recorded request",
  },
} as const;

const engineLabel = {
  unavailable: { tr: "kullanılamıyor", en: "unavailable" },
  stopped: { tr: "durduruldu", en: "stopped" },
  starting: { tr: "başlatılıyor", en: "starting" },
  ready: { tr: "hazır", en: "ready" },
  draining: { tr: "kapanıyor", en: "draining" },
  failed: { tr: "hata", en: "failed" },
  unknown: { tr: "bilinmiyor", en: "unknown" },
} as const;
const ingressLabel = {
  unavailable: { tr: "kullanılamıyor", en: "unavailable" },
  starting: { tr: "başlatılıyor", en: "starting" },
  ready: { tr: "hazır", en: "ready" },
  unknown: { tr: "bilinmiyor", en: "unknown" },
} as const;
const clientLabel = {
  codex: { tr: "Codex", en: "Codex" },
  claude_code: { tr: "Claude Code", en: "Claude Code" },
  opencode: { tr: "OpenCode", en: "OpenCode" },
} as const;

function formatTime(value: string, language: PreferenceLanguage): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "—";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function clientSummary(row: RecoveryClientSnapshot, language: PreferenceLanguage): string {
  const text = copy[language];
  const installation = {
    current: text.current,
    stale: text.staleInstall,
    not_found: text.notFound,
    unknown: text.installationUnknown,
  }[row.installation_status];
  const config = {
    matches_owned_endpoint: text.matched,
    conflict: text.conflict,
    not_owned: text.notOwned,
    unavailable: text.configUnavailable,
    busy: text.busy,
    not_rechecked: text.notRechecked,
  }[row.config_state];
  const scope = {
    user_config: text.userScope,
    project_local_config: text.projectScope,
    unknown: text.scopeUnknown,
  }[row.config_scope];
  const binding = {
    same: text.bindingSame,
    different: text.bindingDifferent,
    not_owned: text.bindingNotOwned,
    unknown: text.bindingUnknown,
  }[row.endpoint_binding];
  return `${installation} · ${row.compatibility === "config_supported" ? text.configSupported : text.configNotVerified} · ${config} · ${scope} · ${binding}`;
}

function snapshotGuidance(snapshot: RecoverySnapshot, language: PreferenceLanguage): { messages: string[]; actions: Array<"applications" | "overview" | "refresh"> } {
  const code = snapshot.runtime.error_code;
  const text = copy[language];
  const messages: string[] = [];
  const actions: Array<"applications" | "overview" | "refresh"> = [];
  const add = (message: string, action: "applications" | "overview" | "refresh") => {
    if (!messages.includes(message)) messages.push(message);
    if (!actions.includes(action)) actions.push(action);
  };
  if (code === "environment_blocked") add(text.runtimeEnvironment, "overview");
  else if (code === "ownership_lost" || code === "port_conflict" || code === "cleanup_failed") add(text.runtimeOwnership, "refresh");
  else if (code === "config_conflict") add(text.runtimeConfig, "applications");
  else if (code === "busy" || code === "timeout" || code === "engine_not_ready") add(text.runtimeRetry, "refresh");
  else if (code === "validation_failed" || code === "unsupported_protocol") add(text.runtimeProtocol, "overview");
  if (snapshot.runtime.engine_status === "stopped" || snapshot.runtime.engine_status === "failed") add(text.runtimeLifecycle, "overview");
  else if (snapshot.runtime.engine_status === "unavailable" && code === null) add(text.runtimeUnavailable, "overview");

  const rows = snapshot.clients;
  if (rows.some(row => row.installation_status === "stale" || row.compatibility === "not_verified")) add(text.staleInstallation, "applications");
  if (rows.some(row => row.config_state === "conflict" || row.endpoint_binding === "different")) add(text.configReview, "applications");
  if (rows.some(row => row.config_state === "not_owned")) add(text.notOwned, "applications");
  if (rows.some(row => row.config_state === "unavailable" || row.config_state === "not_rechecked")) add(text.configUnknown, "applications");
  if (rows.some(row => row.config_state === "busy")) add(text.configUnknown, "refresh");
  if (rows.some(row => row.endpoint_binding === "unknown" && row.config_state !== "not_owned")) add(text.bindingUnknownNext, "overview");
  return { messages, actions };
}

export function ProblemNotice({ language, contextKey, disabled, onOpenApplications, onOpenOverview }: Props) {
  const text = copy[language];
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<RecoveryResult | null>(null);
  const [resultContext, setResultContext] = useState<{ key: string; revision: number } | null>(null);
  const mounted = useRef(true);
  const pendingRef = useRef(false);
  const contextRevision = useRef({ key: contextKey, revision: 0 });

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useLayoutEffect(() => {
    if (contextRevision.current.key !== contextKey) {
      contextRevision.current = { key: contextKey, revision: contextRevision.current.revision + 1 };
    }
  }, [contextKey]);

  const check = async () => {
    if (disabled || pendingRef.current) return;
    const startedContext = { ...contextRevision.current };
    pendingRef.current = true;
    setPending(true);
    setResult(null);
    setResultContext(null);
    try {
      const next = await recoveryClient.getSnapshot();
      if (mounted.current) {
        setResult(next);
        setResultContext(startedContext);
      }
    } catch {
      if (mounted.current) {
        setResult({ kind: "request_failed" });
        setResultContext(startedContext);
      }
    } finally {
      pendingRef.current = false;
      if (mounted.current) setPending(false);
    }
  };

  const stale = result !== null && resultContext !== null
    && (resultContext.key !== contextKey || resultContext.revision !== contextRevision.current.revision);
  const snapshot = result?.kind === "snapshot" ? result.snapshot : null;
  const guidance = snapshot ? snapshotGuidance(snapshot, language) : null;

  return (
    <section className="problem-notice" aria-labelledby="problem-notice-title">
      <div className="problem-notice__intro">
        <div>
          <h2 id="problem-notice-title">{text.title}</h2>
          <p>{text.intro}</p>
          <p className="problem-notice__scope">{text.scope}</p>
        </div>
        <button className="button button-secondary problem-notice__check" type="button" onClick={check} disabled={disabled || pending}>
          {pending ? text.checking : text.check}
        </button>
      </div>
      {disabled && <p className="problem-notice__hint">{text.disabled}</p>}
      {result === null && <p className="problem-notice__hint">{text.none}</p>}
      {result?.kind === "desktop_unavailable" && (
        <p className="problem-notice__status" role="status">{text.nativeOnly}</p>
      )}
      {result?.kind === "client_busy" && (
        <p className="problem-notice__status" role="status">{text.requestBusy}</p>
      )}
      {result !== null && (result.kind === "invalid_response" || result.kind === "request_failed") && (
        <p className="problem-notice__status" role="status">{text.failed}</p>
      )}
      {result?.kind === "recovery_error" && (
        <p className="problem-notice__status" role="status">{result.code === "busy" ? text.requestBusy : text.failed}</p>
      )}
      {stale && <p className="problem-notice__status" role="status">{text.stale}</p>}
      {snapshot && !stale && (
        <div className="problem-notice__report" role="status" aria-live="polite">
          <dl className="problem-notice__facts">
            <div><dt>{text.runtime}</dt><dd>{engineLabel[snapshot.runtime.engine_status][language]}</dd></div>
            <div><dt>{text.ingress}</dt><dd>{ingressLabel[snapshot.runtime.ingress_status][language]}</dd></div>
            <div className="problem-notice__fact--wide">
              <dt>{text.clients}</dt>
              <dd className="problem-notice__clients">
                {snapshot.clients.map(row => <span key={row.client_kind}><strong>{clientLabel[row.client_kind][language]}</strong><small>{clientSummary(row, language)}</small></span>)}
              </dd>
            </div>
            <div className="problem-notice__fact--wide">
              <dt>{text.traffic}</dt>
              <dd>
                {snapshot.traffic.status === "observed" ? text.observed : snapshot.traffic.status === "none" ? text.noTraffic : text.trafficUnavailable}
                {snapshot.traffic.outcome === "no_error_reported" && <small>{text.noError}</small>}
                {snapshot.traffic.outcome === "error_reported" && <small>{text.errorReported}</small>}
                {snapshot.traffic.outcome === "unknown" && snapshot.traffic.status === "observed" && <small>{text.outcomeUnknown}</small>}
              </dd>
            </div>
          </dl>
          <p className="problem-notice__observed">{text.checkedAt}: {formatTime(snapshot.observed_at_utc, language)}</p>
          {snapshot.traffic.last_observed_utc && <p className="problem-notice__observed">{text.lastRequest}: {formatTime(snapshot.traffic.last_observed_utc, language)}</p>}
          {guidance?.messages.map(message => <p className="problem-notice__guidance" key={message}>{message}</p>)}
          {guidance?.actions.includes("applications") && <button className="button button-secondary" type="button" onClick={onOpenApplications} disabled={disabled || pending}>{text.openApplications}</button>}
          {guidance?.actions.includes("overview") && <button className="button button-secondary" type="button" onClick={onOpenOverview} disabled={disabled || pending}>{text.openOverview}</button>}
          {guidance?.actions.includes("refresh") && <button className="button button-secondary" type="button" onClick={check} disabled={disabled || pending}>{text.refresh}</button>}
        </div>
      )}
    </section>
  );
}
