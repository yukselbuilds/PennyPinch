import {
  formatMetricNumber,
  formatMetricPercent,
  type MetricPeriod,
} from "../../lib/metrics";
import type { PreferenceLanguage } from "../../lib/preferences";
import type { Recommendation } from "../../lib/insights";
import type { ClientKind } from "../../lib/integrations";
import { Icon } from "../../components/Icon";

type Props = {
  items: readonly Recommendation[];
  language: PreferenceLanguage;
  period: MetricPeriod;
  observedAt: string;
  onOpenActivity: () => void;
  onOpenApplications: () => void;
};

const copy = {
  tr: {
    heading: "Tasarruf ipuçları",
    introduction: "Son ölçümlerde dikkat çeken noktalar.",
    periods: { session: "Bu oturum", today: "Bugün", seven_days: "Son 7 gün" },
    observedAt: "Gözlem zamanı",
    inputTitle: "Girdi miktarında artış ölçüldü",
    inputScope: { message_input: "Mesaj girdisi", whole_input: "Toplam girdi" },
    bases: { local_estimate: "Yerel tahmin", provider_tokenizer: "Sağlayıcı tokenizer sayımı" },
    events: "ölçüm",
    before: "Önce",
    after: "Sonra",
    tokens: "token",
    slowTitle: "Yerel sıkıştırma süresi yüksek",
    p95: "Sıkıştırma (p95)",
    last15: "Son 15 dakika",
    requests: "istek",
    cacheTitle: "Kaydedilen cache isabeti azaldı",
    previousWindow: "Önceki 15 dk",
    recentWindow: "Son 15 dk",
    hits: "isabet",
    cacheNote: "Sayılar sağlayıcı ve yerel yanıt önbelleği isabetlerini birlikte kapsar.",
    clientNames: { codex: "Codex", claude_code: "Claude Code", opencode: "OpenCode" } satisfies Record<ClientKind, string>,
    staleTitle: "Kurulum yeniden denetlenmeli",
    staleNote: "Uygulamalar’da yeniden tarayarak yerel kurulum kaydını güncelle.",
    activity: "Etkinlikleri aç",
    applications: "Uygulamaları denetle",
  },
  en: {
    heading: "Savings tips",
    introduction: "Signals observed in recent measurements.",
    periods: { session: "This session", today: "Today", seven_days: "Last 7 days" },
    observedAt: "Observed",
    inputTitle: "An increase in input was measured",
    inputScope: { message_input: "Message input", whole_input: "Whole input" },
    bases: { local_estimate: "Local estimate", provider_tokenizer: "Provider tokenizer count" },
    events: "measurements",
    before: "Before",
    after: "After",
    tokens: "tokens",
    slowTitle: "Local compression time is high",
    p95: "Compression (p95)",
    last15: "Last 15 minutes",
    requests: "requests",
    cacheTitle: "Recorded cache hit rate declined",
    previousWindow: "Previous 15 min",
    recentWindow: "Last 15 min",
    hits: "hits",
    cacheNote: "Counts include both provider and local response cache hits.",
    clientNames: { codex: "Codex", claude_code: "Claude Code", opencode: "OpenCode" } satisfies Record<ClientKind, string>,
    staleTitle: "Installation should be checked again",
    staleNote: "Scan again in Applications to refresh the local installation record.",
    activity: "Open activity",
    applications: "Review applications",
  },
} as const;

type Copy = {
  [Key in keyof typeof copy.tr]: typeof copy.tr[Key] extends Record<string, unknown>
    ? { [NestedKey in keyof typeof copy.tr[Key]]: string }
    : string;
};

function formatTime(value: string, language: PreferenceLanguage): string | null {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return null;
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function itemKey(item: Recommendation): string {
  return item.kind === "installation_stale" ? `${item.kind}-${item.client}` : item.kind;
}

function RecommendationRow({
  item,
  rowId,
  language,
  text,
}: {
  item: Recommendation;
  rowId: string;
  language: PreferenceLanguage;
  text: Copy;
}) {
  if (item.kind === "input_growth") {
    return (
      <article className="recommendations__row" aria-labelledby={rowId}>
        <span className="recommendations__icon"><Icon name="chart" /></span>
        <div className="recommendations__content">
          <h4 id={rowId}>{text.inputTitle}</h4>
          <p className="recommendations__meta">
            {text.inputScope[item.component]} · {text.bases[item.basis]}
          </p>
          <dl className="recommendations__values">
            <div><dt>{text.before}</dt><dd>{formatMetricNumber(item.before_tokens, language)} {text.tokens}</dd></div>
            <div><dt>{text.after}</dt><dd>{formatMetricNumber(item.after_tokens, language)} {text.tokens}</dd></div>
            <div className="recommendations__sample"><dt>{text.events}</dt><dd>{formatMetricNumber(item.events, language)}</dd></div>
          </dl>
        </div>
      </article>
    );
  }

  if (item.kind === "slow_optimization") {
    return (
      <article className="recommendations__row" aria-labelledby={rowId}>
        <span className="recommendations__icon"><Icon name="activity" /></span>
        <div className="recommendations__content">
          <h4 id={rowId}>{text.slowTitle}</h4>
          <p className="recommendations__meta">{text.last15}</p>
          <dl className="recommendations__values">
            <div><dt>{text.p95}</dt><dd>{formatMetricNumber(item.p95_ms, language)} ms</dd></div>
            <div className="recommendations__sample"><dt>{text.requests}</dt><dd>{formatMetricNumber(item.requests, language)}</dd></div>
          </dl>
        </div>
      </article>
    );
  }

  if (item.kind === "cache_share_drop") {
    const previousRate = item.previous_requests > 0
      ? formatMetricPercent(item.previous_hits / item.previous_requests * 100, language)
      : "—";
    const recentRate = item.recent_requests > 0
      ? formatMetricPercent(item.recent_hits / item.recent_requests * 100, language)
      : "—";
    return (
      <article className="recommendations__row" aria-labelledby={rowId}>
        <span className="recommendations__icon"><Icon name="activity" /></span>
        <div className="recommendations__content">
          <h4 id={rowId}>{text.cacheTitle}</h4>
          <p className="recommendations__meta">{item.provider} · {item.model}</p>
          <dl className="recommendations__values">
            <div><dt>{text.previousWindow}</dt><dd>{formatMetricNumber(item.previous_hits, language)} / {formatMetricNumber(item.previous_requests, language)} {text.hits} · {previousRate}</dd></div>
            <div><dt>{text.recentWindow}</dt><dd>{formatMetricNumber(item.recent_hits, language)} / {formatMetricNumber(item.recent_requests, language)} {text.hits} · {recentRate}</dd></div>
          </dl>
          <p className="recommendations__note">{text.cacheNote}</p>
        </div>
      </article>
    );
  }

  return (
    <article className="recommendations__row" aria-labelledby={rowId}>
      <span className="recommendations__icon"><Icon name="refresh" /></span>
      <div className="recommendations__content">
        <h4 id={rowId}>{text.clientNames[item.client]} · {text.staleTitle}</h4>
        <p className="recommendations__copy">{text.staleNote}</p>
      </div>
    </article>
  );
}

export function Recommendations({
  items,
  language,
  period,
  observedAt,
  onOpenActivity,
  onOpenApplications,
}: Props) {
  if (items.length === 0) return null;

  const text = copy[language];
  const time = formatTime(observedAt, language);

  return (
    <section className="recommendations" aria-labelledby="recommendations-title">
      <header className="recommendations__heading">
        <div>
          <h3 id="recommendations-title">{text.heading}</h3>
          <p>{text.introduction} · {text.periods[period]}</p>
        </div>
        {time && <p className="recommendations__observed"><span>{text.observedAt}</span><time dateTime={observedAt}>{time}</time></p>}
      </header>
      <div className="recommendations__list">
        {items.map(item => (
          <RecommendationRow
            key={itemKey(item)}
            item={item}
            rowId={`recommendations-${itemKey(item)}`}
            language={language}
            text={text}
          />
        ))}
      </div>
      <footer className="recommendations__actions">
        {items.some(item => item.kind !== "installation_stale") && (
          <button className="recommendations__action" type="button" onClick={onOpenActivity}>{text.activity}<Icon name="arrow" /></button>
        )}
        {items.some(item => item.kind === "installation_stale") && (
          <button className="recommendations__action" type="button" onClick={onOpenApplications}>{text.applications}<Icon name="arrow" /></button>
        )}
      </footer>
    </section>
  );
}
