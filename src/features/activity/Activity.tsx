import { useEffect, useState, type FormEvent } from "react";
import {
  defaultActivityFilter,
  type ActivityError,
  type ActivityFilter,
  type ActivityProps,
  type ActivityRow,
} from "../../lib/activity";
import { formatMetricNumber, formatMetricPercent, isMetricLabel } from "../../lib/metrics";
import type { ClientKind } from "../../lib/integrations";
import { RequestDetails } from "./RequestDetails";
import "./activity.css";

const copy = {
  tr: {
    title: "Etkinlik",
    period: "Dönem",
    periods: { session: "Oturum", today: "Bugün", seven_days: "7 gün" },
    requests: "İstek kayıtları",
    description: "İstek kaydı fiziksel bir API çağrısının bire bir kanıtı değildir; istemci ve ajan atfı bilinmiyor olabilir.",
    shown: "gösterilen",
    matched: "eşleşen",
    capped: "Liste 100 kayıtla sınırlı; daha fazla eşleşme var.",
    unplaced: "İstek",
    unplacedNote: "zamanı bilinmiyor; dönem içinde gerçekleştiği doğrulanmadı",
    filters: "Filtreler",
    client: "İstemci",
    clients: { any: "Tümü", unknown: "Bilinmiyor", codex: "Codex", claude_code: "Claude Code", opencode: "OpenCode" },
    model: "Model",
    models: { any: "Tümü", unknown: "Bilinmiyor", exact: "Tam ad" },
    modelPlaceholder: "Model adı",
    modelInvalid: "Geçerli bir model adı gir (en fazla 128 karakter).",
    outcome: "Hata bildirimi",
    outcomes: { any: "Tümü", unknown: "Bilinmiyor", no_error_reported: "Hata bildirilmedi", error_reported: "Hata bildirildi" },
    measurement: "Ölçüm",
    measurements: { any: "Tümü", measured: "Ölçüldü", unmeasurable: "Ölçülemedi", conflict: "Çakışma" },
    applyFilters: "Filtrele",
    resetFilters: "Temizle",
    time: "Zaman",
    duration: "Süre",
    result: "Sonuç",
    input: "Girdi değişimi",
    clientUnknown: "Uygulama bilinmiyor",
    modelUnknown: "Model bilinmiyor",
    provider: "Sağlayıcı",
    agentUnknown: "Ajan bilinmiyor",
    mainAgent: "Ana ajan",
    subagent: "Alt ajan",
    roleUnknown: "Rol bilinmiyor",
    conflict: "Ölçüm çakışması",
    attributionConflict: "Atıf çakışması",
    metadataConflict: "İstek bilgileri çakıştı",
    noError: "Hata bildirilmedi",
    errorReported: "Hata bildirildi",
    outcomeUnknown: "Bilinmiyor",
    measured: "Ölçüldü",
    unmeasurable: "Ölçülemedi",
    unmeasurableReason: "Ölçüm karşılaştırılabilir değil.",
    changeReduction: "azalma",
    changeIncrease: "artış",
    noChange: "değişim yok",
    unknown: "—",
    loading: "İstek kayıtları okunuyor…",
    empty: "Bu dönemde eşleşen kayıt yok.",
    unavailable: "Bu masaüstü oturumunda etkinlik kullanılamıyor.",
    errors: {
      invalid_input: "Etkinlik isteği doğrulanamadı.",
      unsafe_path: "Etkinlik deposu güvenli bir konumda değil.",
      busy: "Etkinlik deposu meşgul. Biraz sonra yeniden dene.",
      unsupported_schema: "Etkinlik verisinin biçimi desteklenmiyor.",
      corrupt: "Etkinlik kaydı doğrulanamadı.",
      capacity: "Yerel etkinlik saklama sınırına ulaşıldı.",
      unavailable: "Etkinlik deposu kullanılamıyor.",
      invalid_local_time: "Dönem sınırı doğrulanamadı.",
      invalid_response: "Etkinlik yanıtı doğrulanamadı.",
      request_failed: "Etkinlik isteği tamamlanamadı.",
      not_found: "İstek ayrıntısı bulunamadı.",
    } satisfies Record<ActivityError, string>,
    stale: "Önceki kayıtlar gösteriliyor",
    refreshed: "Yenile",
    refreshing: "Yenileniyor…",
    requestId: "Kayıt",
  },
  en: {
    title: "Activity",
    period: "Period",
    periods: { session: "Session", today: "Today", seven_days: "7 days" },
    requests: "Request records",
    description: "A request record is not necessarily one physical API call; client or agent attribution can be unknown.",
    shown: "shown",
    matched: "matched",
    capped: "The list is capped at 100 records; more matches exist.",
    unplaced: "Requests",
    unplacedNote: "have unknown timing; occurrence within this period is unverified",
    filters: "Filters",
    client: "Client",
    clients: { any: "Any", unknown: "Unknown", codex: "Codex", claude_code: "Claude Code", opencode: "OpenCode" },
    model: "Model",
    models: { any: "Any", unknown: "Unknown", exact: "Exact name" },
    modelPlaceholder: "Model name",
    modelInvalid: "Enter a valid model name (up to 128 characters).",
    outcome: "Error notice",
    outcomes: { any: "Any", unknown: "Unknown", no_error_reported: "No error reported", error_reported: "Error reported" },
    measurement: "Measurement",
    measurements: { any: "Any", measured: "Measured", unmeasurable: "Unmeasurable", conflict: "Conflict" },
    applyFilters: "Apply filters",
    resetFilters: "Clear",
    time: "Time",
    duration: "Duration",
    result: "Result",
    input: "Input change",
    clientUnknown: "Application unknown",
    modelUnknown: "Model unknown",
    provider: "Provider",
    agentUnknown: "Agent unknown",
    mainAgent: "Main agent",
    subagent: "Subagent",
    roleUnknown: "Role unknown",
    conflict: "Measurement conflict",
    attributionConflict: "Attribution conflict",
    metadataConflict: "Request metadata conflict",
    noError: "No error reported",
    errorReported: "Error reported",
    outcomeUnknown: "Unknown",
    measured: "Measured",
    unmeasurable: "Unmeasurable",
    unmeasurableReason: "The measurement is not comparable.",
    changeReduction: "reduction",
    changeIncrease: "increase",
    noChange: "no change",
    unknown: "—",
    loading: "Reading request records…",
    empty: "No matching records in this period.",
    unavailable: "Activity is unavailable in this desktop session.",
    errors: {
      invalid_input: "The activity request could not be validated.",
      unsafe_path: "The activity store is not in a safe location.",
      busy: "The activity store is busy. Try again shortly.",
      unsupported_schema: "The activity data format is unsupported.",
      corrupt: "The activity record could not be validated.",
      capacity: "The local activity retention limit was reached.",
      unavailable: "The activity store is unavailable.",
      invalid_local_time: "The period boundary could not be validated.",
      invalid_response: "The activity response could not be validated.",
      request_failed: "The activity request could not be completed.",
      not_found: "Request details could not be found.",
    } satisfies Record<ActivityError, string>,
    stale: "Showing previous records",
    refreshed: "Refresh",
    refreshing: "Refreshing…",
    requestId: "Record",
  },
} as const;

type Copy = { [Key in keyof typeof copy.en]: typeof copy.en[Key] extends object
  ? { [NestedKey in keyof typeof copy.en[Key]]: string }
  : string };
const clientKinds = ["codex", "claude_code", "opencode"] as const;
const modelKinds = ["any", "unknown", "exact"] as const;
const outcomeKinds = ["any", "unknown", "no_error_reported", "error_reported"] as const;
const measurementKinds = ["any", "measured", "unmeasurable", "conflict"] as const;

function formatTime(value: string | null, language: ActivityProps["language"]): string {
  if (!value) return "—";
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "—";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function clientName(client: ClientKind | null, text: Copy): string {
  if (client === null) return text.clientUnknown;
  return client === "claude_code" ? text.clients.claude_code : text.clients[client];
}

function agentName(row: ActivityRow, text: Copy): string {
  const agent = row.identity.agent;
  if (!agent) return text.agentUnknown;
  const role = agent.role === "main" ? text.mainAgent : agent.role === "subagent" ? text.subagent : text.roleUnknown;
  return `${role} · ${agent.id.slice(0, 8)}`;
}

function changeText(row: ActivityRow, language: ActivityProps["language"], text: Copy): string {
  if (row.measurement === "conflict") return text.conflict;
  if (row.measurement === "unmeasurable" || row.totals.status !== "measured") return text.unmeasurableReason;
  if (row.totals.saved_tokens === null) return text.unknown;
  const saved = row.totals.saved_tokens;
  const amount = formatMetricNumber(Math.abs(saved), language);
  const change = saved > 0 ? text.changeReduction : saved < 0 ? text.changeIncrease : text.noChange;
  const percent = formatMetricPercent(row.totals.saved_percent === null ? null : Math.abs(row.totals.saved_percent), language);
  return `${amount} ${change} · ${percent}`;
}

function errorText(error: ActivityError, text: Copy): string {
  return text.errors[error];
}

export function Activity(props: ActivityProps) {
  const text = copy[props.language === "tr" ? "tr" : "en"];
  const filterKey = `${props.filter.client}|${props.filter.model.kind}|${props.filter.model.kind === "exact" ? props.filter.model.value : ""}|${props.filter.outcome}|${props.filter.measurement}`;
  const [client, setClient] = useState<ActivityFilter["client"]>(props.filter.client);
  const [modelKind, setModelKind] = useState<ActivityFilter["model"]["kind"]>(props.filter.model.kind);
  const [modelValue, setModelValue] = useState(props.filter.model.kind === "exact" ? props.filter.model.value : "");
  const [outcome, setOutcome] = useState<ActivityFilter["outcome"]>(props.filter.outcome);
  const [measurement, setMeasurement] = useState<ActivityFilter["measurement"]>(props.filter.measurement);
  const [modelError, setModelError] = useState(false);
  useEffect(() => {
    setClient(props.filter.client);
    setModelKind(props.filter.model.kind);
    setModelValue(props.filter.model.kind === "exact" ? props.filter.model.value : "");
    setOutcome(props.filter.outcome);
    setMeasurement(props.filter.measurement);
    setModelError(false);
  }, [filterKey]);

  const snapshot = props.state.kind === "ready" ? props.state.snapshot : null;
  const data = snapshot?.data;
  const periodOptions = ["session", "today", "seven_days"] as const;
  const clientOptions: readonly ("any" | "unknown" | ClientKind)[] = ["any", "unknown", ...clientKinds];

  function applyFilter(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const clientValue = form.get("client");
    const modelValueKind = form.get("model-kind");
    const outcomeValue = form.get("outcome");
    const measurementValue = form.get("measurement");
    if (typeof clientValue !== "string" || !clientOptions.includes(clientValue as typeof clientOptions[number])
      || typeof modelValueKind !== "string" || !modelKinds.includes(modelValueKind as typeof modelKinds[number])
      || typeof outcomeValue !== "string" || !outcomeKinds.includes(outcomeValue as typeof outcomeKinds[number])
      || typeof measurementValue !== "string" || !measurementKinds.includes(measurementValue as typeof measurementKinds[number])) return;
    const exactModel = form.get("model-value");
    if (modelValueKind === "exact" && !isMetricLabel(exactModel)) {
      setModelError(true);
      event.currentTarget.querySelector<HTMLInputElement>('[name="model-value"]')?.focus();
      return;
    }
    setModelError(false);
    props.onFilterChange({
      client: clientValue as ActivityFilter["client"],
      model: modelValueKind === "exact" ? { kind: "exact", value: exactModel as string }
        : { kind: modelValueKind as "any" | "unknown" },
      outcome: outcomeValue as ActivityFilter["outcome"],
      measurement: measurementValue as ActivityFilter["measurement"],
    });
  }

  function clearFilters() {
    setClient("any");
    setModelKind("any");
    setModelValue("");
    setOutcome("any");
    setMeasurement("any");
    setModelError(false);
    props.onFilterChange(defaultActivityFilter);
  }

  return (
    <section className="activity" aria-labelledby="activity-title">
      <header className="activity__header">
        <div>
          <h2 id="activity-title">{text.title}</h2>
          <p>{text.description}</p>
        </div>
        <button className="activity__refresh" type="button" onClick={props.onRefresh}
          disabled={props.state.kind === "loading" || (props.state.kind === "ready" && props.state.refreshing)}>
          {props.state.kind === "ready" && props.state.refreshing ? text.refreshing : text.refreshed}
        </button>
      </header>

      <div className="activity__period" role="group" aria-label={text.period}>
        {periodOptions.map((period) => (
          <button key={period} type="button" aria-pressed={props.period === period} onClick={() => props.onPeriodChange(period)}>
            {text.periods[period]}
          </button>
        ))}
      </div>

      <details className="activity__filters">
        <summary>{text.filters}</summary>
        <form onSubmit={applyFilter}>
          <label>{text.client}
            <select name="client" value={client} onChange={(event) => setClient(event.currentTarget.value as ActivityFilter["client"])}>
              <option value="any">{text.clients.any}</option><option value="unknown">{text.clients.unknown}</option>
              <option value="codex">Codex</option><option value="claude_code">Claude Code</option><option value="opencode">OpenCode</option>
            </select>
          </label>
          <label>{text.model}
            <select name="model-kind" value={modelKind} onChange={(event) => setModelKind(event.currentTarget.value as ActivityFilter["model"]["kind"])}>
              <option value="any">{text.models.any}</option><option value="unknown">{text.models.unknown}</option><option value="exact">{text.models.exact}</option>
            </select>
          </label>
          <label>{text.modelPlaceholder}
            <input name="model-value" type="text" maxLength={128} value={modelValue} disabled={modelKind !== "exact"}
              aria-invalid={modelError || undefined} aria-describedby={modelError ? "activity-model-error" : undefined}
              onChange={(event) => setModelValue(event.currentTarget.value)} />
            {modelError && <small id="activity-model-error" className="activity__field-error" role="alert">{text.modelInvalid}</small>}
          </label>
          <label>{text.outcome}
            <select name="outcome" value={outcome} onChange={(event) => setOutcome(event.currentTarget.value as ActivityFilter["outcome"])}>
              <option value="any">{text.outcomes.any}</option><option value="unknown">{text.outcomes.unknown}</option>
              <option value="no_error_reported">{text.outcomes.no_error_reported}</option><option value="error_reported">{text.outcomes.error_reported}</option>
            </select>
          </label>
          <label>{text.measurement}
            <select name="measurement" value={measurement} onChange={(event) => setMeasurement(event.currentTarget.value as ActivityFilter["measurement"])}>
              <option value="any">{text.measurements.any}</option><option value="measured">{text.measurements.measured}</option>
              <option value="unmeasurable">{text.measurements.unmeasurable}</option><option value="conflict">{text.measurements.conflict}</option>
            </select>
          </label>
          <div className="activity__filter-actions">
            <button className="activity__button activity__button--primary" type="submit">{text.applyFilters}</button>
            <button className="activity__button" type="button" onClick={clearFilters}>{text.resetFilters}</button>
          </div>
        </form>
      </details>

      {props.state.kind === "loading" && <p className="activity__notice" role="status">{text.loading}</p>}
      {props.state.kind === "unavailable" && <p className="activity__notice" role="status">{text.unavailable}</p>}
      {props.state.kind === "error" && <p className="activity__notice activity__notice--error" role="alert">{errorText(props.state.error, text)}</p>}
      {props.state.kind === "ready" && props.state.stale && <p className="activity__notice" role="status">
        {text.stale} · {formatTime(props.state.snapshot.observed_at_utc, props.language)}{props.state.issue ? ` · ${errorText(props.state.issue, text)}` : ""}
      </p>}

      {data && <>
        <div className="activity__coverage" role="note">
          <span>{formatMetricNumber(data.shown_requests, props.language)} {text.shown} / {formatMetricNumber(data.matched_requests, props.language)} {text.matched}</span>
          {data.has_more && <span>{text.capped}</span>}
          {data.unplaced_requests > 0 && <span>{formatMetricNumber(data.unplaced_requests, props.language)} {text.unplaced} {text.unplacedNote}</span>}
          {data.coverage.rolling_window && <span>{props.language === "tr" ? "Kaynak son isteklerin kayan penceresidir; bazı kayıtlar gözlenmemiş olabilir." : "The source is a rolling window of recent requests; some records may be unobserved."}</span>}
          {snapshot?.collection.sampling_interrupted_since_start && <span>{props.language === "tr" ? "Ölçüm toplamada kesinti oldu; geçmiş kayıtlar korunuyor." : "Collection was interrupted; recorded history is preserved."}</span>}
        </div>
        {data.rows.length === 0 ? <p className="activity__empty">{text.empty}</p> : (
          <ul className="activity__rows" aria-label={text.requests}>
            {data.rows.map((row) => (
              <li key={row.origin_id}>
                <button className="activity__row" type="button" onClick={() => props.onSelect(row.origin_id)} aria-haspopup="dialog">
                  <span className="activity__row-main">
                    <strong>{clientName(row.identity.client, text)} · {row.identity.model ?? text.modelUnknown}</strong>
                    <small>{row.identity.provider ? `${text.provider}: ${row.identity.provider} · ` : ""}{agentName(row, text)}{row.identity.conflicted ? ` · ${text.attributionConflict}` : ""}{row.metadata.conflicted ? ` · ${text.metadataConflict}` : ""}</small>
                  </span>
                  <span><small>{text.time}</small><strong>{formatTime(row.timestamp_utc, props.language)}</strong></span>
                  <span><small>{text.duration}</small><strong>{row.metadata.total_ms === null ? text.unknown : `${formatMetricNumber(row.metadata.total_ms, props.language)} ms`}</strong></span>
                  <span><small>{text.result}</small><strong>{row.metadata.outcome === "error_reported" ? text.errorReported : row.metadata.outcome === "no_error_reported" ? text.noError : text.outcomeUnknown}</strong></span>
                  <span><small>{text.input}</small><strong>{changeText(row, props.language, text)}</strong></span>
                  <span className="activity__row-id">{text.requestId} · {row.origin_id.slice(0, 8)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </>}

      <RequestDetails language={props.language} period={props.period} selectedId={props.selectedId} detail={props.detail} onClose={() => props.onSelect(null)} />
    </section>
  );
}
