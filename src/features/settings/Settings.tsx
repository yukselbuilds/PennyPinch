import { useEffect, useMemo, useRef, useState } from "react";
import pennyPinchLogo from "../../assets/pennypinch-logo.png";
import { operatingClient, type OperatingError, type OperatingSnapshot, type OperatingResult } from "../../lib/operatingSettings";
import type { OperatingPreferences, PreferenceLanguage } from "../../lib/preferences";

type PreferenceStatus = "loading" | "loaded" | "saving" | "saved" | "error" | "session_only";
type ReadState = "loading" | "ready" | "error";
type ReadFailure = OperatingError | "invalid_response" | "request_failed" | "unavailable";

const copy = {
  tr: {
    title: "Çalışma tercihleri",
    intro: "Uygulama ve Windows davranışı",
    aboutTitle: "PennyPinch hakkında",
    aboutEngine: "PennyPinch, bağlam sıkıştırması için Apache-2.0 lisanslı Headroom motorunu kullanır. Headroom katkıcılarına teşekkür ederiz.",
    closeTitle: "Pencereyi kapatınca",
    closeTray: "Tepside çalışmayı sürdür",
    closeTrayHelp: "Kapatma düğmesi pencereyi gizler; motor çalışmaya devam eder.",
    confirmExit: "Pencereyi kapatınca onay isteyip çıkış seçeneği sunar.",
    confirmExitHelp: "Pencereyi kapatmak motoru sessizce durdurmaz.",
    retentionTitle: "Yerel ölçüm saklama süresi",
    retentionHelp: "Bu süre uygulamanın yerel ölçüm kayıtları içindir; CCR süresini değiştirmez.",
    days: "gün",
    retentionWarning: "Daha kısa süre kaydedildiğinde sonraki ölçüm okuması eski kayıtları kalıcı olarak temizleyebilir. Süreyi uzatmak silinen kayıtları geri getirmez.",
    retentionConfirm: "Bu kalıcı temizleme uyarısını anladım",
    toolsTitle: "Korunan araç adları",
    toolsHelp: "Liste sonraki elle motor Başlat'ta uygulanır; otomatik yeniden başlatma olmaz. Aynı motor rotasını kullanan desteklenen istemciler ve ajanlar etkilenebilir. Rota dışı trafik ve araç adı taşımayan çağrılar için garanti yoktur.",
    toolsPlaceholder: "Örnek biçim: tool_name\nanother-tool",
    toolsInvalid: "En fazla 32 benzersiz araç adı gir. Adlar 1–96 ASCII harf, rakam, nokta, alt çizgi, iki nokta, eğik çizgi veya tire içerebilir.",
    advanced: "Gelişmiş araç koruma ayarları",
    save: "Tercihleri kaydet",
    saving: "Kaydediliyor…",
    saved: "Tercihler kaydedildi.",
    changed: "Kaydedilmemiş değişiklikler var.",
    sessionOnly: "Masaüstü kaydı kullanılamıyor; bu tercihler kaydedilemez.",
    preferencesLoading: "Tercihler yükleniyor…",
    preferencesError: "Önce kayıtlı tercihleri yeniden oku.",
    startupTitle: "Windows başlangıcı",
    startupHelp: "Oturum açınca PennyPinch penceresi açılır. Motoru sen başlatırsın; Windows’un başlangıç tercihleri geçerlidir.",
    startupRegistered: "Başlangıç kaydı etkin.",
    startupDisabled: "Başlangıç kaydı kapalı.",
    startupConflict: "Başlangıç kaydı beklenenden farklı; değiştirilmedi.",
    startupUnavailable: "Windows başlangıç kaydı bu ortamda kullanılamıyor.",
    startupEnable: "Windows başlangıcında aç",
    startupDisable: "Windows başlangıcından kaldır",
    startupPending: "Windows ayarı güncelleniyor…",
    startupReadLoading: "Windows başlangıç durumu okunuyor…",
    refresh: "Durumu yeniden oku",
    runtimeTitle: "Çalışan motor ayarları",
    runtimeLoading: "Motor ayarları okunuyor…",
    runtimeUnavailable: "Motor ayarları doğrulanamadı.",
    runtimeVerified: "Motor başlatılırken bu ayarlar doğrulandı.",
    runtimeInactive: "Motor çalışmıyor; etkin ayarlar doğrulanamıyor.",
    runtimeUnknown: "Etkin motor ayarları bilinmiyor.",
    runtimeMismatch: "Korunan araç listesi kaydedilen tercihten farklı. Değişiklik sonraki elle Başlat'ta uygulanır.",
    runtimeMatch: "Korunan araç listesi kaydedilen tercihle eşleşiyor.",
    statsOn: "Yerel istatistikler açık",
    beaconOff: "Anonim kullanım paylaşımı kapalı",
    pinned: "Coding/cache profili ve CCR 7200 saniye",
    toolsApplied: "Motorda doğrulanan araç listesi",
    noTools: "Ek araç koruması yok",
    startupErrors: {
      validation_failed: "Windows başlangıç isteği doğrulanamadı.",
      environment_blocked: "Windows başlangıç kaydı bu ortamda kullanılamıyor.",
      busy: "Windows başlangıç ayarı meşgul. Biraz sonra yeniden dene.",
      config_conflict: "Windows başlangıç kaydı değişti. Yeniden oku ve tekrar dene.",
    },
    readErrors: {
      validation_failed: "Ayar okuma isteği doğrulanamadı.",
      environment_blocked: "Ayar kaynağı bu ortamda kullanılamıyor.",
      busy: "Ayar kaynağı meşgul. Yeniden dene.",
      config_conflict: "Ayar kaynağı değişti. Yeniden oku.",
      invalid_response: "Ayar yanıtı doğrulanamadı.",
      request_failed: "Ayarlar okunamadı.",
      unavailable: "Masaüstü ayarları kullanılamıyor.",
    },
  },
  en: {
    title: "Operating preferences",
    aboutTitle: "About PennyPinch",
    aboutEngine: "PennyPinch uses the Apache-2.0 licensed Headroom engine for context compression. Thanks to the Headroom contributors.",
    intro: "Application and Windows behavior",
    closeTitle: "When closing the window",
    closeTray: "Keep running in the tray",
    closeTrayHelp: "The close button hides the window while the engine keeps running.",
    confirmExit: "Closing the window asks before offering to exit.",
    confirmExitHelp: "Closing the window does not silently stop the engine.",
    retentionTitle: "Local measurement retention",
    retentionHelp: "This applies to local measurement records; it does not change CCR retention.",
    days: "days",
    retentionWarning: "After saving a shorter period, a later measurement read may permanently remove older records. Extending retention cannot restore deleted records.",
    retentionConfirm: "I understand this permanent cleanup warning",
    toolsTitle: "Protected tool names",
    toolsHelp: "The list is applied on the next manual engine start; there is no automatic restart. Supported clients and agents using the same engine route may be affected. No guarantee applies to traffic outside that route or calls without a tool name.",
    toolsPlaceholder: "Name format: tool_name\nanother-tool",
    toolsInvalid: "Enter up to 32 unique tool names. Names may contain 1–96 ASCII letters, digits, periods, underscores, colons, slashes, or hyphens.",
    advanced: "Advanced tool protection",
    save: "Save preferences",
    saving: "Saving…",
    saved: "Preferences saved.",
    changed: "You have unsaved changes.",
    sessionOnly: "Desktop storage is unavailable; these preferences cannot be saved.",
    preferencesLoading: "Loading preferences…",
    preferencesError: "Reload the saved preferences before editing.",
    startupTitle: "Windows startup",
    startupHelp: "Opens PennyPinch when you sign in. Start the engine when you need it. Windows startup preferences still apply.",
    startupRegistered: "Startup entry is enabled.",
    startupDisabled: "Startup entry is off.",
    startupConflict: "The startup entry differs from the expected value; it was not changed.",
    startupUnavailable: "Windows startup registration is unavailable in this environment.",
    startupEnable: "Open at Windows startup",
    startupDisable: "Remove from Windows startup",
    startupPending: "Updating Windows setting…",
    startupReadLoading: "Reading Windows startup status…",
    refresh: "Refresh status",
    runtimeTitle: "Running engine settings",
    runtimeLoading: "Reading engine settings…",
    runtimeUnavailable: "Engine settings could not be verified.",
    runtimeVerified: "These settings were verified when the engine started.",
    runtimeInactive: "The engine is not running; active settings cannot be verified.",
    runtimeUnknown: "Active engine settings are unknown.",
    runtimeMismatch: "The protected tool list differs from the saved preference. It applies on the next manual start.",
    runtimeMatch: "The protected tool list matches the saved preference.",
    statsOn: "Local statistics enabled",
    beaconOff: "Anonymous usage sharing disabled",
    pinned: "Coding/cache profile and CCR 7200 seconds",
    toolsApplied: "Tool list verified on the engine",
    noTools: "No additional tools protected",
    startupErrors: {
      validation_failed: "The Windows startup request could not be validated.",
      environment_blocked: "Windows startup registration is unavailable in this environment.",
      busy: "The Windows startup setting is busy. Try again shortly.",
      config_conflict: "The Windows startup entry changed. Refresh and try again.",
    },
    readErrors: {
      validation_failed: "The settings read request could not be validated.",
      environment_blocked: "The settings source is unavailable in this environment.",
      busy: "The settings source is busy. Try again.",
      config_conflict: "The settings source changed. Refresh it.",
      invalid_response: "The settings response could not be validated.",
      request_failed: "Settings could not be read.",
      unavailable: "Desktop settings are unavailable.",
    },
  },
} as const;

type Copy = (typeof copy)["en"];

function parseToolNames(input: string): { value: string[] | null; error: boolean } {
  if (input.length > 4096) return { value: null, error: true };
  const names = input.split(/[\n,]/).map(name => name.trim()).filter(Boolean);
  if (names.some(name => !/^[a-zA-Z0-9_.:/-]{1,96}$/.test(name))) return { value: null, error: true };
  const unique = [...new Set(names)].sort();
  return unique.length > 32 ? { value: null, error: true } : { value: unique, error: false };
}

function sameTools(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((tool, index) => tool === right[index]);
}

function readFailureOf(result: OperatingResult): ReadFailure | null {
  if (result.kind === "error") return result.code;
  if (result.kind === "unavailable") return "unavailable";
  if (result.kind === "invalid_response" || result.kind === "request_failed") return result.kind;
  return result.kind === "snapshot" ? null : "request_failed";
}

export function Settings({ language, preferences, status, onSave, onBusyChange }: {
  language: PreferenceLanguage;
  preferences: OperatingPreferences;
  status: PreferenceStatus;
  onSave: (value: OperatingPreferences) => Promise<void>;
  onBusyChange: (busy: boolean) => void;
}) {
  const text = copy[language] as Copy;
  const native = operatingClient.available();
  const savedToolsKey = preferences.protected_tools.join("\n");
  const [draft, setDraft] = useState<OperatingPreferences>(preferences);
  const [toolText, setToolText] = useState(preferences.protected_tools.join("\n"));
  const [retentionConfirmed, setRetentionConfirmed] = useState(false);
  const [savePending, setSavePending] = useState(false);
  const [startupPending, setStartupPending] = useState(false);
  const [snapshot, setSnapshot] = useState<OperatingSnapshot | null>(null);
  const [readState, setReadState] = useState<ReadState>(native ? "loading" : "error");
  const [readFailure, setReadFailure] = useState<ReadFailure | null>(native ? null : "unavailable");
  const [startupFailure, setStartupFailure] = useState<OperatingError | null>(null);
  const readSequence = useRef(0);
  const mutationLock = useRef(false);
  const mounted = useRef(false);
  const refreshRef = useRef<() => void>(() => {});
  const parsedTools = useMemo(() => parseToolNames(toolText), [toolText]);
  const hasChanges = draft.close_to_tray !== preferences.close_to_tray
    || draft.retention_days !== preferences.retention_days
    || !sameTools(draft.protected_tools, preferences.protected_tools);
  const preferenceReady = status === "loaded" || status === "saved";
  const retentionShortened = draft.retention_days < preferences.retention_days;
  const canSave = native && preferenceReady && hasChanges && !savePending && !startupPending
    && !mutationLock.current && parsedTools.value !== null && (!retentionShortened || retentionConfirmed);
  const autostart = readState === "ready" && snapshot ? snapshot.autostart : null;
  const runtime = readState === "ready" && snapshot ? snapshot.runtime : null;
  const canSetStartup = native && autostart !== null && autostart.status !== "conflict"
    && autostart.status !== "unavailable" && !savePending && !startupPending
    && status !== "loading" && status !== "saving"
    && !mutationLock.current;

  useEffect(() => {
    setDraft(preferences);
    setToolText(savedToolsKey);
    setRetentionConfirmed(false);
  }, [preferences.close_to_tray, preferences.retention_days, savedToolsKey]);

  useEffect(() => {
    mounted.current = true;
    if (!native) return () => { mounted.current = false; };
    let active = true;
    let inFlight = false;
    const refresh = async () => {
      if (!active || inFlight || mutationLock.current) return;
      inFlight = true;
      const sequence = ++readSequence.current;
      setReadState(current => current === "ready" ? "ready" : "loading");
      const result = await operatingClient.read();
      if (active && mounted.current && sequence === readSequence.current) {
        if (result.kind === "snapshot") {
          setSnapshot(result.snapshot);
          setReadFailure(null);
          setReadState("ready");
        } else {
          setSnapshot(null);
          setReadFailure(readFailureOf(result));
          setReadState("error");
        }
      }
      inFlight = false;
    };
    refreshRef.current = () => { void refresh(); };
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, 5000);
    return () => {
      active = false;
      mounted.current = false;
      readSequence.current += 1;
      window.clearInterval(timer);
      refreshRef.current = () => {};
      if (mutationLock.current) onBusyChange(false);
    };
  }, [native, onBusyChange]);

  function updateToolText(value: string) {
    setToolText(value);
    const parsed = parseToolNames(value);
    setDraft(current => ({ ...current, protected_tools: parsed.value ?? current.protected_tools }));
  }

  async function savePreferences() {
    if (!canSave || !parsedTools.value || mutationLock.current) return;
    mutationLock.current = true;
    setSavePending(true);
    onBusyChange(true);
    try {
      await onSave({ ...draft, protected_tools: parsedTools.value });
    } catch {
      // Parent reports the fixed persistence status and owns the saved value.
    } finally {
      mutationLock.current = false;
      if (mounted.current) {
        setSavePending(false);
        refreshRef.current();
      }
      onBusyChange(false);
    }
  }

  function refreshFromUser() {
    setStartupFailure(null);
    refreshRef.current();
  }

  async function toggleAutostart() {
    const current = snapshot?.autostart;
    if (!canSetStartup || !current || mutationLock.current) return;
    mutationLock.current = true;
    readSequence.current += 1;
    setStartupPending(true);
    setStartupFailure(null);
    onBusyChange(true);
    try {
      const result = await operatingClient.setAutostart(current.status !== "registered", current.fingerprint);
      if (!mounted.current) return;
      if (result.kind === "snapshot") {
        setSnapshot(result.snapshot);
        setReadFailure(null);
        setReadState("ready");
      } else {
        setStartupFailure(result.kind === "error" ? result.code : null);
        setSnapshot(null);
        setReadFailure(readFailureOf(result) ?? "request_failed");
        setReadState("error");
      }
    } finally {
      mutationLock.current = false;
      if (mounted.current) {
        setStartupPending(false);
        refreshRef.current();
      }
      onBusyChange(false);
    }
  }

  const toolMismatch = runtime?.status === "verified"
    ? !sameTools(runtime.protected_tools ?? [], preferences.protected_tools)
    : false;
  const retryStartup = readFailure !== null || autostart?.status === "conflict" || autostart?.status === "unavailable";
  const startupLine = !autostart ? readState === "loading" ? text.startupReadLoading : text.startupUnavailable
    : autostart.status === "registered" ? text.startupRegistered
      : autostart.status === "disabled" ? text.startupDisabled
        : autostart.status === "conflict" ? text.startupConflict : text.startupUnavailable;

  return <section className="operating-settings" aria-labelledby="operating-settings-title">
    <header className="operating-settings__heading">
      <div><h3 id="operating-settings-title">{text.title}</h3><p>{text.intro}</p></div>
    </header>

    <div className="operating-settings__section">
      <h4>{text.closeTitle}</h4>
      <label className="operating-settings__toggle">
        <span><strong>{text.closeTray}</strong>
          <small>{draft.close_to_tray ? text.closeTrayHelp : text.confirmExit}</small></span>
        <input type="checkbox" checked={draft.close_to_tray} disabled={!preferenceReady || savePending || startupPending}
          onChange={event => setDraft(current => ({ ...current, close_to_tray: event.target.checked }))} />
      </label>
    </div>

    <div className="operating-settings__section">
      <label className="operating-settings__retention" htmlFor="operating-retention">
        <span><strong>{text.retentionTitle}</strong><small>{text.retentionHelp}</small></span>
        <select id="operating-retention" value={draft.retention_days} disabled={!preferenceReady || savePending || startupPending}
          onChange={event => { const value = Number(event.target.value) as 7 | 30 | 90; setDraft(current => ({ ...current, retention_days: value })); setRetentionConfirmed(false); }}>
          {[7, 30, 90].map(days => <option key={days} value={days}>{days} {text.days}</option>)}
        </select>
      </label>
      {retentionShortened && <div className="operating-settings__warning">
        <p>{text.retentionWarning}</p>
        <label><input type="checkbox" checked={retentionConfirmed} disabled={savePending}
          onChange={event => setRetentionConfirmed(event.target.checked)} />{text.retentionConfirm}</label>
      </div>}
    </div>

    <details className="operating-settings__advanced">
      <summary>{text.advanced}</summary>
      <div className="operating-settings__advanced-body">
        <label className="operating-settings__tools" htmlFor="protected-tools">
          <span><strong>{text.toolsTitle}</strong><small>{text.toolsHelp}</small></span>
          <textarea id="protected-tools" rows={4} maxLength={4096} value={toolText} disabled={!preferenceReady || savePending || startupPending}
            placeholder={text.toolsPlaceholder} aria-invalid={parsedTools.error}
            onChange={event => updateToolText(event.target.value)} />
        </label>
        {parsedTools.error && <p className="operating-settings__field-error" role="alert">{text.toolsInvalid}</p>}
      </div>
    </details>

    <div className="operating-settings__save-row">
      <div className="operating-settings__status" role="status" aria-live="polite">
        {status === "saving" || savePending ? text.saving
          : hasChanges ? text.changed
            : status === "saved" ? text.saved
            : status === "session_only" ? text.sessionOnly
              : status === "loading" ? text.preferencesLoading
                : status === "error" ? text.preferencesError
                  : ""}
      </div>
      <button className="button button-primary" type="button" disabled={!canSave} onClick={() => void savePreferences()}>
        {savePending ? text.saving : text.save}
      </button>
    </div>

    <div className="operating-settings__section operating-settings__section--status">
      <div className="operating-settings__section-heading"><h4>{text.startupTitle}</h4>
        {retryStartup && <button className="text-button" type="button" disabled={startupPending} onClick={refreshFromUser}>{text.refresh}</button>}
      </div>
      <p className="operating-settings__help">{text.startupHelp}</p>
      <div className="operating-settings__status-line" role="status" aria-live="polite">
        {startupPending ? text.startupPending : readFailure ? text.readErrors[readFailure] : startupFailure ? text.startupErrors[startupFailure] : startupLine}
      </div>
      <button className="button" type="button" disabled={!canSetStartup || startupPending}
        onClick={() => void toggleAutostart()}>
        {startupPending ? text.startupPending : autostart?.status === "registered" ? text.startupDisable : text.startupEnable}
      </button>
    </div>

    <div className="operating-settings__section operating-settings__section--runtime">
      <div className="operating-settings__section-heading"><h4>{text.runtimeTitle}</h4>
        {readFailure && <button className="text-button" type="button" disabled={startupPending} onClick={refreshFromUser}>{text.refresh}</button>}
      </div>
      <p className="operating-settings__status-line" role="status" aria-live="polite">
        {readState === "loading" ? text.runtimeLoading
          : readFailure ? text.readErrors[readFailure]
            : runtime?.status === "verified" ? text.runtimeVerified
              : runtime?.status === "inactive" ? text.runtimeInactive : text.runtimeUnknown}
      </p>
      {runtime?.status === "verified" && <>
        <p className="operating-settings__pins">{text.statsOn} · {text.beaconOff} · {text.pinned}</p>
        <p className="operating-settings__tools-applied">{text.toolsApplied}: {runtime.protected_tools?.length ? runtime.protected_tools.join(", ") : text.noTools}</p>
        {toolMismatch && <p className="operating-settings__warning-note" role="status">{text.runtimeMismatch}</p>}
        {!toolMismatch && <p className="operating-settings__help">{text.runtimeMatch}</p>}
      </>}
    </div>
    <div className="operating-settings__section operating-settings__about">
      <img src={pennyPinchLogo} width={88} height={88} loading="lazy" alt="" />
      <div>
        <h4>{text.aboutTitle}</h4>
        <p className="operating-settings__help">{text.aboutEngine}</p>
      </div>
    </div>
  </section>;
}
