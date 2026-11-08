import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Icon } from "../../components/Icon";
import { exportClient, type ExportErrorCode, type ExportKind, type ExportResult } from "../../lib/exportReport";
import type { PreferenceLanguage } from "../../lib/preferences";
import "./export-button.css";

const copy = {
  tr: {
    title: "Dışa aktar",
    menuLabel: "Rapor türünü seç",
    scope: "Seçili dönemin toplamlarını ve ölçüm kapsamını kaydet.",
    diagnostics: "Tanı raporu kişisel dosya yolu veya içerik taşımaz.",
    nativeRequired: "Dışa aktarma masaüstü uygulamasında kullanılabilir.",
    statsJson: "İstatistik · JSON",
    statsCsv: "İstatistik · CSV",
    diagnosticsJson: "Tanı raporu · JSON",
    exporting: "Rapor hazırlanıyor…",
    saved: "Rapor yeni dosyaya kaydedildi.",
    cancelled: "Kaydetme iptal edildi.",
    client_busy: "Başka bir dışa aktarma sürüyor.",
    desktop_unavailable: "Dışa aktarma masaüstü uygulamasında kullanılabilir.",
    invalid_response: "Dışa aktarma yanıtı doğrulanamadı.",
    request_failed: "Dışa aktarma tamamlanamadı. Yeniden deneyebilirsin.",
    stale: "Seçili ölçüm işlem sırasında değişti; sonuç dışa aktarma başlatıldığındaki seçim içindir.",
    errors: {
      invalid_input: "Dışa aktarma isteği doğrulanamadı.",
      snapshot_expired: "Ölçümün süresi doldu. Dönemi yenileyip yeniden dışa aktar.",
      busy: "Başka bir yerel işlem sürüyor. Biraz sonra yeniden dene.",
      unavailable: "Dışa aktarma şu anda kullanılamıyor.",
      unsafe_path: "Güvenli bir kaydetme konumu seçilemedi.",
      file_exists: "Dosya zaten var. Farklı bir ad seç.",
      write_failed: "Rapor yazılamadı. Başka bir konum seçip yeniden dene.",
    } satisfies Record<ExportErrorCode, string>,
  },
  en: {
    title: "Export",
    menuLabel: "Choose a report type",
    scope: "Save totals and measurement coverage for the selected period.",
    diagnostics: "The diagnostics report does not include personal file paths or content.",
    nativeRequired: "Export is available in the desktop app.",
    statsJson: "Statistics · JSON",
    statsCsv: "Statistics · CSV",
    diagnosticsJson: "Diagnostics · JSON",
    exporting: "Preparing report…",
    saved: "The report was saved to a new file.",
    cancelled: "Saving was cancelled.",
    client_busy: "Another export is in progress.",
    desktop_unavailable: "Export is available in the desktop app.",
    invalid_response: "The export response could not be validated.",
    request_failed: "The export could not be completed. Try again.",
    stale: "The selected measurement changed during export; the result uses the selection from when export started.",
    errors: {
      invalid_input: "The export request could not be validated.",
      snapshot_expired: "The measurement expired. Refresh the period and export again.",
      busy: "Another local operation is running. Try again shortly.",
      unavailable: "Export is temporarily unavailable.",
      unsafe_path: "A safe save location could not be selected.",
      file_exists: "That file already exists. Choose a different name.",
      write_failed: "The report could not be written. Choose another location and try again.",
    } satisfies Record<ExportErrorCode, string>,
  },
} as const;

type Outcome = { result: ExportResult; stale: boolean };
type ResultCopy = {
  errors: Record<ExportErrorCode, string>;
  saved: string;
  cancelled: string;
  client_busy: string;
  desktop_unavailable: string;
  invalid_response: string;
  request_failed: string;
};

function resultText(result: ExportResult, text: ResultCopy): string {
  if (result.kind === "export_error") return text.errors[result.code];
  if (result.kind === "saved") return text.saved;
  return text[result.kind];
}

export function ExportButton({ language, snapshotRequestId }: { language: PreferenceLanguage; snapshotRequestId: string | null }) {
  const text = copy[language];
  const native = exportClient.isAvailable();
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const summaryRef = useRef<HTMLElement>(null);
  const mounted = useRef(false);
  const inFlight = useRef(false);
  const currentSnapshot = useRef(snapshotRequestId);
  currentSnapshot.current = snapshotRequestId;
  const [pending, setPending] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  function closeMenu() {
    if (detailsRef.current) detailsRef.current.open = false;
    summaryRef.current?.focus();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDetailsElement>) {
    if (event.key === "Escape" && detailsRef.current?.open) {
      event.preventDefault();
      event.stopPropagation();
      closeMenu();
    }
  }

  async function run(kind: ExportKind) {
    if (inFlight.current || !native || (kind !== "diagnostics_json" && snapshotRequestId === null)) return;
    const snapshotAtStart = snapshotRequestId;
    inFlight.current = true;
    setPending(true);
    setOutcome(null);
    closeMenu();
    let result: ExportResult;
    try { result = await exportClient.exportReport(kind, language, snapshotAtStart); }
    catch { result = { kind: "request_failed" }; }
    inFlight.current = false;
    if (!mounted.current) return;
    setPending(false);
    setOutcome({ result, stale: currentSnapshot.current !== snapshotAtStart });
  }

  return <div className="export-button">
    <details className="export-button__details" ref={detailsRef} onKeyDown={onKeyDown}>
      <summary className="export-button__trigger" ref={summaryRef} aria-label={text.title} aria-disabled={!native || pending}>
        {text.title}<Icon name="chevron" className="export-button__chevron" />
      </summary>
      <div className="export-button__panel">
        <p>{text.scope}</p>
        <p>{text.diagnostics}</p>
        {!native && <p className="export-button__notice" role="status">{text.nativeRequired}</p>}
        <div className="export-button__options" role="group" aria-label={text.menuLabel}>
          <button type="button" disabled={!native || pending || snapshotRequestId === null} onClick={() => void run("statistics_json")}>{text.statsJson}</button>
          <button type="button" disabled={!native || pending || snapshotRequestId === null} onClick={() => void run("statistics_csv")}>{text.statsCsv}</button>
          <button type="button" disabled={!native || pending} onClick={() => void run("diagnostics_json")}>{text.diagnosticsJson}</button>
        </div>
      </div>
    </details>
    {pending && <p className="export-button__status" role="status" aria-live="polite">{text.exporting}</p>}
    {!pending && outcome && <p className="export-button__status" role="status" aria-live="polite">
      {resultText(outcome.result, text)}{outcome.stale ? ` ${text.stale}` : ""}
    </p>}
  </div>;
}
