import { useEffect, useRef, useState } from "react";
import {
  LatestRequestSequence,
  readRuntimeSnapshot,
  startEngine,
  stopEngine,
  type EngineStatus,
  type PublicErrorCode,
  type RuntimeSnapshot,
  type SnapshotErrorEnvelope,
  type SnapshotResult,
} from "./runtime/snapshot";
import {
  readPreferences,
  savePreferences,
  PreferencesRequestSequence,
  type PreferenceLanguage,
  type PreferenceTheme,
  type PreferencesResult,
  type UserPreferences,
} from "./lib/preferences";
import { Applications } from "./features/integrations/Applications";
import { discoverySession } from "./lib/integrations";

type Screen = "overview" | "applications" | "activity" | "settings";
type LifecycleAction = "start" | "stop";
type PreferenceUiStatus = "loading" | "loaded" | "saving" | "saved" | "error" | "session_only";

const copy = {
  tr: {
    nav: ["Genel bakış", "Uygulamalar", "Etkinlik", "Ayarlar"],
    eyebrow: "YEREL KONTROL ALANI",
    headline: ["Kontrol paneli", "Uygulamalar", "Etkinlik", "Tercihler"],
    subtitle: ["Yerel model trafiği için tek bir görünüm.", "Bağlantı durumu ve tarama kapsamı.", "Ölçüm verisi geldiğinde burada görünür.", "Görünüm ve dil tercihleri."],
    engine: "Motor", connection: "Bağlantı", compression: "Sıkıştırma kontrolü",
    noConnection: "Doğrulanmadı", compressionOff: "Uygulanmadı",
    engineUnavailable: "Kullanılamıyor", engineUnknown: "Durum bilinmiyor", engineStopped: "Durduruldu",
    engineStarting: "Başlatılıyor", engineReady: "Hazır", engineStopping: "Durduruluyor", engineFailed: "Hata",
    metric: "Toplam tasarruf", awaiting: "Veri bekleniyor", amount: "Miktar", rate: "Oran",
    start: "Başlat", stop: "Durdur", starting: "Başlatılıyor…", stopping: "Durduruluyor…", pause: "Duraklat", connect: "Bağlan", unavailable: "Bu işlem henüz kullanılamıyor",
    hintUnavailable: "Masaüstü motor denetimi kullanılamıyor.", hintReady: "Motor hazır; istemci trafiği ayrıca doğrulanır.",
    engineRequirement: "Temel motor gerekir; ağır ek araçlar normal bağlantı için isteğe bağlıdır.",
    hintStopped: "Motor durduruldu. Başlatma bu masaüstü oturumunda kullanılabilir.", hintStarting: "Motor başlatılıyor.",
    hintStopping: "Motor durduruluyor.", hintFailed: "Motor bir hata bildirdi. Sabit hata kodu aşağıda gösteriliyor.",
    hintUnknown: "Motor durumu henüz doğrulanmadı.", hintUnavailableEngine: "Motor bu ortamda kullanılamıyor.",
    overviewTitle: "Henüz ölçüm yok", overviewBody: "Headroom yerel durumunu gösteriyor. Tasarruf değerleri, ölçüm kaynağı hazır olduğunda görünür.",
    clientsTitle: "Tarama yapılmadı", clientsBody: "Bu ekranda bulunan istemciler gösterilir. Henüz hiçbir kurulum incelenmedi.",
    activityTitle: "Etkinlik kaydı bekleniyor", activityBody: "Ölçülebilir istek ve tasarruf özetleri burada listelenir. Etkinlik kaydı henüz uygulanmadı.",
    settingsTitle: "Görünüm", settingsBody: "Dil ve tema tercihleri.", theme: "Tema", language: "Dil", system: "Sistem", dark: "Koyu", light: "Açık", turkish: "Türkçe", english: "English",
    sessionOnly: "Bu seçimler yalnızca bu masaüstü oturumunda geçerli.",
    preferencesLoading: "Tercihler yükleniyor…", preferencesLoaded: "Tercihler yüklendi.", preferencesSaving: "Tercihler kaydediliyor…", preferencesSaved: "Tercihler kaydedildi.", preferencesSessionOnly: "Yalnızca bu oturumda geçerli; masaüstü bağlantısı kullanılamıyor.", preferencesError: "Tercih işlemi tamamlanamadı.", preferencesLoadFailed: "Tercihler doğrulanamadı. Kayıtlı ayarlar değiştirilmedi.", preferencesSaveFailed: "Tercihler kaydedilemedi; önceki kayıtlı seçimler korundu.", preferencesRetry: "Yeniden oku",
    preferenceErrors: {
      validation_failed: "Tercih isteği doğrulanamadı.", unsupported_schema: "Kayıtlı tercihler okunamıyor; dosya korunuyor.",
      environment_blocked: "Tercih deposu bu ortamda kullanılamıyor.", busy: "Tercih deposu meşgul. Yeniden okumayı dene.",
      config_conflict: "Tercihler başka bir işlemle değişti. Önce yeniden oku.",
    },
    bridgeUnavailable: "Masaüstü bağlantısı kullanılamıyor", invalidSnapshot: "Yerel durum yanıtı doğrulanamadı", snapshotFailed: "Yerel durum okunamadı",
    nativeSnapshot: "Yerel durum güncel", refresh: "Durumu yenile", footer: "YEREL İSTATİSTİKLER",
    loading: "Yerel durum okunuyor", systemTheme: "Sistem tercihini izle veya sabit görünüm seç.", interfaceLanguage: "Arayüz dili.",
    commandErrors: {
      validation_failed: "Komut isteği doğrulanamadı.", unsupported_protocol: "Motor denetim protokolü desteklenmiyor.",
      environment_blocked: "Headroom'un temel paketini, disk alanını ve yazma izinlerini kontrol et. Ardından motoru başlatıp uygulamayı yeniden açmayı dene; bu ileti kesin nedeni belirtmez.", busy: "Motor başka bir işlem yürütüyor. Biraz sonra yeniden dene.",
      port_conflict: "Motorun yerel bağlantı noktası kullanılamıyor. Biraz sonra yeniden dene.",
      engine_not_ready: "Motor hazır duruma geçmedi. Durumu yenileyip yeniden dene.", ownership_lost: "Motor sürecinin sahipliği doğrulanamadı.",
      timeout: "Motor denetimi zaman aşımına uğradı. Durumu yenileyip yeniden dene.", cleanup_failed: "Normal kapanış doğrulanamadı. Güncel motor durumunu kontrol et.",
      config_conflict: "Motor yapılandırması beklenen profille eşleşmiyor. Temel paketi, disk alanını ve yazma izinlerini kontrol edip motoru başlatmayı veya uygulamayı yeniden açmayı dene.",
    },
  },
  en: {
    nav: ["Overview", "Applications", "Activity", "Settings"],
    eyebrow: "LOCAL CONTROL SURFACE",
    headline: ["Overview", "Applications", "Activity", "Preferences"],
    subtitle: ["A single view for local model traffic.", "Connection status and discovery scope.", "Measurement data will appear here when available.", "Appearance and language preferences."],
    engine: "Engine", connection: "Connection", compression: "Compression control",
    noConnection: "Not verified", compressionOff: "Not implemented",
    engineUnavailable: "Unavailable", engineUnknown: "Status unknown", engineStopped: "Stopped",
    engineStarting: "Starting", engineReady: "Ready", engineStopping: "Stopping", engineFailed: "Failed",
    metric: "Total savings", awaiting: "Waiting for data", amount: "Amount", rate: "Rate",
    start: "Start", stop: "Stop", starting: "Starting…", stopping: "Stopping…", pause: "Pause", connect: "Connect", unavailable: "This action is not available yet",
    hintUnavailable: "Desktop engine control is unavailable.", hintReady: "The engine is ready; client traffic is reported separately.",
    engineRequirement: "The core engine is required; heavy optional tools are not needed for a normal connection.",
    hintStopped: "The engine is stopped. Start is available in this desktop session.", hintStarting: "The engine is starting.",
    hintStopping: "The engine is stopping.", hintFailed: "The engine reported an error. Its fixed error code is shown below.",
    hintUnknown: "The engine state has not been verified yet.", hintUnavailableEngine: "The engine is unavailable in this environment.",
    overviewTitle: "No measurements yet", overviewBody: "Headroom is showing local state. Savings will appear when a measurement source is ready.",
    clientsTitle: "No scan performed", clientsBody: "Detected clients will appear here. No installation has been inspected yet.",
    activityTitle: "Waiting for activity", activityBody: "Measured request and savings summaries will appear here. Activity recording is not implemented yet.",
    settingsTitle: "Appearance", settingsBody: "Language and appearance preferences.", theme: "Theme", language: "Language", system: "System", dark: "Dark", light: "Light", turkish: "Türkçe", english: "English",
    sessionOnly: "These choices apply only to this desktop session.",
    preferencesLoading: "Loading preferences…", preferencesLoaded: "Preferences loaded.", preferencesSaving: "Saving preferences…", preferencesSaved: "Preferences saved.", preferencesSessionOnly: "Session only; the desktop bridge is unavailable.", preferencesError: "The preference operation could not be completed.", preferencesLoadFailed: "Preferences could not be validated. Saved settings were not changed.", preferencesSaveFailed: "Preferences could not be saved; the previous saved choices were kept.", preferencesRetry: "Read again",
    preferenceErrors: {
      validation_failed: "The preference request could not be validated.", unsupported_schema: "Saved preferences could not be read; the file is preserved.",
      environment_blocked: "The preferences store is unavailable in this environment.", busy: "The preferences store is busy. Read it again before retrying.",
      config_conflict: "Preferences changed elsewhere. Read them again before retrying.",
    },
    bridgeUnavailable: "Desktop bridge unavailable", invalidSnapshot: "Local state response could not be validated", snapshotFailed: "Local state could not be read",
    nativeSnapshot: "Local state updated", refresh: "Refresh status", footer: "LOCAL STATISTICS",
    loading: "Reading local state", systemTheme: "Follow system preference or choose an appearance.", interfaceLanguage: "Interface language.",
    commandErrors: {
      validation_failed: "The engine command could not be validated.", unsupported_protocol: "The engine control protocol is unsupported.",
      environment_blocked: "Check Headroom's core package, available disk space, and write permission. Then try starting the engine or reopening the app; this message does not identify a specific cause.", busy: "The engine is handling another operation. Try again shortly.",
      port_conflict: "The engine loopback port is unavailable. Try again shortly.",
      engine_not_ready: "The engine did not become ready. Refresh its state and try again.", ownership_lost: "Ownership of the engine process could not be verified.",
      timeout: "Engine control timed out. Refresh its state and try again.", cleanup_failed: "Engine shutdown was not verified. Check the current engine state.",
      config_conflict: "The engine configuration does not match the required profile. Check the core package, available disk space, and write permission, then try starting the engine or reopening the app.",
    },
  },
} satisfies Record<PreferenceLanguage, Record<string, unknown>>;

type Labels = typeof copy.tr;
type PreferenceIssue = { operation: "load" | "save"; result: PreferencesResult };

function resolveTheme(preference: PreferenceTheme): "light" | "dark" {
  if (preference !== "system") return preference;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function preferenceIssueMessage(issue: PreferenceIssue | null, labels: Labels): string | null {
  if (!issue) return null;
  if (issue.result.kind === "preferences_error") {
    return labels.preferenceErrors[issue.result.error.error.code];
  }
  if (issue.result.kind === "invalid_response") {
    return issue.operation === "save" ? labels.preferencesSaveFailed : labels.preferencesLoadFailed;
  }
  if (issue.result.kind === "request_failed") {
    return issue.operation === "save" ? labels.preferencesSaveFailed : labels.preferencesLoadFailed;
  }
  return null;
}

function preferenceStatusLabel(status: PreferenceUiStatus, labels: Labels): string {
  const values: Record<PreferenceUiStatus, string> = {
    loading: labels.preferencesLoading,
    loaded: labels.preferencesLoaded,
    saving: labels.preferencesSaving,
    saved: labels.preferencesSaved,
    error: labels.preferencesError,
    session_only: labels.preferencesSessionOnly,
  };
  return values[status];
}

function engineLabel(status: EngineStatus, labels: Labels): string {
  const values: Record<EngineStatus, string> = {
    unavailable: labels.engineUnavailable,
    unknown: labels.engineUnknown,
    stopped: labels.engineStopped,
    starting: labels.engineStarting,
    ready: labels.engineReady,
    stopping: labels.engineStopping,
    failed: labels.engineFailed,
  };
  return values[status];
}

function SnapshotNotice({
  readIssue,
  commandError,
  engineError,
  onRetry,
  labels,
}: {
  readIssue: SnapshotResult | null;
  commandError: SnapshotErrorEnvelope | null;
  engineError: PublicErrorCode | null;
  onRetry: () => void;
  labels: Labels;
}) {
  const code = readIssue?.kind === "snapshot_error" ? readIssue.error.error.code : commandError?.error.code ?? engineError;
  const message = code ? labels.commandErrors[code] :
    readIssue?.kind === "desktop_unavailable" ? labels.bridgeUnavailable :
      readIssue?.kind === "invalid_response" ? labels.invalidSnapshot :
        readIssue?.kind === "request_failed" ? labels.snapshotFailed : null;
  if (!message) return null;
  return <div className="notice" role="alert"><span>{message}</span><button className="text-button" onClick={onRetry}>{labels.refresh}</button></div>;
}

function StatusCard({ title, value, symbol, tone = "neutral" }: { title: string; value: string; symbol: string; tone?: "neutral" | "mint" }) {
  return <section className={`status-card tone-${tone}`} aria-label={`${title}: ${value}`}>
    <div className="status-card__head"><span>{title}</span><span className="status-icon" aria-hidden="true">{symbol}</span></div>
    <strong>{value}</strong>
  </section>;
}

export default function App() {
  const [screen, setScreen] = useState<Screen>("overview");
  const [theme, setTheme] = useState<PreferenceTheme>("system");
  const [language, setLanguage] = useState<PreferenceLanguage>("tr");
  const [preferencesStatus, setPreferencesStatus] = useState<PreferenceUiStatus>("loading");
  const [preferencesIssue, setPreferencesIssue] = useState<PreferenceIssue | null>(null);
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot | null>(null);
  const [readIssue, setReadIssue] = useState<SnapshotResult | null>(null);
  const [commandError, setCommandError] = useState<SnapshotErrorEnvelope | null>(null);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<LifecycleAction | null>(null);
  const requestSequence = useRef(new LatestRequestSequence());
  const mounted = useRef(false);
  const lifecycleLocked = useRef(false);
  const pollInFlight = useRef(false);
  const preferencesSequence = useRef(new PreferencesRequestSequence());
  const preferencesWriteLocked = useRef(false);
  const preferencesRevision = useRef(0);
  const preferencesFingerprint = useRef<string | null>(null);
  const persistedPreferences = useRef<UserPreferences>({ language: "tr", theme: "system" });
  const labels = copy[language] as Labels;
  const nav = labels.nav as string[];
  const headings = labels.headline as string[];
  const subtitles = labels.subtitle as string[];
  const status = snapshot?.engine.status;

  async function loadSnapshot(isBackground = false) {
    const sequence = requestSequence.current.next();
    if (!isBackground && !snapshot) setLoading(true);
    const next = await readRuntimeSnapshot();
    if (!mounted.current || !requestSequence.current.isCurrent(sequence)) return;
    if (next.kind === "snapshot") {
      setSnapshot(next.snapshot);
      setReadIssue(null);
    } else {
      setSnapshot(null);
      setReadIssue(next);
    }
    setLoading(false);
  }

  async function reloadPreferences() {
    const sequence = preferencesSequence.current.nextRead();
    setPreferencesStatus("loading");
    setPreferencesIssue(null);
    const result = await readPreferences();
    if (!mounted.current || !preferencesSequence.current.isReadCurrent(sequence)) return;
    if (result.kind === "snapshot") {
      persistedPreferences.current = result.snapshot.preferences;
      preferencesRevision.current = result.snapshot.revision;
      preferencesFingerprint.current = result.snapshot.state_fingerprint;
      setLanguage(result.snapshot.preferences.language);
      setTheme(result.snapshot.preferences.theme);
      setPreferencesIssue(null);
      setPreferencesStatus("loaded");
    } else if (result.kind === "desktop_unavailable") {
      persistedPreferences.current = { language, theme };
      setPreferencesIssue(null);
      setPreferencesStatus("session_only");
    } else {
      setPreferencesIssue({ operation: "load", result });
      setPreferencesStatus("error");
    }
  }

  async function changePreferences(change: Partial<UserPreferences>) {
    const next: UserPreferences = {
      language: change.language ?? language,
      theme: change.theme ?? theme,
    };
    if (preferencesStatus === "session_only") {
      persistedPreferences.current = next;
      setLanguage(next.language);
      setTheme(next.theme);
      return;
    }
    if ((preferencesStatus !== "loaded" && preferencesStatus !== "saved") || preferencesWriteLocked.current) return;

    preferencesWriteLocked.current = true;
    const sequence = preferencesSequence.current.nextWrite();
    const previous = persistedPreferences.current;
    setPreferencesStatus("saving");
    setPreferencesIssue(null);
    setLanguage(next.language);
    setTheme(next.theme);
    try {
      const result = await savePreferences(
        next,
        preferencesRevision.current,
        preferencesFingerprint.current,
      );
      if (!mounted.current || !preferencesSequence.current.isWriteCurrent(sequence)) return;
      if (result.kind === "snapshot") {
        persistedPreferences.current = result.snapshot.preferences;
        preferencesRevision.current = result.snapshot.revision;
        preferencesFingerprint.current = result.snapshot.state_fingerprint;
        setLanguage(result.snapshot.preferences.language);
        setTheme(result.snapshot.preferences.theme);
        setPreferencesStatus("saved");
      } else {
        setLanguage(previous.language);
        setTheme(previous.theme);
        setPreferencesIssue({ operation: "save", result });
        setPreferencesStatus("error");
      }
    } finally {
      preferencesWriteLocked.current = false;
    }
  }

  async function runLifecycle(action: LifecycleAction) {
    if (lifecycleLocked.current || !snapshot) return;
    if (action === "start" && !snapshot.capabilities.start) return;
    if (action === "stop" && (!snapshot.capabilities.stop || !snapshot.runtime_generation)) return;
    lifecycleLocked.current = true;
    setPending(action);
    setCommandError(null);
    const commandSequence = requestSequence.current.next();
    try {
      const commandResult = action === "start" ? await startEngine() : await stopEngine(snapshot.runtime_generation!);
      if (!mounted.current) return;
      if (commandResult.kind === "snapshot") {
        if (requestSequence.current.isCurrent(commandSequence)) {
          setSnapshot(commandResult.snapshot);
          setReadIssue(null);
        }
        setCommandError(null);
      } else if (commandResult.kind === "snapshot_error") {
        setCommandError(commandResult.error);
      } else {
        setReadIssue(commandResult);
      }
      await loadSnapshot(true);
    } finally {
      lifecycleLocked.current = false;
      if (mounted.current) setPending(null);
    }
  }

  function retrySnapshot() {
    setCommandError(null);
    void loadSnapshot();
  }

  useEffect(() => {
    mounted.current = true;
    void loadSnapshot();
    void discoverySession.loadInitial().catch(() => undefined);
    void reloadPreferences();
    const timer = window.setInterval(() => {
      if (pollInFlight.current) return;
      pollInFlight.current = true;
      void loadSnapshot(true).finally(() => { pollInFlight.current = false; });
    }, 1000);
    return () => {
      mounted.current = false;
      requestSequence.current.invalidate();
      preferencesSequence.current.invalidate();
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-color-scheme: light)");
    const update = () => {
      document.documentElement.dataset.theme = resolveTheme(theme);
    };
    update();
    if (theme !== "system" || !media) return;
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [theme]);

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const screenKey: Screen[] = ["overview", "applications", "activity", "settings"];
  const content = screen === "applications" ? <Applications language={language} /> : renderScreen(
    screen,
    labels,
    snapshot,
    theme,
    language,
    pending,
    runLifecycle,
    changePreferences,
    preferencesStatus,
    preferenceIssueMessage(preferencesIssue, labels),
    reloadPreferences,
    () => setScreen("applications"),
  );

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#overview" onClick={(event) => { event.preventDefault(); setScreen("overview"); }} aria-label={language === "tr" ? "Headroom genel bakış" : "Headroom overview"}>
        <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
        <span>headroom<span className="brand-period">.</span></span>
      </a>
      <div className="sidebar-label">{language === "tr" ? "ÇALIŞMA ALANI" : "WORKSPACE"}</div>
      <nav className="primary-nav" aria-label={language === "tr" ? "Ana gezinme" : "Primary navigation"}>
        {screenKey.map((key, index) => <button key={key} className={`nav-item ${screen === key ? "is-active" : ""}`} onClick={() => setScreen(key)} aria-current={screen === key ? "page" : undefined}>
          <span className={`nav-glyph glyph-${key}`} aria-hidden="true">{["◫", "▤", "◷", "⚙"][index]}</span><span>{nav[index]}</span>
        </button>)}
      </nav>
      <div className="sidebar-bottom">
        <div className="local-mark"><span className="local-dot" />{labels.footer}</div>
        <div className="build-label">DESKTOP · 0.1</div>
      </div>
    </aside>

    <main className="main-panel">
      <header className="page-header">
        <div><div className="eyebrow">{labels.eyebrow}</div><h1>{headings[screenKey.indexOf(screen)]}</h1><p>{subtitles[screenKey.indexOf(screen)]}</p></div>
        <div className="header-status" aria-live="polite"><span className={`pulse-dot ${status === "ready" ? "is-ready" : ""}`} />{loading ? labels.loading : snapshot ? labels.nativeSnapshot : (language === "tr" ? "Yerel durum" : "Local state")}</div>
      </header>
      <SnapshotNotice readIssue={readIssue} commandError={commandError} engineError={snapshot?.engine.error_code ?? null} onRetry={retrySnapshot} labels={labels} />
      {content}
      <footer className="mobile-footer">{labels.footer}</footer>
    </main>
  </div>;
}

function renderScreen(
  screen: Exclude<Screen, "applications">,
  labels: Labels,
  snapshot: RuntimeSnapshot | null,
  theme: PreferenceTheme,
  language: PreferenceLanguage,
  pending: LifecycleAction | null,
  runLifecycle: (action: LifecycleAction) => void,
  onPreferencesChange: (change: Partial<UserPreferences>) => void,
  preferencesStatus: PreferenceUiStatus,
  preferencesErrorMessage: string | null,
  onPreferencesRetry: () => void,
  onOpenApplications: () => void,
) {
  if (screen === "overview") {
    const engineValue = snapshot ? engineLabel(snapshot.engine.status, labels) : labels.engineUnknown;
    const hint = !snapshot ? labels.hintUnavailable : snapshot.engine.status === "ready" ? labels.hintReady :
      snapshot.engine.status === "stopped" ? labels.hintStopped : snapshot.engine.status === "starting" ? labels.hintStarting :
        snapshot.engine.status === "stopping" ? labels.hintStopping : snapshot.engine.status === "failed" ? labels.hintFailed :
          snapshot.engine.status === "unavailable" ? labels.hintUnavailableEngine : labels.hintUnknown;
    const startDisabled = !snapshot?.capabilities.start || pending !== null;
    const stopDisabled = !snapshot?.capabilities.stop || !snapshot.runtime_generation || pending !== null;
    return <>
      <div className="status-grid">
        <StatusCard title={labels.engine} value={engineValue} symbol="◉" tone={snapshot?.engine.status === "ready" ? "mint" : "neutral"} />
        <StatusCard title={labels.connection} value={labels.noConnection} symbol="↔" />
        <StatusCard title={labels.compression} value={snapshot?.compression.status === "not_implemented" ? labels.compressionOff : labels.awaiting} symbol="⌁" />
      </div>
      <section className="savings-panel">
        <div className="panel-kicker"><span className="mint-line" />{labels.metric.toUpperCase()}</div>
        <div className="savings-values">
          <div className="savings-amount"><span>{labels.amount}</span><strong>—</strong><small>{labels.awaiting}</small></div>
          <div className="savings-divider" aria-hidden="true" />
          <div className="savings-rate"><span>{labels.rate}</span><strong>—<em>%</em></strong><small>{labels.awaiting}</small></div>
        </div>
        <div className="savings-footnote"><span className="info-mark">i</span>{language === "tr" ? "Yüzde ve miktar aynı ölçüm kapsamından birlikte hesaplanır." : "Percentage and amount are calculated from the same measurement scope."}</div>
      </section>
      <section className="empty-panel">
        <div className="empty-illustration" aria-hidden="true"><span className="orbit orbit-one" /><span className="orbit orbit-two" /><span className="orbit-core"><i /></span></div>
        <div className="empty-copy"><div className="panel-kicker">{language === "tr" ? "BAŞLANGIÇ DURUMU" : "INITIAL STATE"}</div><h2>{labels.overviewTitle}</h2><p>{labels.overviewBody}</p></div>
      </section>
      <section className="actions-panel">
        <div><div className="panel-kicker">{language === "tr" ? "MOTOR KONTROLÜ" : "ENGINE CONTROL"}</div><p>{hint}</p><small className="engine-requirement">{labels.engineRequirement}</small></div>
        <div className="action-row">
          <button className="button button-primary" disabled={startDisabled} title={startDisabled ? labels.unavailable : undefined} onClick={() => runLifecycle("start")}><span aria-hidden="true">▶</span>{pending === "start" ? labels.starting : labels.start}</button>
          <button className="button" disabled={stopDisabled} title={stopDisabled ? labels.unavailable : undefined} onClick={() => runLifecycle("stop")}><span aria-hidden="true">■</span>{pending === "stop" ? labels.stopping : labels.stop}</button>
          <button className="button" disabled title={labels.unavailable}><span aria-hidden="true">Ⅱ</span>{labels.pause}</button>
          <button className="button button-connect" onClick={onOpenApplications}><span aria-hidden="true">↗</span>{labels.connect}</button>
        </div>
      </section>
    </>;
  }
  if (screen === "activity") return <section className="empty-screen-card">
    <span className="large-outline-icon" aria-hidden="true">◷</span><div className="panel-kicker">{language === "tr" ? "ÖLÇÜM GEÇMİŞİ" : "MEASUREMENT HISTORY"}</div><h2>{labels.activityTitle}</h2><p>{labels.activityBody}</p>
  </section>;
  const preferencesBusy = preferencesStatus === "loading" || preferencesStatus === "saving";
  const preferencesUnavailable = preferencesBusy || preferencesStatus === "error";
  return <section className="settings-card">
    <div className="settings-intro"><div className="panel-kicker">{language === "tr" ? "KİŞİSELLEŞTİRME" : "PERSONALIZATION"}</div><h2>{labels.settingsTitle}</h2><p>{labels.settingsBody}</p></div>
    <label className="setting-row"><span><strong>{labels.theme}</strong><small>{labels.systemTheme}</small></span>
      <select aria-label={labels.theme} disabled={preferencesUnavailable} value={theme} onChange={(event) => onPreferencesChange({ theme: event.target.value as PreferenceTheme })}><option value="system">{labels.system}</option><option value="dark">{labels.dark}</option><option value="light">{labels.light}</option></select>
    </label>
    <label className="setting-row"><span><strong>{labels.language}</strong><small>{labels.interfaceLanguage}</small></span>
      <select aria-label={labels.language} disabled={preferencesUnavailable} value={language} onChange={(event) => onPreferencesChange({ language: event.target.value as PreferenceLanguage })}><option value="tr">{labels.turkish}</option><option value="en">{labels.english}</option></select>
    </label>
    <div className="settings-note" role="status" aria-live="polite">{preferenceStatusLabel(preferencesStatus, labels)}</div>
    {preferencesStatus === "error" && <div className="notice preference-notice" role="alert">
      <span>{preferencesErrorMessage ?? labels.preferencesLoadFailed}</span>
      <button className="text-button" disabled={preferencesBusy} onClick={onPreferencesRetry}>{labels.preferencesRetry}</button>
    </div>}
  </section>;
}
