import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  desktopClient,
  listenForDesktopChanges,
  type DesktopErrorCode,
  type DesktopResult,
  type DesktopSnapshot,
} from "../lib/desktop-events";
import type { PreferenceLanguage } from "../lib/preferences";

type Failure = DesktopErrorCode | Exclude<DesktopResult["kind"], "snapshot" | "desktop_error">;
type PendingAction = "request" | "cancel" | "confirm";

const copy = {
  tr: {
    trayAvailable: "X tepside çalışmayı sürdürür.",
    trayUnavailable: "Tepsi kullanılamıyor; pencere kapanışı çıkış onayı ister.",
    confirmOnClose: "Pencereyi kapatırken çıkış onayı sorulur.",
    trayUnknown: "Tepsi durumu bilinmiyor.",
    exit: "Headroom’dan çık",
    title: "Headroom’dan çıkılsın mı?",
    engine: "Bu işlem motoru ve tepsi simgesini kapatır.",
    routing: "Kayıtlı yönlendirmeler otomatik geri alınmaz.",
    clients: "Bağlı uygulamalar hata alabilir.",
    streams: "Süren yanıtlar kesilebilir.",
    closeWindow: "Tepsi kullanılabiliyorsa pencereyi kapatmak Headroom’u tepside tutar.",
    blocked: "Devam eden işlemin bitmesini bekle.",
    cancel: "Vazgeç",
    confirm: "Çıkışı onayla",
    confirming: "Çıkılıyor…",
    refresh: "Durumu yenile",
    errors: {
      validation_failed: "Çıkış isteği doğrulanamadı.",
      state_conflict: "Çıkış durumu değişti. Durum yeniden okunmalı.",
      busy: "Başka bir işlem sürüyor. Biraz sonra yeniden dene.",
      client_busy: "Çıkış isteği zaten işleniyor.",
      invalid_request: "Çıkış isteği geçersiz.",
      invalid_response: "Masaüstü çıkış yanıtı doğrulanamadı.",
      request_failed: "Masaüstü durumu okunamadı veya çıkış isteği tamamlanamadı.",
      desktop_unavailable: "Masaüstü çıkış denetimi kullanılamıyor.",
    },
  },
  en: {
    trayAvailable: "Closing the window keeps Headroom running in the tray.",
    trayUnavailable: "The tray is unavailable; closing the window asks before exiting.",
    confirmOnClose: "Closing the window asks before exiting.",
    trayUnknown: "Tray status is unknown.",
    exit: "Exit Headroom",
    title: "Exit Headroom?",
    engine: "This closes the engine and tray icon.",
    routing: "Saved routing entries will not be reverted automatically.",
    clients: "Connected apps may receive errors.",
    streams: "In-flight responses may be interrupted.",
    closeWindow: "Closing the window keeps Headroom in the tray when tray support is available.",
    blocked: "Wait for the current operation to finish.",
    cancel: "Cancel",
    confirm: "Confirm exit",
    confirming: "Exiting…",
    refresh: "Refresh status",
    errors: {
      validation_failed: "The exit request could not be validated.",
      state_conflict: "The exit state changed. Refresh the status.",
      busy: "Another operation is in progress. Try again shortly.",
      client_busy: "An exit request is already being processed.",
      invalid_request: "The exit request is invalid.",
      invalid_response: "The desktop exit response could not be validated.",
      request_failed: "Desktop status or the exit request could not be completed.",
      desktop_unavailable: "Desktop exit control is unavailable.",
    },
  },
} as const;

function failureOf(result: DesktopResult): Failure | null {
  if (result.kind === "desktop_error") return result.error.error.code;
  return result.kind === "snapshot" ? null : result.kind;
}

export function DesktopLifecycle({ language, blocked }: { language: PreferenceLanguage; blocked: boolean }) {
  const native = desktopClient.available();
  const text = copy[language];
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [fresh, setFresh] = useState(false);
  const [loading, setLoading] = useState(true);
  const [readFailure, setReadFailure] = useState<Failure | null>(null);
  const [actionFailure, setActionFailure] = useState<Failure | null>(null);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const exitButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const disposed = useRef(false);
  const mutationLock = useRef(false);
  const refreshQueued = useRef(false);
  const readSequence = useRef(0);
  const mutationSequence = useRef(0);

  const refresh = useCallback(async () => {
    if (!native || disposed.current) return;
    if (mutationLock.current) {
      refreshQueued.current = true;
      return;
    }
    const sequence = ++readSequence.current;
    setLoading(true);
    const result = await desktopClient.getState();
    if (disposed.current || sequence !== readSequence.current) return;
    if (result.kind === "snapshot") {
      setSnapshot(result.snapshot);
      setFresh(true);
      setReadFailure(null);
    } else {
      setFresh(false);
      setReadFailure(failureOf(result));
    }
    setLoading(false);
  }, [native]);

  useEffect(() => {
    disposed.current = false;
    if (!native) {
      setLoading(false);
      return () => { disposed.current = true; };
    }
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void (async () => {
      try {
        const stopListening = await listenForDesktopChanges(() => { void refresh(); });
        if (cancelled) {
          stopListening?.();
          return;
        }
        unlisten = stopListening;
        await refresh();
      } catch {
        if (!cancelled) {
          setFresh(false);
          setReadFailure("request_failed");
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
      disposed.current = true;
      readSequence.current += 1;
      mutationSequence.current += 1;
      unlisten?.();
    };
  }, [native, refresh]);

  const dialogOpen = snapshot?.exit_state === "confirmation_required" || snapshot?.exit_state === "shutting_down";
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (dialogOpen && !dialog.open) {
      returnFocusRef.current = document.activeElement instanceof HTMLElement
        ? document.activeElement : exitButtonRef.current;
      dialog.showModal();
      cancelRef.current?.focus();
    } else if (!dialogOpen && dialog.open) {
      dialog.close();
      const target = returnFocusRef.current;
      if (target?.isConnected) target.focus();
      else exitButtonRef.current?.focus();
    }
  }, [dialogOpen]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialogOpen || !dialog) return;
    const keepTabInside = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || document.querySelector("dialog[open][data-desktop-exit-confirmation]") !== dialog) return;
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      )).filter(element => element.tabIndex >= 0 && window.getComputedStyle(element).visibility !== "hidden");
      const first = focusable[0];
      const last = focusable.at(-1);
      if (!first || !last) {
        event.preventDefault();
        dialog.focus();
      } else if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", keepTabInside, true);
    return () => document.removeEventListener("keydown", keepTabInside, true);
  }, [dialogOpen]);

  async function mutate(action: PendingAction) {
    if (mutationLock.current || pending !== null || !native) return;
    if (action === "confirm" && blocked) return;
    const token = snapshot?.exit_token;
    if (action !== "request" && (!fresh || !token || snapshot?.exit_state !== "confirmation_required")) return;
    mutationLock.current = true;
    refreshQueued.current = false;
    readSequence.current += 1;
    const sequence = ++mutationSequence.current;
    setPending(action);
    setActionFailure(null);
    const result = action === "request" ? await desktopClient.requestExit()
      : action === "cancel" ? await desktopClient.cancelExit(token!)
      : await desktopClient.confirmExit(token!);
    if (!disposed.current && sequence === mutationSequence.current) {
      if (result.kind === "snapshot") {
        setSnapshot(result.snapshot);
        setFresh(true);
        setReadFailure(null);
      } else {
        setFresh(false);
        setActionFailure(failureOf(result));
        refreshQueued.current = true;
      }
    }
    mutationLock.current = false;
    if (!disposed.current && sequence === mutationSequence.current) {
      setPending(null);
      if (refreshQueued.current) {
        refreshQueued.current = false;
        void refresh();
      }
    }
  }

  if (!native) return null;
  const error = actionFailure ?? readFailure;
  const stateText = loading && !snapshot ? (language === "tr" ? "Tepsi durumu okunuyor…" : "Reading tray status…")
    : !fresh ? text.trayUnknown
    : snapshot?.tray_available ? snapshot.close_to_tray ? text.trayAvailable : text.confirmOnClose : text.trayUnavailable;
  const confirming = pending !== null || snapshot?.exit_state === "shutting_down";
  const tokenUsable = fresh && snapshot?.exit_state === "confirmation_required" && snapshot.exit_token !== null;
  const canConfirm = tokenUsable && !blocked && pending === null;

  return <>
    <section className="desktop-lifecycle" aria-label={language === "tr" ? "Masaüstü çıkışı" : "Desktop exit"}>
      <p className="desktop-lifecycle__tray" role="status">{stateText}</p>
      {error && <p className="desktop-lifecycle__error" role="status">{text.errors[error]}</p>}
      {!fresh && !loading && <button className="desktop-lifecycle__refresh" type="button" onClick={() => void refresh()}>{text.refresh}</button>}
      <button ref={exitButtonRef} className="desktop-lifecycle__exit" type="button"
        disabled={pending !== null || snapshot?.exit_state === "shutting_down" || snapshot?.exit_state === "confirmation_required"}
        onClick={() => void mutate("request")}>{text.exit}</button>
    </section>
    {createPortal(<dialog ref={dialogRef} className="desktop-exit-dialog" data-desktop-exit-confirmation="true"
      aria-labelledby="desktop-exit-title" aria-describedby="desktop-exit-explanation"
      onCancel={event => {
        event.preventDefault();
        if (!confirming && tokenUsable) void mutate("cancel");
      }}>
      <div className="desktop-exit-dialog__content">
        <span className="desktop-exit-dialog__eyebrow">Headroom</span>
        <h2 id="desktop-exit-title">{text.title}</h2>
        <div id="desktop-exit-explanation" className="desktop-exit-dialog__copy">
          <p>{text.engine}</p><p>{text.routing}</p><p>{text.clients}</p><p>{text.streams}</p><p>{snapshot?.tray_available && snapshot.close_to_tray ? text.closeWindow : text.confirmOnClose}</p>
        </div>
        {blocked && <p className="desktop-exit-dialog__blocked" role="status">{text.blocked}</p>}
        {error && <p className="desktop-exit-dialog__error" role="alert">{text.errors[error]}</p>}
        {!fresh && <button className="text-button desktop-exit-dialog__refresh" type="button" disabled={pending !== null} onClick={() => void refresh()}>{text.refresh}</button>}
        <div className="desktop-exit-dialog__actions">
          <button ref={cancelRef} className="button" type="button" disabled={!tokenUsable || confirming || pending !== null}
            onClick={() => void mutate("cancel")}>{text.cancel}</button>
          <button className="button desktop-exit-dialog__confirm" type="button" disabled={!canConfirm || confirming}
            onClick={() => void mutate("confirm")}>{confirming ? text.confirming : text.confirm}</button>
        </div>
      </div>
    </dialog>, document.body)}
  </>;
}
