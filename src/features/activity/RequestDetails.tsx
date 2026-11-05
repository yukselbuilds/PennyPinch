import { useEffect, useRef } from "react";
import type {
  ActivityDetailSnapshot,
  ActivityLayer,
  ActivityMeasurementDetail,
  ActivityState,
} from "../../lib/activity";
import { formatMetricNumber, formatMetricPercent } from "../../lib/metrics";
import type { MetricPeriod } from "../../lib/metrics";
import type { PreferenceLanguage } from "../../lib/preferences";

interface RequestDetailsProps {
  language: PreferenceLanguage;
  period: MetricPeriod;
  selectedId: string | null;
  detail: ActivityState<ActivityDetailSnapshot>;
  onClose: () => void;
}

const copy = {
  tr: {
    title: "İstek ayrıntısı",
    close: "Kapat",
    loading: "Ayrıntı okunuyor…",
    unavailable: "Bu kaydın ayrıntısı kullanılamıyor.",
    errors: {
      invalid_input: "Ayrıntı isteği doğrulanamadı.", unsafe_path: "Etkinlik deposu güvenli değil.", busy: "Etkinlik deposu meşgul.",
      unsupported_schema: "Etkinlik biçimi desteklenmiyor.", corrupt: "Etkinlik kaydı doğrulanamadı.", capacity: "Yerel saklama sınırına ulaşıldı.",
      unavailable: "Etkinlik deposu kullanılamıyor.", invalid_local_time: "Dönem sınırı doğrulanamadı.",
      invalid_response: "Ayrıntı yanıtı doğrulanamadı.", request_failed: "Ayrıntı isteği tamamlanamadı.", not_found: "İstek ayrıntısı bulunamadı.",
    },
    stale: "Önceki ayrıntı gösteriliyor.", periodMismatch: "Ayrıntı seçili istek için doğrulanamadı.",
    id: "Kayıt", client: "Uygulama bilinmiyor", model: "Model bilinmiyor", provider: "Sağlayıcı",
    agent: "Ajan bilinmiyor", main: "Ana ajan", subagent: "Alt ajan", roleUnknown: "Rol bilinmiyor",
    conflict: "Atıf çakışması", metadataConflict:"İstek bilgileri çakıştı", sourceScope:"Cache bildirimi sağlayıcı indirimi kanıtı değildir. Hata bildirilmemesi başarı anlamına gelmez.", metadata: "İstek bilgileri", total: "Toplam süre", optimization: "İyileştirme süresi",
    firstToken: "İlk token süresi", cache: "Kaynak cache bildirimi", transforms: "Dönüşüm sayısı", hit: "Vuruş bildirildi",
    noHit: "Vuruş bildirilmedi", unknown: "Bilinmiyor", optimizers: "Kayıtlı optimizer etiketleri; tüm ayarların etkin olduğunu göstermez.",
    reduction: "azalma", increase: "artış", unchanged: "değişim yok",
    measurements: "Ölçüm ayrıntıları", via: "Gözlem kaynağı", status: "Durum", before: "Önce", after: "Sonra",
    basis: "Temel", local: "Yerel tahmin", tokenizer: "Sağlayıcı tokenizer’ı", component: "Bileşen",
    eligible: "Ana metriğe uygun", yes: "Evet", no: "Hayır", providerUsage: "Sağlayıcı bildirimi", input: "Girdi",
    output: "Çıktı", cachedInput: "Cache girdisi", omitted: "ölçüm gösterilmedi", measured: "Ölçüldü",
    missingCounter: "Sayaç yok", unknownBasis: "Temel bilinmiyor", incomparable: "Karşılaştırılamaz",
    uncorrelated: "İlişkilendirilemedi", missingTimestamp: "Zaman yok", conflictStatus: "Çakışma",
    components: { message_input: "Mesaj girdisi", whole_input: "Tüm girdi", tool_schema: "Araç şeması", novel_input: "Yeni girdi", cache_input: "Cache girdisi", output: "Çıktı", unknown: "Bilinmiyor" },
    layers: { headroom: "Headroom", rtk: "RTK", cache: "Cache", output: "Output" },
    time: "Zaman", error: "Hata bildirildi", noError: "Hata bildirilmedi", outcomeUnknown: "Hata durumu bilinmiyor",
    unmeasurable: "Ölçülemedi",
  },
  en: {
    title: "Request details",
    close: "Close",
    loading: "Reading details…",
    unavailable: "Details are unavailable for this record.",
    errors: {
      invalid_input: "The detail request could not be validated.", unsafe_path: "The activity store is not safe.", busy: "The activity store is busy.",
      unsupported_schema: "The activity format is unsupported.", corrupt: "The activity record could not be validated.", capacity: "The local retention limit was reached.",
      unavailable: "The activity store is unavailable.", invalid_local_time: "The period boundary could not be validated.",
      invalid_response: "The detail response could not be validated.", request_failed: "The detail request could not be completed.", not_found: "Request details could not be found.",
    },
    stale: "Showing previous details.", periodMismatch: "Details could not be verified for the selected request.",
    id: "Record", client: "Application unknown", model: "Model unknown", provider: "Provider",
    agent: "Agent unknown", main: "Main agent", subagent: "Subagent", roleUnknown: "Role unknown",
    conflict: "Attribution conflict", metadataConflict:"Request metadata conflict", sourceScope:"A cache report does not prove a provider discount. No error reported does not establish success.", metadata: "Request metadata", total: "Total time", optimization: "Optimization time",
    firstToken: "First-token time", cache: "Source cache report", transforms: "Transform count", hit: "Hit reported",
    noHit: "No hit reported", unknown: "Unknown", optimizers: "Recorded optimizer labels; this does not show that all settings are active.",
    reduction: "reduction", increase: "increase", unchanged: "no change",
    measurements: "Measurement details", via: "Observed via", status: "Status", before: "Before", after: "After",
    basis: "Basis", local: "Local estimate", tokenizer: "Provider tokenizer", component: "Component",
    eligible: "Eligible for main metric", yes: "Yes", no: "No", providerUsage: "Provider report", input: "Input",
    output: "Output", cachedInput: "Cached input", omitted: "measurements omitted", measured: "Measured",
    missingCounter: "Missing counter", unknownBasis: "Unknown basis", incomparable: "Incomparable",
    uncorrelated: "Uncorrelated", missingTimestamp: "Missing timestamp", conflictStatus: "Conflict",
    components: { message_input: "Message input", whole_input: "Whole input", tool_schema: "Tool schema", novel_input: "Novel input", cache_input: "Cache input", output: "Output", unknown: "Unknown" },
    layers: { headroom: "Headroom", rtk: "RTK", cache: "Cache", output: "Output" },
    time: "Time", error: "Error reported", noError: "No error reported", outcomeUnknown: "Error status unknown",
    unmeasurable: "Unmeasurable",
  },
} as const;

type Copy = { [Key in keyof typeof copy.en]: typeof copy.en[Key] extends object
  ? { [NestedKey in keyof typeof copy.en[Key]]: string }
  : string };

function timeText(value: string | null, language: PreferenceLanguage): string {
  if (!value) return "—";
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "—";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function duration(value: number | null, language: PreferenceLanguage): string {
  return value === null ? "—" : `${new Intl.NumberFormat(language==="tr"?"tr-TR":"en-US",{maximumFractionDigits:2}).format(value)} ms`;
}

function layerName(layer: ActivityLayer, text: Copy): string {
  return text.layers[layer];
}

function errorMessage(error: keyof Copy["errors"], text: Copy): string {
  return text.errors[error];
}

function DetailMeasurement({ row, language, text }: { row: ActivityMeasurementDetail; language: PreferenceLanguage; text: Copy }) {
  const status = {
    measured: text.measured, missing_counter: text.missingCounter, unknown_basis: text.unknownBasis,
    incomparable: text.incomparable, uncorrelated: text.uncorrelated, missing_timestamp: text.missingTimestamp, conflict: text.conflictStatus,
  }[row.status];
  const reading = (value: ActivityMeasurementDetail["before"]) => {
    const basis = value.basis_kind === "local_estimate" ? text.local : value.basis_kind === "provider_tokenizer" ? text.tokenizer : text.unknown;
    return `${formatMetricNumber(value.tokens, language)} · ${basis}${value.basis_id ? ` · ${value.basis_id.slice(0, 8)}` : ""} · ${text.components[value.component]}`;
  };
  const saved = row.saved_tokens === null ? "—" : `${formatMetricNumber(Math.abs(row.saved_tokens), language)} ${row.saved_tokens > 0 ? text.reduction : row.saved_tokens < 0 ? text.increase : text.unchanged}`;
  const percent = formatMetricPercent(row.saved_percent === null ? null : Math.abs(row.saved_percent), language);

  return (
    <li className="activity__measurement">
      <div className="activity__measurement-heading">
        <strong>{layerName(row.optimizer, text)} · {status}</strong>
        <span>{timeText(row.timestamp_utc, language)}</span>
      </div>
      <dl>
        <div><dt>{text.via}</dt><dd>{row.observed_via.length ? row.observed_via.join(" · ") : "—"}</dd></div>
        <div><dt>{text.before}</dt><dd>{reading(row.before)}</dd></div>
        <div><dt>{text.after}</dt><dd>{reading(row.after)}</dd></div>
        <div><dt>{text.eligible}</dt><dd>{row.main_metric_eligible ? text.yes : text.no}</dd></div>
        <div><dt>{text.input}</dt><dd>{saved} · {percent}</dd></div>
        <div><dt>{text.providerUsage}</dt><dd>{text.input}: {formatMetricNumber(row.provider_usage.input, language)} · {text.output}: {formatMetricNumber(row.provider_usage.output, language)} · {text.cachedInput}: {formatMetricNumber(row.provider_usage.cached_input, language)}</dd></div>
        <div><dt>{text.total}</dt><dd>{duration(row.metadata.total_ms, language)}</dd></div>
        <div><dt>{text.optimization}</dt><dd>{duration(row.metadata.optimization_ms, language)}</dd></div>
        <div><dt>{text.firstToken}</dt><dd>{duration(row.metadata.first_token_ms, language)}</dd></div>
      </dl>
    </li>
  );
}

export function RequestDetails({ language, period, selectedId, detail, onClose }: RequestDetailsProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const text = copy[language === "tr" ? "tr" : "en"];
  const current = detail.kind === "ready" && detail.snapshot.data.row.origin_id === selectedId
    && detail.snapshot.data.period === period ? detail.snapshot : null;
  const row = current?.data.row;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (selectedId && !dialog.open) dialog.showModal();
    else if (!selectedId && dialog.open) dialog.close();
  }, [selectedId]);

  const agent = row?.identity.agent;
  const agentText = !agent ? text.agent : `${agent.role === "main" ? text.main : agent.role === "subagent" ? text.subagent : text.roleUnknown} · ${agent.id.slice(0, 8)}`;
  const model = row?.identity.model ?? text.model;
  const client = row?.identity.client === "codex" ? "Codex" : row?.identity.client === "claude_code" ? "Claude Code" : row?.identity.client === "opencode" ? "OpenCode" : text.client;

  return (
    <dialog ref={dialogRef} className="activity__dialog" aria-labelledby="activity-detail-title" onClose={() => {if(!dialogRef.current?.open)onClose();}}>
      <header className="activity__dialog-header">
        <div>
          <h2 id="activity-detail-title">{text.title}</h2>
          <p>{text.id} · {selectedId?.slice(0, 8) ?? "—"}</p>
        </div>
        <button className="activity__close" type="button" onClick={() => dialogRef.current?.close()}>{text.close}</button>
      </header>

      {detail.kind === "loading" && <p className="activity__notice" role="status">{text.loading}</p>}
      {detail.kind === "unavailable" && <p className="activity__notice" role="status">{text.unavailable}</p>}
      {detail.kind === "error" && <p className="activity__notice activity__notice--error" role="alert">{errorMessage(detail.error, text)}</p>}
      {detail.kind === "ready" && !current && <p className="activity__notice activity__notice--error" role="alert">{text.periodMismatch}</p>}
      {detail.kind === "ready" && detail.stale && <p className="activity__notice" role="status">{text.stale}</p>}

      {row && <>
        <section className="activity__detail-identity" aria-label={text.metadata}>
          <strong>{client} · {model}</strong>
          {row.identity.provider && <span>{text.provider}: {row.identity.provider}</span>}
          <span>{agentText}</span>
          {row.identity.conflicted && <span className="activity__field-error">{text.conflict}</span>}
          {row.metadata.conflicted && <span className="activity__field-error">{text.metadataConflict}</span>}
          <span>{text.time}: {timeText(row.timestamp_utc, language)}</span>
        </section>

        <section className="activity__detail-section">
          <h3>{text.metadata}</h3>
          <dl className="activity__detail-grid">
            <div><dt>{text.total}</dt><dd>{duration(row.metadata.total_ms, language)}</dd></div>
            <div><dt>{text.optimization}</dt><dd>{duration(row.metadata.optimization_ms, language)}</dd></div>
            <div><dt>{text.firstToken}</dt><dd>{duration(row.metadata.first_token_ms, language)}</dd></div>
            <div><dt>{text.cache}</dt><dd>{row.metadata.cache_reported_hit === null ? text.unknown : row.metadata.cache_reported_hit ? text.hit : text.noHit}</dd></div>
            <div><dt>{text.transforms}</dt><dd>{formatMetricNumber(row.metadata.transforms_count, language)}</dd></div>
            <div><dt>{text.status}</dt><dd>{row.metadata.outcome === "error_reported" ? text.error : row.metadata.outcome === "no_error_reported" ? text.noError : text.outcomeUnknown}</dd></div>
          </dl>
          <p className="activity__optimizers">{text.optimizers} {row.layers.length ? row.layers.map((layer) => layerName(layer, text)).join(" · ") : "—"}</p>
          <p className="activity__optimizers">{text.sourceScope}</p>
        </section>

        <section className="activity__detail-section">
          <h3>{text.measurements}</h3>
          {current.data.measurements.length === 0 ? <p className="activity__notice">{text.unmeasurable}</p> : (
            <ol className="activity__measurements">
              {current.data.measurements.slice(0, 32).map((measurement) => (
                <DetailMeasurement key={measurement.event_id} row={measurement} language={language} text={text} />
              ))}
            </ol>
          )}
          {current.data.omitted_measurements > 0 && <p className="activity__notice">
            {formatMetricNumber(current.data.omitted_measurements, language)} {text.omitted}
          </p>}
        </section>
      </>}
    </dialog>
  );
}
