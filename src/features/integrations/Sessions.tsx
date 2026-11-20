import { useEffect, useRef, useState } from "react";
import type { PreferenceLanguage } from "../../lib/preferences";
import { discoverySession, type DiscoverySnapshot } from "../../lib/integrations";
import { sessionClient, type SessionError, type SessionSnapshot } from "../../lib/sessions";
import { Accounts } from "./Accounts";
import { SessionList } from "./SessionList";

// Visibility can change while an IPC read is awaiting its result.
function pageIsHidden(): boolean { return document.visibilityState === "hidden"; }

export function Sessions({ language, onBusyChange, onDiscoveryChange }: {
  language: PreferenceLanguage;
  onBusyChange: (busy: boolean) => void;
  onDiscoveryChange: (generation: number) => void;
}) {
  const [tab, setTab] = useState<"chats" | "accounts">("chats");
  const [snapshot, setSnapshot] = useState<SessionSnapshot | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<SessionError | null>(null);
  const [actionError, setActionError] = useState<SessionError | null>(null);
  const [stale, setStale] = useState(false);
  const [revision, setRevision] = useState(0);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);
  const [opened, setOpened] = useState(false);
  const [discovery, setDiscovery] = useState<DiscoverySnapshot | null>(null);
  const [discoveryPending, setDiscoveryPending] = useState(false);
  const [discoveryFailed, setDiscoveryFailed] = useState(false);
  const [accountsBusy, setAccountsBusy] = useState(false);
  const launchLock = useRef(false);
  const readBlocked = useRef(false);
  readBlocked.current = confirm !== null || opening;
  const alive = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const tr = language === "tr";

  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    onBusyChange(opening || accountsBusy);
    return () => onBusyChange(false);
  }, [opening, accountsBusy, onBusyChange]);

  useEffect(() => {
    if (tab !== "chats") return;
    let disposed = false, reading = false;
    async function read() {
      if (disposed || reading || readBlocked.current || pageIsHidden()) return;
      reading = true;
      setPending(true);
      const result = await sessionClient.read();
      if (disposed) return;
      reading = false;
      setPending(false);
      if (pageIsHidden()) return;
      if (result.kind === "snapshot") {
        setSnapshot(result.snapshot); setError(null); setStale(false);
      } else {
        // Do not leave old live claims visible after a failed observation.
        setSnapshot(null); setStale(true);
        setError(result.kind === "error" ? result.code : "invalid_response");
      }
    }
    function visibility() {
      if (pageIsHidden()) {
        setSnapshot(null); setStale(true);
      } else { void read(); }
    }
    const first = window.setTimeout(() => void read(), 0);
    const interval = window.setInterval(() => void read(), 10_000);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      disposed = true; clearTimeout(first); clearInterval(interval);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [tab, revision]);

  useEffect(() => {
    if (tab !== "accounts") return;
    let disposed = false;
    setDiscoveryPending(true); setDiscoveryFailed(false);
    void discoverySession.loadInitial().then(result => {
      if (disposed) return;
      setDiscoveryPending(false);
      if (result.kind === "snapshot") {
        setDiscovery(result.snapshot); onDiscoveryChange(result.snapshot.scan_generation);
      } else { setDiscovery(null); setDiscoveryFailed(true); }
    });
    return () => { disposed = true; };
  }, [tab, revision, onDiscoveryChange]);

  useEffect(() => {
    const element = dialog.current;
    if (!confirm || !element) return;
    if (!element.open) element.showModal();
    cancel.current?.focus();
    return () => { if (element.open) element.close(); };
  }, [confirm]);

  async function openCenter() {
    if (!confirm || launchLock.current || pageIsHidden()) return;
    launchLock.current = true; setOpening(true); setActionError(null); setOpened(false);
    const result = await sessionClient.openCodex(confirm, true);
    launchLock.current = false;
    if (!alive.current) return;
    setOpening(false); setConfirm(null); setSnapshot(null);
    setRevision(v => v + 1);
    if (result.kind === "launched") setOpened(true);
    else { setStale(true); setActionError(result.kind === "error" ? result.code : "invalid_response"); }
  }

  return <div className="sessions-screen">
    <div className="sessions-tabs" role="group" aria-label={tr ? "Oturum görünümü" : "Session view"}>
      <button type="button" aria-pressed={tab === "chats"} disabled={accountsBusy || opening}
        onClick={() => { if (tab !== "chats") { setTab("chats"); setSnapshot(null); } }}>{tr ? "Sohbetler" : "Chats"}</button>
      <button type="button" aria-pressed={tab === "accounts"} disabled={accountsBusy || opening}
        onClick={() => setTab("accounts")}>{tr ? "Hesaplar" : "Accounts"}</button>
    </div>
    {tab === "chats" ? <>
      {opened && <p className="sessions__notice" role="status">{tr ? "Resmî Codex istemcisi açıldı. Sohbet seçimini orada yapabilirsin; PennyPinch kapanınca istemci açık kalır." : "The official Codex client opened. Choose a chat there; the client stays open when PennyPinch exits."}</p>}
      <SessionList language={language} snapshot={snapshot} pending={pending} error={actionError ?? error} stale={stale}
        opening={opening} onRefresh={() => { setOpened(false); setActionError(null); setRevision(v => v + 1); }}
        onOpenCodex={() => { if (snapshot?.can_open_codex && !pending && !stale) setConfirm(snapshot.snapshot_id); }} />
    </> : <>
      {discoveryPending && <p role="status">{tr ? "Yerel kurulumlar okunuyor…" : "Reading local installations…"}</p>}
      {discoveryFailed && <div className="sessions__notice" role="alert">
        <p>{tr ? "Hesaplar için kurulum bilgisi alınamadı." : "Installation information for accounts is unavailable."}</p>
        <button className="button" onClick={() => setRevision(v => v + 1)}>{tr ? "Yeniden dene" : "Try again"}</button>
      </div>}
      {discovery && <Accounts language={language} installations={discovery.installations}
        scanGeneration={discovery.scan_generation} disabled={discoveryPending} onBusyChange={setAccountsBusy} />}
    </>}
    <dialog ref={dialog} className="accounts-dialog" aria-labelledby="session-open-title" aria-describedby="session-open-body"
      onCancel={event => { if (launchLock.current) event.preventDefault(); else setConfirm(null); }}
      onClose={() => { if (!launchLock.current) setConfirm(null); }}>
      <div className="accounts-dialog__body">
        <h2 id="session-open-title">{tr ? "Codex sohbet merkezini aç?" : "Open the Codex chat center?"}</h2>
        <p id="session-open-body">{tr ? "Geçerli CLI profiline bağlı resmî istemci yeni pencerede açılır. Sohbeti orada sen seçersin. Bu eylem mesaj göndermez; bir sohbetin model, sağlayıcı veya akıl yürütme ayarını değiştirmez." : "The official client for the current CLI profile opens in a new window. Choose the chat there. This action sends no message and does not change a chat's model, provider or reasoning settings."}</p>
        <p>{tr ? "Yeni görev açarsan çalışma klasörünü Codex içinde seç. Bu pencere PennyPinch'ten çıktıktan sonra da açık kalır." : "If you create a new task, choose its working directory in Codex. This window remains open after PennyPinch exits."}</p>
        <div className="accounts-dialog__actions">
          <button ref={cancel} className="button" disabled={opening} onClick={() => setConfirm(null)}>{tr ? "Vazgeç" : "Cancel"}</button>
          <button className="button button-primary" disabled={opening} onClick={() => void openCenter()}>{opening ? (tr ? "Açılıyor…" : "Opening…") : (tr ? "Codex'i aç" : "Open Codex")}</button>
        </div>
      </div>
    </dialog>
  </div>;
}
