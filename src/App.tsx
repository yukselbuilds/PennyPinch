import { useCallback, useEffect, useRef, useState } from "react";
import {
  LatestRequestSequence,
  readRuntimeSnapshot,
  startEngine,
  stopEngine,
  setOptimization,
  type PublicErrorCode,
  type RuntimeSnapshot,
  type SnapshotErrorEnvelope,
  type SnapshotResult,
} from "./runtime/snapshot";
import {
  readPreferences,
  savePreferences,
  defaultOperatingPreferences,
  PreferencesRequestSequence,
  type PreferenceLanguage,
  type PreferenceTheme,
  type PreferencesResult,
  type UserPreferences,
  type OperatingPreferences,
} from "./lib/preferences";
import { Applications } from "./features/integrations/Applications";
import type { ClientKind } from "./lib/integrations";
import { Overview } from "./features/overview/Overview";
import { Activity } from "./features/activity/Activity";
import { useActivity } from "./features/activity/useActivity";
import { useOverviewConnections, useOverviewMetrics } from "./features/overview/useOverview";
import type { MetricPeriod } from "./lib/metrics";
import type { RuntimeAction } from "./lib/optimization";
import { windowZoomClient, type WindowScale } from "./lib/windowZoom";
import { HeadroomMark, Icon } from "./components/Icon";
import { ProblemNotice } from "./components/ProblemNotice";
import { DesktopLifecycle } from "./components/DesktopLifecycle";
import { Settings } from "./features/settings/Settings";

type Screen = "overview" | "applications" | "activity" | "settings";
type LifecycleAction = RuntimeAction;
type WindowZoomView = { scale: WindowScale; pending: boolean; failed: boolean; change: (scale: WindowScale) => void };
type PreferenceUiStatus = "loading" | "loaded" | "saving" | "saved" | "error" | "session_only";

const copy = {
  tr: {
    nav: ["Genel bakış", "Uygulamalar", "Etkinlik", "Ayarlar"],
    eyebrow: "YEREL KONTROL ALANI",
    headline: ["Genel bakış", "Uygulamalar", "Etkinlik", "Ayarlar"],
    subtitle: ["Daha az bağlam. Daha fazla alan.", "Kodlama araçlarını Headroom’a bağla.", "İsteklerini ve ölçümlerini takip et.", "Headroom’u kendine göre ayarla."],
    engine: "Motor", connection: "Bağlantı", compression: "Sıkıştırma kontrolü",
    noConnection: "Doğrulanmadı", compressionOff: "Uygulanmadı",
    engineUnavailable: "Kullanılamıyor", engineUnknown: "Durum bilinmiyor", engineStopped: "Durduruldu",
    engineStarting: "Başlatılıyor", engineReady: "Hazır", engineStopping: "Durduruluyor", engineFailed: "Hata",
    metric: "Toplam tasarruf", awaiting: "Veri bekleniyor", amount: "Miktar", rate: "Oran",
    start: "Başlat", stop: "Durdur", starting: "Başlatılıyor…", stopping: "Durduruluyor…", pause: "Duraklat", connect: "Bağlan", unavailable: "Bu işlem henüz kullanılamıyor",
    hintUnavailable: "Masaüstü motor denetimi kullanılamıyor.", hintReady: "Motor hazır; istemci trafiği ayrıca doğrulanır.",
    engineRequirement: "Temel motor gerekir; ağır ek araçlar normal bağlantı için isteğe bağlıdır.",
    hintStopped: "Motor durduruldu. Başlatma bu masaüstü oturumunda kullanılabilir.", hintStarting: "Motor başlatılıyor.",
    hintStopping: "Motor durduruluyor.", hintFailed: "Motor bir hata bildirdi; aşağıdaki iletiye bakın.",
    hintUnknown: "Motor durumu henüz doğrulanmadı.", hintUnavailableEngine: "Motor bu ortamda kullanılamıyor.",
    overviewTitle: "Henüz ölçüm yok", overviewBody: "Headroom yerel durumunu gösteriyor. Tasarruf değerleri, ölçüm kaynağı hazır olduğunda görünür.",
    clientsTitle: "Tarama yapılmadı", clientsBody: "Bu ekranda bulunan istemciler gösterilir. Henüz hiçbir kurulum incelenmedi.",
    settingsTitle: "Görünüm", settingsBody: "Dil ve tema tercihleri.", theme: "Tema", language: "Dil", system: "Sistem", dark: "Koyu", light: "Açık", turkish: "Türkçe", english: "English",
    sessionOnly: "Bu seçimler yalnızca bu masaüstü oturumunda geçerli.",
    preferencesLoading: "Tercihler yükleniyor…", preferencesLoaded: "Tercihler yüklendi.", preferencesSaving: "Tercihler kaydediliyor…", preferencesSaved: "Tercihler kaydedildi.", preferencesSessionOnly: "Yalnızca bu oturumda geçerli; masaüstü bağlantısı kullanılamıyor.", preferencesError: "Tercih işlemi tamamlanamadı.", preferencesLoadFailed: "Tercihler doğrulanamadı. Kayıtlı ayarlar değiştirilmedi.", preferencesSaveFailed: "Tercihler kaydedilemedi; önceki kayıtlı seçimler korundu.", preferencesRetry: "Yeniden oku",
    preferenceErrors: {
      validation_failed: "Tercih isteği doğrulanamadı.", unsupported_schema: "Kayıtlı tercihler okunamıyor; dosya korunuyor.",
      environment_blocked: "Tercih deposu bu ortamda kullanılamıyor.", busy: "Tercih deposu meşgul. Yeniden okumayı dene.",
      config_conflict: "Tercihler başka bir işlemle değişti. Önce yeniden oku.",
    },
    bridgeUnavailable: "Masaüstü bağlantısı kullanılamıyor", invalidSnapshot: "Yerel durum yanıtı doğrulanamadı", snapshotFailed: "Yerel durum okunamadı",
    nativeSnapshot: "Yerel veriler", refresh: "Durumu yenile", footer: "Yerel ölçümler",
    loading: "Yerel durum okunuyor", systemTheme: "Sistem tercihini izle veya sabit görünüm seç.", interfaceLanguage: "Arayüz dili.",
    commandErrors: {
      validation_failed: "Komut isteği doğrulanamadı.", unsupported_protocol: "Motor denetim protokolü desteklenmiyor.",
      environment_blocked: "Headroom'un temel paketini, disk alanını ve yazma izinlerini kontrol et. Ardından motoru başlatıp uygulamayı yeniden açmayı dene; bu ileti kesin nedeni belirtmez.", busy: "Motor başka bir işlem yürütüyor. Biraz sonra yeniden dene.",
      port_conflict: "Motorun yerel bağlantı noktası kullanılamıyor. Biraz sonra yeniden dene.",
      engine_not_ready: "Motor hazır duruma geçmedi. Durumu yenileyip yeniden dene.", ownership_lost: "Motor sürecinin sahipliği doğrulanamadı.",
      timeout: "Motor denetimi zaman aşımına uğradı. Durumu yenileyip yeniden dene.", cleanup_failed: "Normal kapanış doğrulanamadı. Güncel motor durumunu kontrol et.",
      config_conflict: "Motorun durumu veya yapılandırması değişti. Durumu yenileyip yeniden dene.",
    },
  },
  en: {
    nav: ["Overview", "Applications", "Activity", "Settings"],
    eyebrow: "LOCAL CONTROL SURFACE",
    headline: ["Overview", "Applications", "Activity", "Preferences"],
    subtitle: ["Less context. More headroom.", "Connect your coding tools to Headroom.", "Keep track of requests and measurements.", "Make Headroom feel like home."],
    engine: "Engine", connection: "Connection", compression: "Compression control",
    noConnection: "Not verified", compressionOff: "Not implemented",
    engineUnavailable: "Unavailable", engineUnknown: "Status unknown", engineStopped: "Stopped",
    engineStarting: "Starting", engineReady: "Ready", engineStopping: "Stopping", engineFailed: "Failed",
    metric: "Total savings", awaiting: "Waiting for data", amount: "Amount", rate: "Rate",
    start: "Start", stop: "Stop", starting: "Starting…", stopping: "Stopping…", pause: "Pause", connect: "Connect", unavailable: "This action is not available yet",
    hintUnavailable: "Desktop engine control is unavailable.", hintReady: "The engine is ready; client traffic is reported separately.",
    engineRequirement: "The core engine is required; heavy optional tools are not needed for a normal connection.",
    hintStopped: "The engine is stopped. Start is available in this desktop session.", hintStarting: "The engine is starting.",
    hintStopping: "The engine is stopping.", hintFailed: "The engine reported an error; see the message below.",
    hintUnknown: "The engine state has not been verified yet.", hintUnavailableEngine: "The engine is unavailable in this environment.",
    overviewTitle: "No measurements yet", overviewBody: "Headroom is showing local state. Savings will appear when a measurement source is ready.",
    clientsTitle: "No scan performed", clientsBody: "Detected clients will appear here. No installation has been inspected yet.",
    settingsTitle: "Appearance", settingsBody: "Language and appearance preferences.", theme: "Theme", language: "Language", system: "System", dark: "Dark", light: "Light", turkish: "Türkçe", english: "English",
    sessionOnly: "These choices apply only to this desktop session.",
    preferencesLoading: "Loading preferences…", preferencesLoaded: "Preferences loaded.", preferencesSaving: "Saving preferences…", preferencesSaved: "Preferences saved.", preferencesSessionOnly: "Session only; the desktop bridge is unavailable.", preferencesError: "The preference operation could not be completed.", preferencesLoadFailed: "Preferences could not be validated. Saved settings were not changed.", preferencesSaveFailed: "Preferences could not be saved; the previous saved choices were kept.", preferencesRetry: "Read again",
    preferenceErrors: {
      validation_failed: "The preference request could not be validated.", unsupported_schema: "Saved preferences could not be read; the file is preserved.",
      environment_blocked: "The preferences store is unavailable in this environment.", busy: "The preferences store is busy. Read it again before retrying.",
      config_conflict: "Preferences changed elsewhere. Read them again before retrying.",
    },
    bridgeUnavailable: "Desktop bridge unavailable", invalidSnapshot: "Local state response could not be validated", snapshotFailed: "Local state could not be read",
    nativeSnapshot: "Local data", refresh: "Refresh status", footer: "Local measurements",
    loading: "Reading local state", systemTheme: "Follow system preference or choose an appearance.", interfaceLanguage: "Interface language.",
    commandErrors: {
      validation_failed: "The engine command could not be validated.", unsupported_protocol: "The engine control protocol is unsupported.",
      environment_blocked: "Check Headroom's core package, available disk space, and write permission. Then try starting the engine or reopening the app; this message does not identify a specific cause.", busy: "The engine is handling another operation. Try again shortly.",
      port_conflict: "The engine loopback port is unavailable. Try again shortly.",
      engine_not_ready: "The engine did not become ready. Refresh its state and try again.", ownership_lost: "Ownership of the engine process could not be verified.",
      timeout: "Engine control timed out. Refresh its state and try again.", cleanup_failed: "Engine shutdown was not verified. Check the current engine state.",
      config_conflict: "The engine state or configuration changed. Refresh its state and try again.",
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

export default function App() {
  const [screen, setScreen] = useState<Screen>("overview");
  const [period, setPeriod] = useState<MetricPeriod>("session");
  const [requestedClient, setRequestedClient] = useState<ClientKind | null>(null);
  const [applicationsBusy, setApplicationsBusy] = useState(false);
  const [recoveryRevision, setRecoveryRevision] = useState(0);
  const [recoveryScanGeneration, setRecoveryScanGeneration] = useState<number | null>(null);
  const applicationsLock = useRef(false);
  const onApplicationsBusy = useCallback((busy: boolean) => {
    if (busy && !applicationsLock.current) setRecoveryRevision(value => value + 1);
    applicationsLock.current = busy;
    setApplicationsBusy(busy);
  }, []);
  const overview = useOverviewMetrics(period, screen === "overview");
  const activity = useActivity(screen === "activity");
  const connections = useOverviewConnections(screen === "overview");
  const [theme, setTheme] = useState<PreferenceTheme>("system");
  const [windowScale, setWindowScale] = useState<WindowScale>(1);
  const [zoomPending, setZoomPending] = useState(false);
  const [zoomFailed, setZoomFailed] = useState(false);
  const zoomLock = useRef(false);
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
  const persistedPreferences = useRef<UserPreferences>({ language: "tr", theme: "system", operating: defaultOperatingPreferences() });
  const labels = copy[language] as Labels;
  const nav = labels.nav as string[];
  const headings = labels.headline as string[];
  const subtitles = labels.subtitle as string[];

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
      persistedPreferences.current = { language, theme, operating: defaultOperatingPreferences() };
      setPreferencesIssue(null);
      setPreferencesStatus("session_only");
    } else {
      setPreferencesIssue({ operation: "load", result });
      setPreferencesStatus("error");
    }
  }

  async function changePreferences(change: Partial<UserPreferences>) {
    const next: UserPreferences = {
      ...persistedPreferences.current,
      language: change.language ?? language,
      theme: change.theme ?? theme,
      operating: change.operating ?? persistedPreferences.current.operating,
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
    if (lifecycleLocked.current || applicationsLock.current || !snapshot) return;
    if (action === "start" && !snapshot.capabilities.start) return;
    if (action === "stop" && (!snapshot.capabilities.stop || !snapshot.runtime_generation)) return;
    if ((action==="pause"||action==="resume")&&(!snapshot.capabilities.pause||!snapshot.runtime_generation||snapshot.compression.revision===null
      ||snapshot.compression.status!==(action==="pause"?"enabled":"paused"))) return;
    lifecycleLocked.current = true;
    setPending(action);
    setCommandError(null);
    const commandSequence = requestSequence.current.next();
    try {
      const commandResult = action === "start" ? await startEngine() : action==="stop"?await stopEngine(snapshot.runtime_generation!)
        :await setOptimization(snapshot.runtime_generation!,snapshot.compression.revision!,action==="resume");
      if (!mounted.current) return;
      if ((action === "pause" || action === "resume") && commandResult.kind !== "snapshot") {
        setSnapshot(current => current?.engine.status === "ready" && current.runtime_generation === snapshot.runtime_generation
          ? { ...current, compression: { status: "unknown", active_stream_count: null, revision: null }, capabilities: { ...current.capabilities, pause: false } }
          : current);
      }
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

  async function changeWindowScale(scale: WindowScale) {
    if (zoomLock.current) return;
    zoomLock.current = true;
    setZoomPending(true);
    setZoomFailed(false);
    try {
      const changed = await windowZoomClient.set(scale);
      if (!mounted.current) return;
      if (changed) setWindowScale(scale);
      setZoomFailed(!changed);
    } finally {
      zoomLock.current = false;
      if (mounted.current) setZoomPending(false);
    }
  }

  useEffect(() => {
    mounted.current = true;
    void loadSnapshot();
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
  function navigate(next: Screen) {
    if (applicationsLock.current || lifecycleLocked.current) return;
    setRequestedClient(null);
    setScreen(next);
  }
  function connect(client: ClientKind) {
    if (applicationsLock.current || lifecycleLocked.current) return;
    if (connections.connections.some(item => item.client === client && item.state === "saved")) {
      navigate("applications");
      return;
    }
    setRequestedClient(client);
    setScreen("applications");
  }
  const content = screen === "overview" ? <Overview language={language} runtime={snapshot} pending={pending}
    period={period} metrics={overview.metrics} connections={connections.connections} connectionsLoading={connections.loading}
    onPeriodChange={setPeriod} onRefresh={overview.refresh} onConnect={connect}
    onOpenApplications={() => navigate("applications")} onStart={() => void runLifecycle("start")} onStop={() => void runLifecycle("stop")}
    onPause={()=>void runLifecycle("pause")} onResume={()=>void runLifecycle("resume")} onRuntimeRefresh={retrySnapshot} />
    : screen === "applications" ? <Applications language={language} requestedClient={requestedClient} onBusyChange={onApplicationsBusy} onDiscoveryChange={setRecoveryScanGeneration} />
    : screen === "activity" ? <Activity language={language} {...activity} /> : renderSettings(
    labels,
    theme,
    language,
    changePreferences,
    preferencesStatus,
    preferenceIssueMessage(preferencesIssue, labels),
    reloadPreferences,
    { scale: windowScale, pending: zoomPending, failed: zoomFailed, change: changeWindowScale },
    persistedPreferences.current.operating,
    (operating) => changePreferences({ operating }),
    onApplicationsBusy,
  );

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#overview" aria-disabled={applicationsBusy || pending !== null} onClick={(event) => { event.preventDefault(); navigate("overview"); }} aria-label={language === "tr" ? "Headroom genel bakış" : "Headroom overview"}>
        <HeadroomMark />
        <span>headroom<span className="brand-period">.</span></span>
      </a>
      <nav className="primary-nav" aria-label={language === "tr" ? "Ana gezinme" : "Primary navigation"}>
        {screenKey.map((key, index) => <button key={key} disabled={applicationsBusy || pending !== null} className={`nav-item ${screen === key ? "is-active" : ""}`} onClick={() => navigate(key)} aria-current={screen === key ? "page" : undefined}>
          <Icon name={key} className="nav-glyph" /><span>{nav[index]}</span>
        </button>)}
      </nav>
      <div className="sidebar-bottom">
        <div className="local-mark"><Icon name="shield" />{labels.footer}</div>
        <span className="sidebar-note">{language === "tr" ? "Cihazında saklanır." : "Stored on this device."}</span>
        <DesktopLifecycle language={language} blocked={applicationsBusy || pending !== null} />
      </div>
    </aside>

    <main className="main-panel">
      <header className="page-header">
        <div><h1>{headings[screenKey.indexOf(screen)]}</h1><p>{subtitles[screenKey.indexOf(screen)]}</p></div>
        <div className="header-status" aria-live="polite"><Icon name="shield" />{loading ? labels.loading : snapshot ? labels.nativeSnapshot : (language === "tr" ? "Yerel durum" : "Local state")}</div>
      </header>
      <SnapshotNotice readIssue={readIssue} commandError={commandError} engineError={snapshot?.engine.error_code ?? null} onRetry={retrySnapshot} labels={labels} />
      {content}
      {(screen === "overview" || screen === "applications") && <ProblemNotice
        language={language} disabled={applicationsBusy || pending !== null}
        contextKey={`${recoveryRevision}:${recoveryScanGeneration ?? "none"}:${snapshot?.runtime_generation ?? "none"}:${snapshot?.engine.status ?? "unknown"}:${snapshot?.engine.error_code ?? "none"}`}
        onOpenApplications={() => navigate("applications")}
        onOpenOverview={() => navigate("overview")}
      />}
      <footer className="mobile-footer">{labels.footer}</footer>
    </main>
  </div>;
}

function renderSettings(
  labels: Labels,
  theme: PreferenceTheme,
  language: PreferenceLanguage,
  onPreferencesChange: (change: Partial<UserPreferences>) => void,
  preferencesStatus: PreferenceUiStatus,
  preferencesErrorMessage: string | null,
  onPreferencesRetry: () => void,
  zoom: WindowZoomView,
  operating: OperatingPreferences,
  onOperatingSave: (preferences: OperatingPreferences) => Promise<void>,
  onOperatingBusy: (busy: boolean) => void,
) {
  const preferencesBusy = preferencesStatus === "loading" || preferencesStatus === "saving";
  const preferencesUnavailable = preferencesBusy || preferencesStatus === "error";
  return <section className="settings-card">
    <div className="settings-intro"><span className="settings-icon"><Icon name="settings" /></span><div><h2>{labels.settingsTitle}</h2><p>{labels.settingsBody}</p></div></div>
    <label className="setting-row"><span><strong>{labels.theme}</strong><small>{labels.systemTheme}</small></span>
      <select aria-label={labels.theme} disabled={preferencesUnavailable} value={theme} onChange={(event) => onPreferencesChange({ theme: event.target.value as PreferenceTheme })}><option value="system">{labels.system}</option><option value="dark">{labels.dark}</option><option value="light">{labels.light}</option></select>
    </label>
    <label className="setting-row"><span><strong>{labels.language}</strong><small>{labels.interfaceLanguage}</small></span>
      <select aria-label={labels.language} disabled={preferencesUnavailable} value={language} onChange={(event) => onPreferencesChange({ language: event.target.value as PreferenceLanguage })}><option value="tr">{labels.turkish}</option><option value="en">{labels.english}</option></select>
    </label>
    <label className="setting-row"><span><strong>{language === "tr" ? "Görünüm ölçeği" : "View scale"}</strong><small>{language === "tr" ? "Bu pencere için. Sistem ekran ölçeğini değiştirmez." : "For this window. System display scaling stays the same."}</small></span>
      <select aria-label={language === "tr" ? "Görünüm ölçeği" : "View scale"} value={zoom.scale} disabled={zoom.pending || !windowZoomClient.available()}
        onChange={event => zoom.change(Number(event.target.value) as WindowScale)}>
        <option value="1">100%</option><option value="1.25">125%</option><option value="1.5">150%</option>
      </select>
    </label>
    {zoom.failed && <p className="settings-note" role="alert">{language === "tr" ? "Pencere ölçeği değiştirilemedi. Yeniden deneyebilirsin." : "The window scale could not be changed. Try again."}</p>}
    <div className="settings-note" role="status" aria-live="polite">{preferenceStatusLabel(preferencesStatus, labels)}</div>
    {preferencesStatus === "error" && <div className="notice preference-notice" role="alert">
      <span>{preferencesErrorMessage ?? labels.preferencesLoadFailed}</span>
      <button className="text-button" disabled={preferencesBusy} onClick={onPreferencesRetry}>{labels.preferencesRetry}</button>
    </div>}
    <Settings language={language} preferences={operating} status={preferencesStatus} onSave={onOperatingSave} onBusyChange={onOperatingBusy} />
  </section>;
}
