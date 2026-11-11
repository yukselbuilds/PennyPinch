import { useCallback, useEffect, useRef, useState } from "react";
import {
  retainedVersionsClient,
  type RetainedVersionsResult,
  type RetainedVersionsSnapshot,
  type RetainedClientKind,
  type RetainedScope,
} from "../../lib/retainedVersions";
import "./retained-versions.css";

const copy = {
  tr: {
    title: "Korunan sürümler",
    loading: "Korunan sürümler okunuyor…",
    refresh: "Yenile",
    refreshing: "Yenileniyor…",
    selected: "Bu kapsamda seçilen işlem",
    active: "Etkin kayıt",
    remaining: "Arşiv kapasitesi",
    changed: (count: number) => `${count} kayıt, sonradan yapılan ayar değişikliğini içeriyor; özgün sürüm ayrıca korunur.`,
    unchanged: "Mevcut ayar değişmez. Önceki sürümler silinmeden saklanır. Bu seçimden sonra işlemler mevcut ayarla devam eder.",
    confirm: "Mevcut ayarı kullanmak istiyorum; korunan sürümler saklansın.",
    archive: "Mevcut ayarla devam et",
    archiving: "Sürümler korunuyor…",
    archived: "Sürümler korundu. İşlemler mevcut ayarla devam ediyor.",
    unresolved: "Bazı kayıtlar güvenle eşleştirilemedi; işlem yapılamaz.",
    noSelection: "Bu kapsamda saklanacak eski sürüm yok.",
    full: "Korunan kayıt sınırına ulaşıldı. Yeni işlem için saklanan kayıtları inceleyin.",
    errorTitle: "Sürümler okunamadı",
    errors: {
      invalid_input: "İstek doğrulanamadı.",
      scope_changed: "Ayar kapsamı değişti. Yenileyip tekrar deneyin.",
      busy: "Başka bir ayar işlemi sürüyor. Biraz sonra yenileyin.",
      conflict: "Ayarlar değişti. Güncel durumu yenileyin.",
      unavailable: "Bu işlem şu anda kullanılamıyor.",
      archive_limit: "Korunan kayıt sınırına ulaşıldı. Saklanan kayıtları inceleyin.",
      commit_unknown: "İşlemin sonucu doğrulanamadı. Güncel durumu yenileyin.",
      invalid_response: "Uygulamadan güvenli yanıt alınamadı. Yeniden deneyin.",
    },
  },
  en: {
    title: "Retained versions",
    loading: "Reading retained versions…",
    refresh: "Refresh",
    refreshing: "Refreshing…",
    selected: "Operations selected in this scope",
    active: "Active records",
    remaining: "Archive capacity",
    changed: (count: number) => `${count} record includes a later settings edit; the original version is retained separately.`,
    unchanged: "The current setting stays unchanged. Earlier versions are kept without deleting them. Operations continue with the current setting after this choice.",
    confirm: "I want to keep using the current setting and retain these versions.",
    archive: "Continue with current setting",
    archiving: "Retaining versions…",
    archived: "Versions retained. Operations continue with the current setting.",
    unresolved: "Some records could not be matched safely, so this action is unavailable.",
    noSelection: "There are no earlier versions to retain in this scope.",
    full: "The retained record limit has been reached. Review retained records before starting another operation.",
    errorTitle: "Versions could not be read",
    errors: {
      invalid_input: "The request could not be validated.",
      scope_changed: "The settings scope changed. Refresh before trying again.",
      busy: "Another settings operation is running. Refresh in a moment.",
      conflict: "Settings changed. Refresh the current state.",
      unavailable: "This action is unavailable right now.",
      archive_limit: "The retained record limit has been reached. Review retained records.",
      commit_unknown: "The result could not be confirmed. Refresh the current state.",
      invalid_response: "A safe response was not received. Try again.",
    },
  },
} as const;

type Language = keyof typeof copy;
type ViewResult = RetainedVersionsResult | null;

function shouldShow(snapshot: RetainedVersionsSnapshot): boolean {
  return snapshot.supported && (snapshot.selected_records > 0 || snapshot.active_records >= snapshot.record_limit);
}

export function RetainedVersions({
  clientKind,
  scope,
  language,
  disabled = false,
  client = retainedVersionsClient,
  onArchived,
  onBusyChange,
}: {
  clientKind: RetainedClientKind;
  scope: RetainedScope;
  language: Language;
  disabled?: boolean;
  client?: ReturnType<typeof import("../../lib/retainedVersions").createRetainedVersionsClient>;
  onArchived: () => void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const text = copy[language];
  const [result, setResult] = useState<ViewResult>(null);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [archived, setArchived] = useState(false);
  const mounted = useRef(false);
  const sequence = useRef(0);
  const actionLock = useRef(false);
  const readLock = useRef(false);
  const queuedRead = useRef(false);
  const readRef = useRef<() => Promise<void>>(async () => {});
  const currentContext = useRef("");
  const clientRef = useRef(client);
  const onArchivedRef = useRef(onArchived);
  const onBusyChangeRef = useRef(onBusyChange);
  const contextKey = `${clientKind}:${scope}`;
  clientRef.current = client;
  onArchivedRef.current = onArchived;
  onBusyChangeRef.current = onBusyChange;

  const read = useCallback(async () => {
    if (actionLock.current || readLock.current) {
      queuedRead.current = true;
      setLoading(true);
      return;
    }
    readLock.current = true;
    const expectedContext = contextKey;
    const readSequence = ++sequence.current;
    currentContext.current = expectedContext;
    setLoading(true);
    setArchived(false);
    setConfirmed(false);
    try {
      const next = await clientRef.current.read(clientKind, scope);
      if (!mounted.current || currentContext.current !== expectedContext || sequence.current !== readSequence) return;
      setResult(next);
    } catch {
      if (mounted.current && currentContext.current === expectedContext && sequence.current === readSequence) {
        setResult({ kind: "error", data: {
          code: "unavailable",
          retryable: true,
          message_key: "retained.unavailable",
          diagnostic_id: null,
        } });
      }
    } finally {
      readLock.current = false;
      if (mounted.current && currentContext.current === expectedContext && sequence.current === readSequence) {
        setLoading(false);
      }
      if (queuedRead.current && mounted.current) {
        queuedRead.current = false;
        void readRef.current();
      }
    }
  }, [clientKind, contextKey, scope]);
  readRef.current = read;

  useEffect(() => {
    mounted.current = true;
    currentContext.current = contextKey;
    void read();
    return () => {
      mounted.current = false;
      sequence.current += 1;
      currentContext.current = "";
    };
  }, [contextKey, read]);

  const archive = async () => {
    if (disabled || actionLock.current || readLock.current || !confirmed || result?.kind !== "snapshot") return;
    const reviewed = result.data;
    if (!reviewed.can_archive || !reviewed.review_fingerprint || loading) return;
    actionLock.current = true;
    const expectedContext = contextKey;
    const operationSequence = ++sequence.current;
    setPending(true);
    setArchived(false);
    try {
      onBusyChangeRef.current?.(true);
      const next = await clientRef.current.archive(clientKind, scope, reviewed.review_fingerprint);
      if (!mounted.current || currentContext.current !== expectedContext || sequence.current !== operationSequence) return;
      setResult(next);
      if (next.kind === "snapshot" && next.data.status === "archived") {
        setArchived(true);
        setConfirmed(false);
        onArchivedRef.current();
      }
    } catch {
      if (mounted.current && currentContext.current === expectedContext && sequence.current === operationSequence) {
        setResult({ kind: "error", data: {
          code: "unavailable",
          retryable: true,
          message_key: "retained.unavailable",
          diagnostic_id: null,
        } });
      }
    } finally {
      actionLock.current = false;
      onBusyChangeRef.current?.(false);
      if (mounted.current && currentContext.current === expectedContext && sequence.current === operationSequence) {
        setPending(false);
      }
      if (queuedRead.current && mounted.current) {
        queuedRead.current = false;
        void readRef.current();
      }
    }
  };

  if (!loading && !archived && result?.kind === "snapshot" && !shouldShow(result.data)) return null;

  const snapshotData = result?.kind === "snapshot" ? result.data : null;
  const error = result?.kind === "error" ? result.data : null;
  const remaining = snapshotData ? Math.max(0, snapshotData.archive_limit - snapshotData.completed_archives) : null;
  const needsAction = snapshotData && snapshotData.selected_records > 0;

  return <section className="retained-versions" aria-labelledby="retained-versions-title">
    <div className="retained-versions__heading">
      <h3 id="retained-versions-title">{text.title}</h3>
      <button className="retained-versions__refresh" type="button" disabled={disabled || loading || pending} onClick={() => void read()}>
        {loading ? text.refreshing : text.refresh}
      </button>
    </div>

    {loading && <p className="retained-versions__status" role="status" aria-live="polite">{text.loading}</p>}
    {error && <div className="retained-versions__error" role="status" aria-live="polite">
      <strong>{text.errorTitle}</strong>
      <span>{text.errors[error.code]}</span>
    </div>}

    {snapshotData && <>
      <div className="retained-versions__counts">
        <div><span>{text.selected}</span><strong>{snapshotData.selected_records} / {snapshotData.active_records}</strong></div>
        <div><span>{text.remaining}</span><strong>{remaining} / {snapshotData.archive_limit}</strong></div>
      </div>
      {snapshotData.changed_versions > 0 && <p className="retained-versions__changed">{text.changed(snapshotData.changed_versions)}</p>}
      {snapshotData.unresolved_records > 0 && <p className="retained-versions__notice">{text.unresolved}</p>}
      {(snapshotData.completed_archives >= snapshotData.archive_limit
        || (snapshotData.selected_records === 0 && snapshotData.active_records >= snapshotData.record_limit))
        && <p className="retained-versions__notice">{text.full}</p>}
      {snapshotData.selected_records === 0 && snapshotData.active_records < snapshotData.record_limit && <p className="retained-versions__muted">{text.noSelection}</p>}
      {needsAction && <div className="retained-versions__action">
        <p className="retained-versions__copy">{text.unchanged}</p>
        <label className="retained-versions__confirm">
          <input type="checkbox" checked={confirmed} disabled={disabled || loading || pending || !snapshotData.can_archive} onChange={(event) => setConfirmed(event.currentTarget.checked)} />
          <span>{text.confirm}</span>
        </label>
        <button className="retained-versions__primary" type="button" disabled={disabled || pending || loading || !snapshotData.can_archive || !confirmed} onClick={() => void archive()}>
          {pending ? text.archiving : text.archive}
        </button>
      </div>}
    </>}
    {archived && <p className="retained-versions__success" role="status" aria-live="polite">{text.archived}</p>}
  </section>;
}
