import type { MetricBucket } from "../../lib/metrics";
import { formatMetricNumber } from "../../lib/metrics";
import type { PreferenceLanguage } from "../../lib/preferences";

interface SavingsChartProps {
  language: PreferenceLanguage;
  buckets: readonly MetricBucket[];
  fromUtc: string | null;
  toUtc: string;
  emptyLabel: string;
  savedLabel: string;
  increaseLabel: string;
}

const WIDTH = 720;
const HEIGHT = 172;
const LEFT = 18;
const RIGHT = WIDTH - 18;
const BASELINE = 83;
const MAX_BAR_HEIGHT = 53;

function timestamp(value: string | null): number | null {
  if (!value) return null;
  const result = Date.parse(value);
  return Number.isFinite(result) ? result : null;
}

function timeLabel(value: string | null, language: PreferenceLanguage): string {
  const parsed = timestamp(value);
  if (parsed === null) return "";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", {
    month: "short",
    day: "numeric",
  }).format(parsed);
}

function fullTimeLabel(value: string | null, language: PreferenceLanguage): string {
  const parsed = timestamp(value);
  if (parsed === null) return "";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}

export function SavingsChart({
  language,
  buckets,
  fromUtc,
  toUtc,
  emptyLabel,
  savedLabel,
  increaseLabel,
}: SavingsChartProps) {
  const points = buckets.flatMap((bucket) => {
    const value = bucket.totals.status === "measured" ? bucket.totals.saved_tokens : null;
    const time = timestamp(bucket.at_utc);
    return value === null ? [] : [{ at: bucket.at_utc, time, value }];
  });
  if (points.length === 0) {
    return <div className="overview__chart-empty">{emptyLabel}</div>;
  }

  const observedTimes = points.flatMap((point) => point.time === null ? [] : [point.time]);
  const boundaryStart = timestamp(fromUtc);
  const boundaryEnd = timestamp(toUtc);
  const start = boundaryStart ?? (observedTimes.length ? Math.min(...observedTimes) : null);
  const end = boundaryEnd ?? (observedTimes.length ? Math.max(...observedTimes) : null);
  const span = start !== null && end !== null ? end - start : 0;
  const maxValue = points.reduce((max, point) => Math.max(max, Math.abs(point.value)), 1);
  const barWidth = Math.min(28, Math.max(5, (RIGHT - LEFT) / Math.max(1.8 * points.length, 1)));

  const xFor = (index: number, pointTime: number | null) => {
    if (span > 0 && pointTime !== null && start !== null) {
      const ratio = Math.min(1, Math.max(0, (pointTime - start) / span));
      return LEFT + ratio * (RIGHT - LEFT);
    }
    if (points.length === 1) return (LEFT + RIGHT) / 2;
    return LEFT + (index / (points.length - 1)) * (RIGHT - LEFT);
  };
  const formatValue = (value: number) => {
    const label = value < 0 ? increaseLabel : savedLabel;
    return `${formatMetricNumber(Math.abs(value), language)} ${label}`;
  };
  const startLabel = timeLabel(fromUtc ?? (points[0]?.at ?? null), language);
  const endLabel = timeLabel(toUtc, language);

  return (
    <div className="overview__chart">
      <div className="overview__chart-legend" aria-hidden="true">
        <span><i className="is-saved" />{savedLabel}</span>
        <span><i className="is-increase" />{increaseLabel}</span>
      </div>
      <svg
        className="overview__chart-svg"
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={language === "tr" ? "Gerçek ölçüm kovalarındaki token değişimi" : "Token changes in actual measurement buckets"}
      >
        <line x1={LEFT} x2={RIGHT} y1={BASELINE} y2={BASELINE} className="overview__chart-baseline" />
        {points.map((point, index) => {
          const x = xFor(index, point.time);
          const height = point.value === 0 ? 2 : Math.max(2, Math.abs(point.value) / maxValue * MAX_BAR_HEIGHT);
          const y = point.value < 0 ? BASELINE : BASELINE - height;
          return (
            <rect
              key={`${point.at}-${index}`}
              x={x - barWidth / 2}
              y={y}
              width={barWidth}
              height={height}
              rx="2"
              className={point.value < 0 ? "is-increase" : "is-saved"}
            >
              <title>{`${fullTimeLabel(point.at, language)} · ${formatValue(point.value)}`}</title>
            </rect>
          );
        })}
        <text x={LEFT} y="162" textAnchor="start" className="overview__chart-axis-label">{startLabel}</text>
        <text x={RIGHT} y="162" textAnchor="end" className="overview__chart-axis-label">{endLabel}</text>
      </svg>
      <ol className="overview__chart-sr-list" aria-label={language === "tr" ? "Ölçüm değerleri" : "Measurement values"}>
        {points.map((point, index) => (
          <li key={`${point.at}-${index}`}>{`${fullTimeLabel(point.at, language)}: ${formatValue(point.value)}`}</li>
        ))}
      </ol>
    </div>
  );
}
