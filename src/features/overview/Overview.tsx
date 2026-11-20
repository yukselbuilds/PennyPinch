import "./overview.css";
import { SavingsChart } from "./SavingsChart";
import { Coverage } from "./Coverage";
import {OptimizationControl} from "./OptimizationControl";
import {optimizationView} from "../../lib/optimization";
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
import { Icon } from "../../components/Icon";
import { ExportButton } from "./ExportButton";
import { Recommendations } from "./Recommendations";

const copy = {
  tr: {
    section: "Genel bakış",
    heading: "Kullanım özeti",
    period: "Dönem",
    periods: { session: "Oturum", today: "Bugün", seven_days: "7 gün" },
    saved: "Girdi tasarrufu",
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
    refresh: "Yenile",
    manage: "Yönet",
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
    sourceNote: "Kaynak: PennyPinch’in yerel ölçüm kayıtları; sohbet veya hesap geçmişi değil.",
    samplingInterrupted: "Başlangıçtan beri örnekleme kesintisi",
    yes: "Evet",
    no: "Hayır",
    chart: "Token değişimi",
    chartEmpty: "Karşılaştırılabilir zaman kovası henüz yok.",
    chartSaved: "Girdi azalması",
    chartIncrease: "Girdi artışı",
    chartDescription: "Kaydedilen ölçümler. Veri olmayan aralıklar dahil edilmez.",
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
    section: "Overview",
    heading: "Usage summary",
    period: "Period",
    periods: { session: "Session", today: "Today", seven_days: "7 days" },
    saved: "Input savings",
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
    refresh: "Refresh",
    manage: "Manage",
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
    sourceNote: "Source: PennyPinch’s local measurement records, not chat or account history.",
    samplingInterrupted: "Sampling interrupted since start",
    yes: "Yes",
    no: "No",
    chart: "Token trend",
    chartEmpty: "There are no comparable time buckets yet.",
    chartSaved: "Input reduction",
    chartIncrease: "Input increase",
    chartDescription: "Recorded measurements. Unobserved intervals stay empty.",
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
  const control=optimizationView(props.runtime,props.pending);
  const controlAction=()=>({start:props.onStart,pause:props.onPause,resume:props.onResume,refresh:props.onRuntimeRefresh})[control.primary]();
  const periodOptions: readonly MetricPeriod[] = ["session", "today", "seven_days"];
  const number = (value: number | null) => formatMetricNumber(value, props.language);
  const empty = measuredTotals === null;
  const emptyTitle = props.metrics.kind === "loading"
    ? (props.language === "tr" ? "Ölçümler yükleniyor" : "Loading measurements")
    : props.metrics.kind !== "ready"
      ? (props.language === "tr" ? "Ölçümler kullanılamıyor" : "Measurements unavailable")
      : snapshot!.data.coverage.events === 0
        ? (props.language === "tr" ? "İlk ölçümünü bekliyor" : "Your first measurement awaits")
        : (props.language === "tr" ? "Karşılaştırma bekleniyor" : "Awaiting a comparison");

  return (
    <section className="overview" aria-label={props.language === "tr" ? "Genel bakış" : "Overview"}>
      <div className="overview__toolbar">
        <div>
          <h2>{t.heading}</h2>
        </div>
        <div className="overview__actions">
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
        <ExportButton language={props.language} snapshotRequestId={snapshot?.request_id ?? null} />
        </div>
      </div>

      <div className={`overview__hero${empty ? " is-empty" : ""}`}>
        <section className="overview__metric" aria-labelledby="overview-metric-title">
          <div className="overview__metric-label" id="overview-metric-title"><Icon name="chart" />{t.saved}</div>
          <div className={`overview__percent${isIncrease ? " is-increase" : ""}${empty ? " is-empty" : ""}`}>
            <strong>{empty ? emptyTitle : formatMetricPercent(isIncrease && percent !== null ? Math.abs(percent) : percent, props.language)}</strong>
            {!empty && <span>{isIncrease ? t.inputIncrease : t.inputReduction}</span>}
          </div>
          {!empty && <div className="overview__amount-row">
            <strong>{number(saved === null ? null : Math.abs(saved))}</strong>
            <span>{saved !== null && saved < 0 ? t.tokensIncreased : t.tokensSaved}</span>
          </div>}
          <p className="overview__metric-message">{metricMessage(props.metrics, props.language, t)}</p>
          {snapshot && snapshot.data.coverage.events > 0 && !snapshot.data.coverage.complete_traffic && (
            <p className="overview__scope-note"><span aria-hidden="true">i</span>{t.partial}</p>
          )}
          {snapshot && (!empty || snapshot.data.coverage.events > 0) && <div className="overview__range">
              <span><small>{t.from}</small><strong>{number(snapshot.data.totals.before_tokens)}</strong></span>
              <span className="overview__range-arrow" aria-hidden="true">→</span>
              <span><small>{t.to}</small><strong>{number(snapshot.data.totals.after_tokens)}</strong></span>
              <span className="overview__request-count"><small>{props.language === "tr" ? "Kaydedilen istek" : "Recorded requests"}</small><strong>{number(snapshot.data.coverage.distinct_origins)}</strong></span>
            </div>}
          {props.metrics.kind === "error" && (
            <button className="overview__text-button" type="button" onClick={props.onRefresh}>
              {t.retry}
            </button>
          )}
          {snapshot && snapshot.data.coverage.events === 0 && <button className="overview__empty-action" type="button" onClick={props.onOpenApplications}>
            {props.language === "tr" ? "Uygulamalarına göz at" : "Explore your applications"}<Icon name="arrow" />
          </button>}
        </section>

        <OptimizationControl language={props.language} view={control} onPrimary={controlAction} onStop={props.onStop}/>
      </div>

      <div className="overview__content-grid">
      <section className="overview__chart-panel" aria-labelledby="overview-chart-title">
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
        {snapshot && snapshot.data.series.length > 0 ? <SavingsChart
            language={props.language}
            buckets={snapshot.data.series}
            fromUtc={snapshot.data.from_utc}
            toUtc={snapshot.data.to_utc}
            emptyLabel={t.chartEmpty}
            savedLabel={t.chartSaved}
            increaseLabel={t.chartIncrease}
        /> : <div className="overview__chart-empty"><Icon name="activity" /><strong>{props.language === "tr" ? "Trafik geldikçe şekillenecek" : "Your traffic will take shape here"}</strong><span>{props.language === "tr" ? "Kaydedilen ölçümler bu alanda görünecek." : "Recorded measurements will appear in this space."}</span></div>}
      </section>

      <section className="overview__connections" aria-labelledby="overview-connections-title">
        <div className="overview__section-heading">
          <div>
            <h3 id="overview-connections-title">{t.connections}</h3>
            <p>{props.language === "tr" ? "Kayıtlı ayarlar canlı trafik anlamına gelmez." : "Saved settings do not confirm live traffic."}</p>
          </div>
        </div>
        <ul>
          {clients.map(({ kind, label }) => {
            const connection = props.connections.find((candidate) => candidate.client === kind);
            return (
              <li key={kind}>
                <span className={`overview__client-mark is-${kind}`} aria-hidden="true"><Icon name={kind === "codex" ? "terminal" : kind === "claude_code" ? "asterisk" : "code"}/></span>
                <strong>{label}</strong>
                <span className={`overview__connection-state is-${connection?.state ?? "unknown"}`}>
                  {connectionText(connection, props.connectionsLoading, t)}
                  {connection && connection.state !== "unknown" && (
                    <small>{connection.detected === null ? t.installationUnknown : connection.detected ? t.installationFound : t.installationNotFound}</small>
                  )}
                </span>
                <button className="overview__connect-link" type="button" onClick={() => connection?.detected === false ? props.onOpenApplications() : props.onConnect(kind)}>
                  {connection?.state === "saved" ? t.manage : connection?.detected === false ? (props.language === "tr" ? "Bul / ekle" : "Find / add") : t.connect}
                  <Icon name="chevron" />
                </button>
              </li>
            );
          })}
        </ul>
      </section>
      </div>

      {props.showRecommendations && props.metrics.kind === "ready" && !props.metrics.stale && snapshot && (
        <Recommendations items={snapshot.recommendations} language={props.language} period={props.period}
          observedAt={snapshot.observed_at_utc} onOpenActivity={props.onOpenActivity} onOpenApplications={props.onOpenApplications} />
      )}
      {snapshot && <Coverage language={props.language} summary={snapshot.data.attribution}/>}

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
