import { useEffect, useRef, useState } from "react";
import type { PreferenceLanguage } from "../../lib/preferences";
import { updateClient, type UpdateAction, type UpdateChannel, type UpdateFailure, type UpdateSnapshot } from "../../lib/updates";

const copy = {
  tr: {
    title: "Uygulama güncellemeleri",
    installed: "Kurulu sürüm",
    channel: "Yayın kanalı",
    stable: "Kararlı",
    preview: "Deneme",
    check: "Güncellemeleri kontrol et",
    download: "İndir ve doğrula",
    refresh: "Durumu yeniden oku",
    loading: "Güncelleme durumu okunuyor…",
    checking: "İmzalı sürüm aranıyor…",
    downloading: "Paket indiriliyor ve doğrulanıyor…",
    unavailable: "Bu derleme için güncelleme kaynağı kullanılamıyor.",
    current: "Bu kanalda daha yeni sürüm bulunmadı.",
    available: "İmzalı bir güncelleme bulundu.",
    waiting_for_engine: "Paket doğrulandı. Motorun durması bekleniyor.",
    ready: "Paket doğrulandı ve bu oturum için hazır.",
    exiting: "Uygulama yeniden açılmak üzere kapanıyor…",
    applying: "Güncelleme hazırlanıyor…",
    restoring: "Önceki sürüme dönüş hazırlanıyor…",
    apply: "Güncelle ve yeniden aç",
    restore: "Önceki sürüme dön",
    previous: "Önceki sürüm",
    confirm: "Devam et",
    cancel: "Vazgeç",
    confirmApply: "Headroom kapanacak, güncelleme uygulanacak ve uygulama yeniden açılacak.",
    confirmRestore: "Headroom kapanacak ve önceki uygulama sürümüyle yeniden açılacak. Güncel tercihler ve geçmiş korunur.",
    stopFirst: "Devam etmek için önce Genel bakış’tan motoru durdur.",
    version: "Yeni sürüm",
    size: "İndirme boyutu",
    notes: "Sürüm notları",
    errors: {
      validation_failed: "Güncelleme isteği doğrulanamadı.",
      busy: "Başka bir güncelleme işlemi sürüyor. Durumu yeniden oku.",
      unavailable: "Bu ortamda güncellemeler kullanılamıyor.",
      source_failed: "Güvenilen güncelleme kaynağına erişilemedi.",
      invalid_release: "Sürüm bilgisi veya imza doğrulanamadı.",
      candidate_changed: "Seçili güncelleme değişti. Yeniden kontrol et.",
      storage_failed: "Paket uygulama dizinine yazılamadı.",
      download_failed: "İndirme kesildi. Yeniden deneyebilirsin.",
      integrity_failed: "Paket imzalı sürümle eşleşmedi.",
      preparation_failed: "Güncelleme paketi hazırlanamadı. Boş disk alanını kontrol edip yeniden dene.",
      engine_running: "Motor durduğu doğrulanamadı. Motoru durdurup yeniden dene.",
      invalid_response: "Güncelleme yanıtı doğrulanamadı.",
      request_failed: "Güncelleme durumu alınamadı.",
    },
  },
  en: {
    title: "App updates",
    installed: "Installed version",
    channel: "Release channel",
    stable: "Stable",
    preview: "Preview",
    check: "Check for updates",
    download: "Download and verify",
    refresh: "Refresh status",
    loading: "Reading update status…",
    checking: "Checking for a signed release…",
    downloading: "Downloading and verifying the package…",
    unavailable: "An update source is unavailable for this build.",
    current: "No newer release was found on this channel.",
    available: "A signed update is available.",
    waiting_for_engine: "Package verified. Waiting for the engine to stop.",
    ready: "Package verified and ready for this session.",
    exiting: "The app is closing to restart…",
    applying: "Preparing the update…",
    restoring: "Preparing the previous version…",
    apply: "Update and restart",
    restore: "Restore previous version",
    previous: "Previous version",
    confirm: "Continue",
    cancel: "Cancel",
    confirmApply: "Headroom will close, apply the update, and reopen.",
    confirmRestore: "Headroom will close and reopen the previous app version. Current preferences and history are preserved.",
    stopFirst: "Stop the engine from Overview before continuing.",
    version: "New version",
    size: "Download size",
    notes: "Release notes",
    errors: {
      validation_failed: "The update request could not be validated.",
      busy: "Another update operation is running. Refresh the status.",
      unavailable: "Updates are unavailable in this environment.",
      source_failed: "The trusted update source could not be reached.",
      invalid_release: "The release information or signature could not be verified.",
      candidate_changed: "The selected update changed. Check again.",
      storage_failed: "The package could not be written to the app directory.",
      download_failed: "The download was interrupted. You can retry.",
      integrity_failed: "The package did not match the signed release.",
      preparation_failed: "The update package could not be prepared. Check available disk space and retry.",
      engine_running: "The engine could not be confirmed stopped. Stop the engine and try again.",
      invalid_response: "The update response could not be validated.",
      request_failed: "The update status could not be read.",
    },
  },
} as const;

function formatSize(bytes: number, language: PreferenceLanguage) {
  const format = new Intl.NumberFormat(language === "tr" ? "tr-TR" : "en-US", { maximumFractionDigits: 1 });
  return bytes < 1024 ** 2 ? `${format.format(bytes)} B` : `${format.format(bytes / 1024 ** 2)} MiB`;
}

export function Updates({ language, runtimeKey }: { language: PreferenceLanguage; runtimeKey: string }) {
  const text = copy[language];
  const [channel, setChannel] = useState<UpdateChannel>("stable");
  const [snapshot, setSnapshot] = useState<UpdateSnapshot | null>(null);
  const [readFailure, setReadFailure] = useState<UpdateFailure | null>(null);
  const [actionFailure, setActionFailure] = useState<UpdateFailure | null>(null);
  const [pending, setPending] = useState<"check" | "download" | "apply" | "rollback" | null>(null);
  const [confirmation, setConfirmation] = useState<Extract<UpdateAction, { kind: "apply" | "rollback" }> | null>(null);
  const [refresh, setRefresh] = useState(0);
  const firstRead = useRef(true);
  const inFlight = useRef(false);

  useEffect(() => {
    if (pending) return;
    let alive = true;
    void updateClient.request({ kind: "snapshot" }).then(result => {
      if (!alive || inFlight.current) return;
      if (result.kind === "error") {
        setReadFailure(result.code);
        setSnapshot(null);
      } else {
        if (firstRead.current && result.snapshot.channel) setChannel(result.snapshot.channel);
        firstRead.current = false;
        setReadFailure(null);
        setSnapshot(result.snapshot);
      }
    });
    return () => { alive = false; };
  }, [runtimeKey, pending, refresh]);

  async function run(action: Exclude<UpdateAction, { kind: "snapshot" }>) {
    if (inFlight.current) return;
    inFlight.current = true;
    setPending(action.kind);
    setConfirmation(null);
    setActionFailure(null);
    if (action.kind === "check") setSnapshot(current => current && { ...current, status: "idle", channel: null, candidate: null });
    const result = await updateClient.request(action);
    inFlight.current = false;
    if (result.kind === "error") setActionFailure(result.code);
    else setSnapshot(result.snapshot);
    if (result.kind !== "snapshot" || result.snapshot.status !== "exiting") setPending(null);
  }

  const available = snapshot !== null && snapshot.status !== "unavailable" && snapshot.status !== "exiting";
  const selected = snapshot?.channel === channel ? snapshot : null;
  const candidate = selected?.candidate;
  const status = selected?.status;
  const line = pending === "check" ? text.checking : pending === "download" ? text.downloading
    : snapshot?.status === "exiting" ? text.exiting
    : pending === "apply" ? text.applying : pending === "rollback" ? text.restoring
    : snapshot?.status === "unavailable" || readFailure === "unavailable" ? text.unavailable
      : !snapshot && !readFailure ? text.loading
        : status && status !== "idle" ? text[status] : "";
  const failure = actionFailure ?? (readFailure === "unavailable" ? null : readFailure);

  return <section className="operating-settings" aria-labelledby="updates-title">
    <div className="operating-settings__heading"><h3 id="updates-title">{text.title}</h3>
      {snapshot && <p>{text.installed} {snapshot.installed_version}</p>}
    </div>
    <div className="operating-settings__section operating-settings__section--status">
      {available && <>
        <label className="operating-settings__retention"><span>{text.channel}</span>
          <select value={channel} disabled={pending !== null} onChange={event => {
            setChannel(event.target.value === "preview" ? "preview" : "stable");
            setActionFailure(null);
            setConfirmation(null);
          }}>
            <option value="stable">{text.stable}</option><option value="preview">{text.preview}</option>
          </select>
        </label>
        <button className="button" type="button" disabled={pending !== null}
          onClick={() => void run({ kind: "check", channel })}>{text.check}</button>
      </>}
      {line && <p className="operating-settings__status-line" role="status" aria-live="polite">{line}</p>}
      {failure && <p className="operating-settings__field-error" role="alert">{text.errors[failure]}</p>}
      {readFailure && readFailure !== "unavailable" && <button className="text-button update-refresh" type="button"
        onClick={() => { setReadFailure(null); setActionFailure(null); setRefresh(value => value + 1); }}>{text.refresh}</button>}
      {candidate && <div className="update-candidate">
        <dl className="update-candidate__details">
          <div><dt>{text.version}</dt><dd>{candidate.version}</dd></div>
          <div><dt>{text.size}</dt><dd>{formatSize(candidate.size_bytes, language)}</dd></div>
        </dl>
        {candidate.notes && <div><h4>{text.notes}</h4><p className="update-candidate__notes">{candidate.notes}</p></div>}
        {status === "available" && <button className="button button-primary" type="button" disabled={pending !== null}
          onClick={() => void run({ kind: "download", candidate_id: candidate.id })}>{text.download}</button>}
        {snapshot?.can_apply && <button className="button button-primary" type="button" disabled={pending !== null}
          onClick={() => setConfirmation({ kind: "apply", candidate_id: candidate.id })}>{text.apply}</button>}
      </div>}
      {snapshot?.recovery && <div className="update-candidate">
        <p>{text.previous} {snapshot.recovery.version}</p>
        {!snapshot.recovery.can_restore && <p className="operating-settings__status-line">{text.stopFirst}</p>}
        <button className="button" type="button" disabled={pending !== null || !snapshot.recovery.can_restore}
          onClick={() => setConfirmation({ kind: "rollback", recovery_id: snapshot.recovery!.id })}>{text.restore}</button>
      </div>}
      {confirmation && <div className="update-candidate" role="group" aria-label={confirmation.kind === "apply" ? text.apply : text.restore}>
        <p>{confirmation.kind === "apply" ? text.confirmApply : text.confirmRestore}</p>
        <div className="update-confirmation-actions">
          <button className="button button-primary" type="button" onClick={() => void run(confirmation)}>{text.confirm}</button>
          <button className="button" type="button" onClick={() => setConfirmation(null)}>{text.cancel}</button>
        </div>
      </div>}
    </div>
  </section>;
}
