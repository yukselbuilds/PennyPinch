import "./overview.css";
import { SavingsChart } from "./SavingsChart";
import { Coverage } from "./Coverage";
import {
  formatMetricNumber,
  formatMetricPercent,
  type MetricPeriod,
  type MetricsUiError,
  type OverviewConnection,
  type OverviewProps,
  type OverviewSnapshot,
} from "../../lib/metrics";
import type { ClientKind } from "../../lib/integrations";
import type { EngineStatus } from "../../runtime/snapshot";

const copy = {
  tr: {
    section: "GENEL BAKIŞ",
    heading: "Girdi kullanımı",
    period: "Dönem",
    periods: { session: "Oturum", today: "Bugün", seven_days: "7 gün" },
    saved: "Ölçülen girdi azalması",
    inputReduction: "girdi azalması",
    inputIncrease: "girdi artışı",
    tokensSaved: "token azaltımı",
    tokensIncreased: "token artışı",
    from: "önce",
    to: "sonra",
    measuredScope: "Yüzde ve miktar aynı ölçüm kapsamından hesaplanır.",
    noMeasurements: "Bu dönemde ölçülebilir kayıt yok.",
    noComparable: "İstekler kaydediliyor; tasarruf göstermek için yeterli karşılaştırılabilir ölçüm yok.",
    mixed: "Ölçümler farklı temellere dayanıyor. Ayrı gruplar tek toplama birleştirilmiyor.",
    overflow: "Ölçüm sayacı güvenli sınırı aştı; toplam gösterilmiyor.",
    zeroBaseline: "Başlangıç değeri sıfır olduğu için yüzde hesaplanamıyor.",
    partial: "Bu özet kaydedilen ölçümlerle sınırlıdır; tüm trafiği temsil etmeyebilir.",
    stale: "Önceki ölçüm görüntüleniyor",
    captured: "Ölçüm zamanı",
    loading: "Ölçümler okunuyor",
    unavailable: "Bu masaüstü oturumunda ölçüm kullanılamıyor.",
    failed: "Ölçüm okunamadı.",
    retry: "Yeniden dene",
    refreshing: "Güncelleniyor…",
    refresh: "Veriyi yenile",
    engine: "Motor durumu",
    engineReady: "Hazır",
    engineStopped: "Durduruldu",
    engineStarting: "Başlatılıyor",
    engineStopping: "Durduruluyor",
    engineFailed: "Hata bildirdi",
    engineUnavailable: "Kullanılamıyor",
    engineUnknown: "Bilinmiyor",
    engineReadyHint: "Motor hazır; istemci trafiği ayrı ölçülür.",
    engineRequirement: "Bağlantıdan sonra kodlama uygulamanı kullanmaya devam edebilirsin.",
    noSavedConnection: "Henüz yönlendirme kaydı yok.",
    savedConnection: "Headroom ayar kaydı var; açık istemci ve trafik durumu ayrıca değerlendirilir.",
    connectAndStart: "Bağla ve başlat",
    manage: "Uygulamaları yönet",
    start: "Başlat",
    starting: "Başlatılıyor…",
    stop: "Motoru durdur",
    running: "Çalışıyor",
    more: "Diğer motor işlemleri",
    connections: "Uygulama bağlantıları",
    connectionSaved: "Yönlendirme kaydı var",
    connectionMissing: "Yönlendirme kaydı yok",
    connectionUnknown: "Kayıt durumu bilinmiyor",
    installationFound: "Kurulum bulundu",
    installationNotFound: "Bu taramada kurulum bulunmadı",
    installationUnknown: "Kurulum keşif durumu bilinmiyor",
    recordLoading: "Bağlantı kaydı okunuyor",
    connect: "Bağla",
    retention: "Saklama",
    days: "gün",
    lastCapture: "Son yakalama",
    lastEvent: "Son ölçülen olay",
    collection: "Kayıt durumu",
    collectionRecording: "Ölçüm kaydı sürüyor",
    collectionWaiting: "Ölçüm kaydı bekliyor",
    collectionStopped: "Ölçüm kaydı durdu",
    collectionInterrupted: "Ölçüm kaydı kesintiye uğradı",
    collectionUnavailable: "Ölçüm kaydı kullanılamıyor",
    collectionErrors: {
      busy: "Ölçüm toplama işlemi meşguldü.",
      unavailable: "Motorun ölçüm kaynağı kullanılamıyordu.",
      ownership_lost: "Motorun ölçüm kaynağının sahipliği doğrulanamadı.",
      transport: "Ölçüm kaynağına ulaşılamadı.",
      invalid_snapshot: "Ölçüm yanıtı doğrulanamadı.",
      storage: "Ölçüm kaydı yazılamadı.",
    } satisfies Record<OverviewSnapshot["collection"]["last_error"] & string, string>,
    recordedEvents: "Yerel ölçüm kayıtları",
    comparableEvents: "Karşılaştırılabilir",
    excludedEvents: "Hariç tutulan",
    sourceNote: "Kaynak: Headroom’un yerel ölçüm kayıtları; sohbet veya hesap geçmişi değil.",
    samplingInterrupted: "Başlangıçtan beri örnekleme kesintisi",
    yes: "Evet",
    no: "Hayır",
    chart: "Dönem içindeki ölçümler",
    chartEmpty: "Karşılaştırılabilir zaman kovası henüz yok.",
    chartSaved: "Girdi azalması",
    chartIncrease: "Girdi artışı",
    chartDescription: "Çubuklar yalnızca gerçek ölçüm kovalarını gösterir; aradaki zamanlar sıfır kabul edilmez.",
    metricErrors: {
      invalid_input: "Ölçüm isteği doğrulanamadı.",
      unsafe_path: "Ölçüm deposu güvenli bir konumda değil.",
      busy: "Ölçüm deposu meşgul. Biraz sonra yeniden dene.",
      unsupported_schema: "Ölçüm verisinin biçimi desteklenmiyor.",
      corrupt: "Ölçüm kaydı doğrulanamadı; dosya korunuyor.",
      capacity: "Yerel ölçüm saklama sınırına ulaşıldı.",
      unavailable: "Ölçüm deposu kullanılamıyor.",
      invalid_local_time: "Yerel dönem sınırı doğrulanamadı.",
      invalid_response: "Ölçüm yanıtı doğrulanamadı.",
      request_failed: "Ölçüm isteği tamamlanamadı.",
    } satisfies Record<MetricsUiError, string>,
  },
  en: {
    section: "OVERVIEW",
    heading: "Input usage",
    period: "Period",
    periods: { session: "Session", today: "Today", seven_days: "7 days" },
    saved: "Measured input reduction",
    inputReduction: "input reduction",
    inputIncrease: "input increase",
    tokensSaved: "token reduction",
    tokensIncreased: "token increase",
    from: "before",
    to: "after",
    measuredScope: "Percentage and amount use the same measurement scope.",
    noMeasurements: "No measurable records in this period.",
    noComparable: "Requests are recorded; there is not enough comparable measurement to show savings.",
    mixed: "Measurements use different bases. Separate groups are not combined.",
    overflow: "A measurement counter exceeded the safe limit; no total is shown.",
    zeroBaseline: "A percentage is unavailable because the starting value is zero.",
    partial: "This summary covers recorded measurements and may not represent all traffic.",
    stale: "Showing the previous measurement",
    captured: "Measurement time",
    loading: "Reading measurements",
    unavailable: "Measurements are unavailable in this desktop session.",
    failed: "Measurements could not be read.",
    retry: "Try again",
    refreshing: "Updating…",
    refresh: "Refresh data",
    engine: "Engine status",
    engineReady: "Ready",
    engineStopped: "Stopped",
    engineStarting: "Starting",
    engineStopping: "Stopping",
    engineFailed: "Reported an error",
    engineUnavailable: "Unavailable",
    engineUnknown: "Unknown",
    engineReadyHint: "The engine is ready; client traffic is measured separately.",
    engineRequirement: "After connecting, keep working in your coding application.",
    noSavedConnection: "No routing setting is saved yet.",
    savedConnection: "A Headroom setting is saved; open client and traffic status are separate.",
    connectAndStart: "Connect and start",
    manage: "Manage applications",
    start: "Start",
    starting: "Starting…",
    stop: "Stop engine",
    running: "Running",
    more: "More engine actions",
    connections: "Application connections",
    connectionSaved: "Routing setting saved",
    connectionMissing: "No routing setting saved",
    connectionUnknown: "Record status unknown",
    installationFound: "Installation found",
    installationNotFound: "No installation found in this scan",
    installationUnknown: "Installation discovery status unknown",
    recordLoading: "Reading connection record",
    connect: "Connect",
    retention: "Retention",
    days: "days",
    lastCapture: "Last capture",
    lastEvent: "Last measured event",
    collection: "Collection status",
    collectionRecording: "Measurement collection is recording",
    collectionWaiting: "Measurement collection is waiting",
    collectionStopped: "Measurement collection stopped",
    collectionInterrupted: "Measurement collection was interrupted",
    collectionUnavailable: "Measurement collection is unavailable",
    collectionErrors: {
      busy: "Measurement collection was busy.",
      unavailable: "The engine measurement source was unavailable.",
      ownership_lost: "Engine measurement source ownership could not be confirmed.",
      transport: "The measurement source could not be reached.",
      invalid_snapshot: "The measurement response could not be validated.",
      storage: "The measurement record could not be written.",
    } satisfies Record<OverviewSnapshot["collection"]["last_error"] & string, string>,
    recordedEvents: "Local measurement records",
    comparableEvents: "Comparable",
    excludedEvents: "Excluded",
    sourceNote: "Source: Headroom’s local measurement records, not chat or account history.",
    samplingInterrupted: "Sampling interrupted since start",
    yes: "Yes",
    no: "No",
    chart: "Measurements over time",
    chartEmpty: "There are no comparable time buckets yet.",
    chartSaved: "Input reduction",
    chartIncrease: "Input increase",
    chartDescription: "Bars show only actual measurement buckets; gaps in time are not treated as zero.",
    metricErrors: {
      invalid_input: "The measurement request could not be validated.",
      unsafe_path: "The measurement store is not in a safe location.",
      busy: "The measurement store is busy. Try again shortly.",
      unsupported_schema: "The measurement data format is unsupported.",
      corrupt: "The measurement record could not be validated; the file is preserved.",
      capacity: "The local measurement retention limit was reached.",
      unavailable: "The measurement store is unavailable.",
      invalid_local_time: "The local period boundary could not be validated.",
      invalid_response: "The measurement response could not be validated.",
      request_failed: "The measurement request could not be completed.",
    } satisfies Record<MetricsUiError, string>,
  },
} as const;

type OverviewCopy = {
  [Key in keyof typeof copy.tr]: typeof copy.tr[Key] extends Record<string, unknown>
    ? { [NestedKey in keyof typeof copy.tr[Key]]: string }
    : string;
};

const clients: readonly { kind: ClientKind; label: string }[] = [
  { kind: "codex", label: "Codex" },
  { kind: "claude_code", label: "Claude Code" },
  { kind: "opencode", label: "OpenCode" },
];

function formatTime(value: string | null, language: OverviewProps["language"]): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return null;
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function engineText(status: EngineStatus, t: OverviewCopy): string {
  switch (status) {
    case "ready": return t.engineReady;
    case "stopped": return t.engineStopped;
    case "starting": return t.engineStarting;
    case "stopping": return t.engineStopping;
    case "failed": return t.engineFailed;
    case "unavailable": return t.engineUnavailable;
    default: return t.engineUnknown;
  }
}

function collectionText(snapshot: OverviewSnapshot, t: OverviewCopy): string {
  switch (snapshot.collection.status) {
    case "recording": return t.collectionRecording;
    case "waiting": return t.collectionWaiting;
    case "stopped": return t.collectionStopped;
    case "interrupted": return t.collectionInterrupted;
    case "unavailable": return t.collectionUnavailable;
  }
}

function metricMessage(
  metrics: OverviewProps["metrics"],
  language: OverviewProps["language"],
  t: OverviewCopy,
): string {
  if (metrics.kind === "loading") return t.loading;
  if (metrics.kind === "unavailable") return t.unavailable;
  if (metrics.kind === "error") return t.metricErrors[metrics.error];
  if (metrics.stale) {
    const time = formatTime(metrics.snapshot.observed_at_utc, language);
    const issue = metrics.issue ? t.metricErrors[metrics.issue] : null;
    return `${t.stale}${time ? ` · ${t.captured}: ${time}` : ""}${issue ? ` · ${issue}` : ""}`;
  }
  const { totals, coverage } = metrics.snapshot.data;
  if (totals.status === "mixed_bases") return t.mixed;
  if (totals.status === "counter_overflow") return t.overflow;
  if (totals.status === "no_comparable_events") {
    return coverage.events > 0 ? t.noComparable : t.noMeasurements;
  }
  if (totals.before_tokens === 0 && totals.saved_tokens !== null && totals.saved_percent === null) return t.zeroBaseline;
  return t.measuredScope;
}

function connectionText(connection: OverviewConnection | undefined, loading: boolean, t: OverviewCopy): string {
  if (loading) return t.recordLoading;
  if (!connection || connection.state === "unknown") return t.connectionUnknown;
  if (connection.state === "saved") return t.connectionSaved;
  if (connection.detected === null) return t.installationUnknown;
  return t.connectionMissing;
}

export function Overview(props: OverviewProps) {
  const t = copy[props.language];
  const snapshot = props.metrics.kind === "ready" ? props.metrics.snapshot : null;
  const totals = snapshot?.data.totals;
  const measuredTotals = totals?.status === "measured" ? totals : null;
  const saved = measuredTotals?.saved_tokens ?? null;
  const percent = measuredTotals?.saved_percent ?? null;
  const isIncrease = saved !== null && saved < 0;
  const engineStatus = props.runtime?.engine.status ?? "unknown";
  const hasSavedConnection = props.connections.some((connection) => connection.state === "saved");
  const connectionRecordsKnown = clients.every((client) =>
    props.connections.some((connection) => connection.client === client.kind && connection.state !== "unknown"),
  );
  const engineReady = engineStatus === "ready";
  const showStart = hasSavedConnection && !engineReady && Boolean(props.runtime?.capabilities.start);
  const periodOptions: readonly MetricPeriod[] = ["session", "today", "seven_days"];
  const number = (value: number | null) => formatMetricNumber(value, props.language);

  return (
    <section className="overview" aria-label={props.language === "tr" ? "Genel bakış" : "Overview"}>
      <div className="overview__toolbar">
        <div>
          <div className="overview__eyebrow">{t.section}</div>
          <h2>{t.heading}</h2>
        </div>
        <div className="overview__period" role="group" aria-label={t.period}>
          {periodOptions.map((period) => (
            <button
              key={period}
              type="button"
              aria-pressed={props.period === period}
              onClick={() => props.onPeriodChange(period)}
            >
              {t.periods[period]}
            </button>
          ))}
        </div>
      </div>

      <div className="overview__hero">
        <section className="overview__metric" aria-labelledby="overview-metric-title">
          <div className="overview__metric-label" id="overview-metric-title">{t.saved}</div>
          <div className={`overview__percent${saved !== null && saved < 0 ? " is-increase" : ""}`}>
            <strong>{formatMetricPercent(isIncrease && percent !== null ? Math.abs(percent) : percent, props.language)}</strong>
            <span>{isIncrease ? t.inputIncrease : t.inputReduction}</span>
          </div>
          <div className="overview__amount-row">
            <strong>{number(saved === null ? null : Math.abs(saved))}</strong>
            <span>{saved !== null && saved < 0 ? t.tokensIncreased : t.tokensSaved}</span>
          </div>
          <p className="overview__metric-message">{metricMessage(props.metrics, props.language, t)}</p>
          {snapshot && !snapshot.data.coverage.complete_traffic && (
            <p className="overview__scope-note"><span aria-hidden="true">i</span>{t.partial}</p>
          )}
          {snapshot && snapshot.data.totals.before_tokens !== null && snapshot.data.totals.after_tokens !== null && (
            <div className="overview__range">
              <span><small>{t.from}</small><strong>{number(snapshot.data.totals.before_tokens)}</strong></span>
              <span className="overview__range-arrow" aria-hidden="true">→</span>
              <span><small>{t.to}</small><strong>{number(snapshot.data.totals.after_tokens)}</strong></span>
            </div>
          )}
          {props.metrics.kind === "error" && (
            <button className="overview__text-button" type="button" onClick={props.onRefresh}>
              {t.retry}
            </button>
          )}
        </section>

        <aside className="overview__engine" aria-label={t.engine}>
          <div className="overview__engine-status">
            <span className={`overview__status-dot is-${engineStatus}`} aria-hidden="true" />
            <div>
              <span>{t.engine}</span>
              <strong>{engineText(engineStatus, t)}</strong>
            </div>
          </div>
          <p>{engineReady ? t.engineReadyHint : hasSavedConnection ? t.savedConnection : connectionRecordsKnown ? t.noSavedConnection : t.connectionUnknown}</p>
          {!hasSavedConnection && connectionRecordsKnown && !props.connectionsLoading ? (
            <button className="overview__primary" type="button" onClick={props.onOpenApplications}>
              {engineReady ? t.connect : t.connectAndStart}<span aria-hidden="true">↗</span>
            </button>
          ) : props.connectionsLoading || !connectionRecordsKnown ? (
            <button className="overview__primary" type="button" onClick={props.onOpenApplications}>{t.manage}</button>
          ) : showStart ? (
            <button className="overview__primary" type="button" disabled={props.pending !== null} onClick={props.onStart}>
              {props.pending === "start" ? t.starting : t.start}<span aria-hidden="true">▶</span>
            </button>
          ) : engineReady ? (
            <div className="overview__running">{t.running}</div>
          ) : (
            <button className="overview__primary overview__primary--quiet" type="button" onClick={props.onOpenApplications}>{t.manage}</button>
          )}
          <p className="overview__engine-requirement">{t.engineRequirement}</p>
          {engineReady && props.runtime?.capabilities.stop && (
            <details className="overview__engine-more">
              <summary>{t.more}</summary>
              <button type="button" disabled={props.pending !== null} onClick={props.onStop}>
                {props.pending === "stop" ? (props.language === "tr" ? "Durduruluyor…" : "Stopping…") : t.stop}
              </button>
            </details>
          )}
        </aside>
      </div>

      {snapshot && snapshot.data.series.length > 0 && <section className="overview__chart-panel" aria-labelledby="overview-chart-title">
        <div className="overview__section-heading">
          <div>
            <h3 id="overview-chart-title">{t.chart}</h3>
            <p>{t.chartDescription}</p>
          </div>
          {props.metrics.kind === "ready" && (
            <button className="overview__text-button" type="button" disabled={props.metrics.refreshing} onClick={props.onRefresh}>
              {props.metrics.refreshing ? t.refreshing : t.refresh}
            </button>
          )}
        </div>
          <SavingsChart
            language={props.language}
            buckets={snapshot.data.series}
            fromUtc={snapshot.data.from_utc}
            toUtc={snapshot.data.to_utc}
            emptyLabel={t.chartEmpty}
            savedLabel={t.chartSaved}
            increaseLabel={t.chartIncrease}
          />
      </section>}

      {snapshot && <Coverage language={props.language} summary={snapshot.data.attribution}/>}

      <section className="overview__connections" aria-labelledby="overview-connections-title">
        <div className="overview__section-heading">
          <div>
            <h3 id="overview-connections-title">{t.connections}</h3>
            <p>{props.language === "tr" ? "Ayar kaydı, istemcinin kullandığı veya trafik ürettiği anlamına gelmez." : "A saved setting does not mean a client is open or sending traffic."}</p>
          </div>
        </div>
        <ul>
          {clients.map(({ kind, label }) => {
            const connection = props.connections.find((candidate) => candidate.client === kind);
            return (
              <li key={kind}>
                <span className="overview__client-mark" aria-hidden="true">{label === "Claude Code" ? "C" : label === "OpenCode" ? "O" : "X"}</span>
                <strong>{label}</strong>
                <span className={`overview__connection-state is-${connection?.state ?? "unknown"}`}>
                  {connectionText(connection, props.connectionsLoading, t)}
                  {connection && connection.state !== "unknown" && (
                    <small>{connection.detected === null ? t.installationUnknown : connection.detected ? t.installationFound : t.installationNotFound}</small>
                  )}
                </span>
                <button className="overview__connect-link" type="button" onClick={() => connection?.detected === false ? props.onOpenApplications() : props.onConnect(kind)}>
                  {connection?.state === "saved" ? t.manage : connection?.detected === false ? (props.language === "tr" ? "Bul / ekle" : "Find / add") : t.connect}
                </button>
              </li>
            );
          })}
        </ul>
      </section>

      {snapshot && (
        <footer className="overview__metadata">
          <span><strong>{t.collection}</strong>{collectionText(snapshot, t)}</span>
          <span><strong>{t.retention}</strong>{snapshot.data.retention_days} {t.days}</span>
          <span><strong>{t.lastCapture}</strong>{formatTime(snapshot.collection.last_capture_utc, props.language) ?? "—"}</span>
          {snapshot.collection.sampling_interrupted_since_start && <span className="overview__metadata-warning">{t.samplingInterrupted}</span>}
          {snapshot.collection.last_error && (
            <span className="overview__metadata-warning">{t.collectionErrors[snapshot.collection.last_error]}</span>
          )}
          <details className="overview__measurement-details"><summary>{props.language === "tr" ? "Ölçüm ayrıntıları" : "Measurement details"}</summary>
          <div className="overview__metadata">
          <span><strong>{t.lastEvent}</strong>{formatTime(snapshot.data.last_event_utc, props.language) ?? "—"}</span>
          <span><strong>{t.recordedEvents}</strong>{formatMetricNumber(snapshot.data.coverage.events, props.language)}</span>
          <span><strong>{t.comparableEvents}</strong>{formatMetricNumber(snapshot.data.totals.comparable_events, props.language)}</span>
          <span><strong>{t.excludedEvents}</strong>{formatMetricNumber(snapshot.data.totals.excluded_events, props.language)}</span>
          <span><strong>{t.samplingInterrupted}</strong>{snapshot.collection.sampling_interrupted_since_start ? t.yes : t.no}</span>
          <span className="overview__metadata-source">{t.sourceNote}</span>
          </div></details>
        </footer>
      )}
    </section>
  );
}
