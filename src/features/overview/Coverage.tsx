import {
  formatMetricNumber,
  formatMetricPercent,
  type AttributionRow,
  type AttributionStatus,
  type AttributionSummary,
} from "../../lib/metrics";
import type { PreferenceLanguage } from "../../lib/preferences";
import "./coverage.css";

interface CoverageProps {
  language: PreferenceLanguage;
  summary: AttributionSummary;
}

const copy = {
  tr: {
    title: "Uygulama ve model bazında",
    scope: "Yalnız kaydedilen istekler gösterilir; bu, tüm sağlayıcı trafiğini temsil etmez.",
    evidence: "Model ve sağlayıcı adları gözlenen etiketlerdir. Rota ve ajan ataması ayrı kanıt gerektirir.",
    clients: "İstemci ataması",
    agents: "Ajan ataması",
    unknown: "Bilinmiyor",
    partial: "Kısmi",
    verified: "Doğrulanmış alt küme",
    requests: "istek",
    events: "ölçüm olayı",
    conflicts: "çakışma",
    application: "Uygulama / model",
    agent: "Ajan",
    observedRequests: "Gözlenen istek",
    input: "Girdi · önce → sonra",
    applicationUnknown: "Uygulama bilinmiyor",
    modelUnknown: "Model bilinmiyor",
    agentUnknown: "Ajan bilinmiyor",
    main: "Ana ajan",
    subagent: "Alt ajan",
    roleUnknown: "Rol bilinmiyor",
    route: "Rota",
    conflicted: "Kimlik çakışması",
    otherGroups: "Diğer gruplar",
    multipleGroups: "Birden çok grup",
    omitted: "grup Diğer satırında",
    reduction: "azalma",
    increase: "artış",
    unchanged: "değişim yok",
    noComparable: "Karşılaştırılabilir ölçüm yok",
    mixed: "Temeller farklı; toplam gösterilmiyor",
    overflow: "Sayaç sınırı; toplam gösterilmiyor",
    empty: "Bu kapsamda grup satırı yok.",
  },
  en: {
    title: "By application and model",
    scope: "Only recorded requests are shown; this may not represent all provider traffic.",
    evidence: "Model and provider names are observed labels. Route and agent assignment need separate evidence.",
    clients: "Client attribution",
    agents: "Agent attribution",
    unknown: "Unknown",
    partial: "Partial",
    verified: "Verified subset",
    requests: "requests",
    events: "measurement events",
    conflicts: "conflicts",
    application: "Application / model",
    agent: "Agent",
    observedRequests: "Observed requests",
    input: "Input · before → after",
    applicationUnknown: "Application unknown",
    modelUnknown: "Model unknown",
    agentUnknown: "Agent unknown",
    main: "Main agent",
    subagent: "Subagent",
    roleUnknown: "Role unknown",
    route: "Route",
    conflicted: "Attribution conflict",
    otherGroups: "Other groups",
    multipleGroups: "Multiple groups",
    omitted: "groups in the Other row",
    reduction: "reduction",
    increase: "increase",
    unchanged: "no change",
    noComparable: "No comparable measurement",
    mixed: "Different bases; total hidden",
    overflow: "Counter limit; total hidden",
    empty: "There are no group rows in this scope.",
  },
} as const;
type CoverageCopy = { [Key in keyof typeof copy.en]: string };

const clientNames = {
  codex: "Codex",
  claude_code: "Claude Code",
  opencode: "OpenCode",
} as const;
function statusLabel(status: AttributionStatus, text: CoverageCopy): string {
  switch (status) {
    case "verified": return text.verified;
    case "partial": return text.partial;
    default: return text.unknown;
  }
}
function shortId(value: string | null): string | null {
  return value ? value.slice(0, 8) : null;
}
function groupName(row: AttributionRow, text: CoverageCopy): string {
  if (row.is_other) return text.otherGroups;
  const client = row.client ? clientNames[row.client] : text.applicationUnknown;
  return `${client} · ${row.model ?? text.modelUnknown}`;
}
function rowInput(row: AttributionRow, language: PreferenceLanguage, text: CoverageCopy): { before: string; after: string; detail: string } {
  const totals = row.totals;
  if (totals.status !== "measured" || totals.before_tokens === null || totals.after_tokens === null || totals.saved_tokens === null) {
    const status = totals.status === "mixed_bases" ? text.mixed
      : totals.status === "counter_overflow" ? text.overflow : text.noComparable;
    return { before: "—", after: "—", detail: status };
  }
  const amount = formatMetricNumber(Math.abs(totals.saved_tokens), language);
  const change = totals.saved_tokens > 0 ? text.reduction : totals.saved_tokens < 0 ? text.increase : text.unchanged;
  const percent = totals.saved_percent === null
    ? "—"
    : formatMetricPercent(Math.abs(totals.saved_percent), language);
  return {
    before: formatMetricNumber(totals.before_tokens, language),
    after: formatMetricNumber(totals.after_tokens, language),
    detail: `${amount} ${change} · ${percent}`,
  };
}
function CoverageRow({ row, language, text }: { row: AttributionRow; language: PreferenceLanguage; text: CoverageCopy }) {
  const agent = row.agent;
  const agentLabel = row.is_other ? text.multipleGroups : !agent ? text.agentUnknown : `${agent.role === "main" ? text.main : agent.role === "subagent" ? text.subagent : text.roleUnknown} · ${shortId(agent.id)}`;
  const input = rowInput(row, language, text);
  const routeId = shortId(row.upstream_id);

  return (
    <tr className={row.conflicted ? "coverage__row is-conflicted" : undefined}>
      <th scope="row">
        <span className="coverage__primary-label">{groupName(row, text)}</span>
        {row.provider && !row.is_other && <span className="coverage__secondary-label">{row.provider}</span>}
        {routeId && !row.is_other && <span className="coverage__route-label">{text.route} · {routeId}</span>}
        {row.conflicted && <span className="coverage__conflict">{text.conflicted}</span>}
      </th>
      <td>{agentLabel}</td>
      <td className="coverage__number">{formatMetricNumber(row.coverage.requests, language)}</td>
      <td>
        <span className="coverage__primary-label">{input.before} → {input.after}</span>
        <span className="coverage__secondary-label">{input.detail}</span>
      </td>
    </tr>
  );
}

export function Coverage({ language, summary }: CoverageProps) {
  if (summary.coverage.requests === 0) return null;
  const text = copy[language === "tr" ? "tr" : "en"];
  const displayedGroups = summary.groups.slice(0, 64);
  const omitted = summary.omitted_groups + Math.max(0, summary.groups.length - displayedGroups.length);
  const clientVerified = formatMetricNumber(summary.coverage.client_verified, language);
  const agentVerified = formatMetricNumber(summary.coverage.agent_verified, language);

  return (
    <section className="coverage" aria-labelledby="coverage-title">
      <header className="coverage__header">
        <div>
          <h3 id="coverage-title">{text.title}</h3>
          <p>{text.scope}</p>
        </div>
        <div className="coverage__counts" aria-label={`${formatMetricNumber(summary.coverage.requests, language)} ${text.requests}`}>
          <strong>{formatMetricNumber(summary.coverage.requests, language)}</strong>
          <span>{text.requests}</span>
        </div>
      </header>

      <p className="coverage__evidence">{text.evidence}</p>

      <details className="coverage__details"><summary>{language === "tr" ? "Kimlik ve kapsam ayrıntıları" : "Identity and coverage details"}</summary>
      <div className="coverage__status-grid">
        <div className={`coverage__status is-${summary.client_status}`}>
          <span>{text.clients}</span>
          <strong>{statusLabel(summary.client_status, text)}</strong>
          <small>{clientVerified} / {formatMetricNumber(summary.coverage.requests, language)} {text.requests}</small>
        </div>
        <div className={`coverage__status is-${summary.agent_status}`}>
          <span>{text.agents}</span>
          <strong>{statusLabel(summary.agent_status, text)}</strong>
          <small>{agentVerified} / {formatMetricNumber(summary.coverage.requests, language)} {text.requests}</small>
        </div>
      </div>
      </details>

      {(summary.coverage.events > 0 || summary.coverage.conflicts > 0 || omitted > 0) && (
        <p className="coverage__meta">
          {formatMetricNumber(summary.coverage.events, language)} {text.events}
          {summary.coverage.conflicts > 0 && <> · {formatMetricNumber(summary.coverage.conflicts, language)} {text.conflicts}</>}
          {omitted > 0 && <> · {formatMetricNumber(omitted, language)} {text.omitted}</>}
        </p>
      )}

      <div className="coverage__table-scroll" role="region" aria-label={text.title} tabIndex={0}>
        <table>
          <caption className="coverage__sr-only">{text.evidence}</caption>
          <thead>
            <tr>
              <th scope="col">{text.application}</th>
              <th scope="col">{text.agent}</th>
              <th scope="col" className="coverage__number">{text.observedRequests}</th>
              <th scope="col">{text.input}</th>
            </tr>
          </thead>
          <tbody>
            {displayedGroups.map((row) => <CoverageRow key={row.group_id} row={row} language={language} text={text} />)}
          </tbody>
        </table>
        {displayedGroups.length === 0 && <p className="coverage__empty">{text.empty}</p>}
      </div>
    </section>
  );
}
