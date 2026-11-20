import { useEffect, useRef } from "react";
import type { ConnectionScope } from "../../lib/contracts";
import type { PreferenceLanguage } from "../../lib/preferences";

const copy = {
  tr: {
    title: "Bağlantıyı kaldır?",
    userScope: "Kapsam: kullanıcı ayarları",
    projectScope: "Kapsam: seçilen proje",
    effect: "Yalnız PennyPinch'in bu kapsamda eklediği yönlendirme kaydı kaldırılır. Motor ve diğer istemcilerin bağlantıları açık kalır.",
    restart: "Açık istemcinin güncellenen ayarı kullanması için yeniden açılması gerekebilir.",
    cancel: "Vazgeç",
    remove: "Bağlantıyı kaldır",
    removing: "Bağlantı kaldırılıyor…",
  },
  en: {
    title: "Remove this connection?",
    userScope: "Scope: user settings",
    projectScope: "Scope: selected project",
    effect: "Only the routing entry PennyPinch added in this scope will be removed. The engine and other client connections will stay running.",
    restart: "An open client may need to be restarted to load its updated setting.",
    cancel: "Cancel",
    remove: "Remove connection",
    removing: "Removing connection…",
  },
} as const;

export function DisconnectDialog({
  open,
  pending,
  language,
  clientName,
  scope,
  restoreFocusRef,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  pending: boolean;
  language: PreferenceLanguage;
  clientName: string;
  scope: ConnectionScope;
  restoreFocusRef: { readonly current: HTMLElement | null };
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const text = copy[language];
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!open || !dialog) return;
    returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!dialog.open) dialog.showModal();
    cancelRef.current?.focus();
    return () => {
      if (dialog.open) dialog.close();
      if (returnFocus.current?.isConnected) returnFocus.current.focus();
      else restoreFocusRef.current?.focus();
    };
  }, [open, restoreFocusRef]);

  useEffect(() => {
    if (open && pending) dialogRef.current?.focus();
  }, [open, pending]);

  return <dialog
    ref={dialogRef}
    id="disconnect-dialog"
    className="disconnect-dialog"
    tabIndex={-1}
    aria-labelledby="disconnect-dialog-title"
    aria-describedby="disconnect-dialog-effect disconnect-dialog-restart"
    onCancel={(event) => { event.preventDefault(); event.stopPropagation(); if (!pending) onCancel(); }}
    onKeyDown={(event) => {
      if (event.key !== "Tab") return;
      const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not([disabled])"));
      if (buttons.length === 0) { event.preventDefault(); event.currentTarget.focus(); return; }
      const first = buttons[0], last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }}
  >
    <div className="disconnect-dialog__body">
      <div className="disconnect-dialog__eyebrow">{clientName}</div>
      <h2 id="disconnect-dialog-title">{text.title}</h2>
      <p className="disconnect-dialog__scope">{scope === "project_local_config" ? text.projectScope : text.userScope}</p>
      <p id="disconnect-dialog-effect" className="disconnect-dialog__copy">{text.effect}</p>
      <p id="disconnect-dialog-restart" className="disconnect-dialog__copy">{text.restart}</p>
      {pending && <p className="disconnect-dialog__pending" role="status" aria-live="polite">{text.removing}</p>}
      <div className="disconnect-dialog__actions">
        <button ref={cancelRef} className="button" type="button" disabled={pending} onClick={onCancel}>{text.cancel}</button>
        <button className="button disconnect-dialog__confirm" type="button" disabled={pending} onClick={onConfirm}>
          {pending ? text.removing : text.remove}
        </button>
      </div>
    </div>
  </dialog>;
}
