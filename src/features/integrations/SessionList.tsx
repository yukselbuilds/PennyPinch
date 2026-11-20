import type { PreferenceLanguage } from "../../lib/preferences";
import type { SessionClient, SessionError, SessionRow, SessionScope, SessionSnapshot } from "../../lib/sessions";
import "./sessions.css";

type SessionListProps = {
  language: PreferenceLanguage;
  snapshot: SessionSnapshot | null;
  pending: boolean;
  error: SessionError | null;
  stale: boolean;
  opening: boolean;
  onRefresh: () => void;
  onOpenCodex: () => void;
};

const clients: readonly SessionClient[] = ["codex", "claude_code", "opencode"];
const priority: Record<SessionRow["state"], number> = {
  working: 0,
  waiting: 1,
  open: 2,
  loaded: 3,
  error: 4,
  unknown: 5,
  stored: 6,
  ended: 7,
};

const copy = {
  tr: {
    title: "Sohbetler",
    intro: "Geçerli yerel profilde görülen sohbetler. Diğer Desktop ve hesap profilleri bu kapsamın dışındadır.",
    privacy: "Başlıklar ve mesaj içerikleri kaydedilmez. Son trafik ve sohbet tasarrufu bilinmiyor; istek/sohbet bağı doğrulanmadı.",
    refresh: "Durumu yenile",
    refreshing: "Oturum durumu yenileniyor…",
    opening: "Codex sohbet merkezi açılıyor…",
    openCodex: "Codex sohbet merkezini aç",
    openUnavailable: "Bu taramada Codex merkezi açılamıyor.",
    openExplanation: "Belirli bir oturum otomatik sürdürülmez; seçimi resmî Codex sohbet merkezinde yaparsınız.",
    observedAt: "Son gözlem",
    scopeHeading: "İstemci kapsamı",
    sessionHeading: "Görülen oturumlar",
    unknownHeading: "Durumu bilinmeyen kayıtlar",
    oldHeading: "Saklanan veya kapanmış kayıtları göster",
    empty: "Bu taramada gösterilecek etkin oturum yok.",
    emptyWithUnknown: "Bazı istemci kapsamları bilinmediğinden boş liste, oturum bulunmadığı anlamına gelmez.",
    noSnapshot: "Oturum bilgisi henüz yok. Durumu yenileyin.",
    waitForRefresh: "Yenileme tamamlanınca Codex merkezi açılabilir.",
    stale: "Bu oturum görüntüsü güncel değil. Yenileyip yeni bir görüntü alın.",
    pending: "Oturum bilgileri okunuyor…",
    state: {
      working: "İstek işleniyor",
      waiting: "Onay veya kullanıcı yanıtı bekliyor",
      open: "Sunucuda açık",
      loaded: "Yüklü oturum",
      stored: "Kayıtlı · yüklenmemiş",
      ended: "Sunucu kaydı kapandı",
      error: "Sunucu hata durumu",
      unknown: "Durum bilinmiyor",
    } satisfies Record<SessionRow["state"], string>,
    role: { main: "Ana oturum", subagent: "Alt ajan", unknown: "Rol bilinmiyor" } satisfies Record<SessionRow["role"], string>,
    client: { codex: "Codex · geçerli profil", claude_code: "Claude Code", opencode: "OpenCode" } satisfies Record<SessionClient, string>,
    count: (count: number, partial: boolean) => partial ? `En az ${count} yüklü` : `${count} yüklü`,
    scopeUnknown: "Bilinmiyor",
    scopeReason: {
      scope_unavailable: "CLI kapsamı kullanılamıyor",
      endpoint_unavailable: "Yerel sunucuya erişilemiyor",
      unsafe_scope: "Kapsam güvenli biçimde doğrulanamadı",
      untrusted_endpoint: "Yerel sunucu doğrulanamadı",
      unsupported_version: "Bu sürümün sohbet durumu henüz doğrulanmadı",
      timeout: "Zaman aşımı",
      read_failed: "Oturum bilgisi okunamadı",
      invalid_response: "Sunucu yanıtı geçersiz",
      unsupported_metadata: "Oturum metadata biçimi desteklenmiyor",
      unavailable: "Bilgi kullanılamıyor",
    } satisfies Record<SessionScope["reason"] & string, string>,
    error: {
      desktop_unavailable: "Bu görünüm yalnız PennyPinch masaüstü uygulamasında kullanılabilir.",
      invalid_response: "Oturum yanıtı doğrulanamadı. Durumu yeniden okuyun.",
      validation_failed: "Oturum isteği doğrulanamadı.",
      busy: "Başka bir oturum işlemi sürüyor. Biraz sonra yeniden deneyin.",
      unavailable: "Oturum bilgisi alınamadı. Yeniden deneyin.",
      stale_snapshot: "Oturum görünümü değişti. Yenileyip tekrar deneyin.",
      launch_failed: "Codex sohbet merkezi açılamadı.",
      untrusted_endpoint: "Yerel sunucu doğrulanamadı; oturumlar gösterilmiyor.",
      unsafe_scope: "CLI kapsamı güvenli biçimde doğrulanamadı.",
    } satisfies Record<SessionError, string>,
    id: "Kayıt",
    project: "Proje",
    model: "Model",
    provider: "Sağlayıcı",
    reasoning: "Akıl yürütme",
    parent: "Üst kayıt",
    created: "Oluşturuldu",
    updated: "Kayıt güncellendi",
    staleOpen: "Önce durumu yenileyin.",
  },
  en: {
    title: "Sessions",
    intro: "Chats observed in the current local profile. Other Desktop and account profiles are outside this scope.",
    privacy: "Titles and message contents are not saved. Last traffic and chat savings are unknown; the request-to-chat link is unverified.",
    refresh: "Refresh status",
    refreshing: "Refreshing session status…",
    opening: "Opening Codex chat center…",
    openCodex: "Open Codex chat center",
    openUnavailable: "The Codex center is unavailable in this snapshot.",
    openExplanation: "No specific session is resumed automatically; choose a session in the official Codex chat center.",
    observedAt: "Last observed",
    scopeHeading: "Client scope",
    sessionHeading: "Observed sessions",
    unknownHeading: "Records with unknown status",
    oldHeading: "Show stored or ended records",
    empty: "No active sessions are listed in this snapshot.",
    emptyWithUnknown: "Some client scopes are unknown, so an empty list does not mean no sessions exist.",
    noSnapshot: "Session information is not available yet. Refresh status.",
    waitForRefresh: "Wait for the refresh to finish before opening Codex.",
    stale: "This session snapshot is stale. Refresh to get a current snapshot.",
    pending: "Reading session information…",
    state: {
      working: "Request in progress",
      waiting: "Waiting for approval or user input",
      open: "Open in server",
      loaded: "Loaded session",
      stored: "Stored · not loaded",
      ended: "Server record ended",
      error: "Server error state",
      unknown: "Status unknown",
    } satisfies Record<SessionRow["state"], string>,
    role: { main: "Main session", subagent: "Subagent", unknown: "Role unknown" } satisfies Record<SessionRow["role"], string>,
    client: { codex: "Codex · current profile", claude_code: "Claude Code", opencode: "OpenCode" } satisfies Record<SessionClient, string>,
    count: (count: number, partial: boolean) => partial ? `At least ${count} loaded` : `${count} loaded`,
    scopeUnknown: "Unknown",
    scopeReason: {
      scope_unavailable: "CLI scope unavailable",
      endpoint_unavailable: "Local server unavailable",
      unsafe_scope: "Scope could not be safely verified",
      untrusted_endpoint: "Local server could not be verified",
      unsupported_version: "Session status is not yet verified for this version",
      timeout: "Timed out",
      read_failed: "Session information could not be read",
      invalid_response: "Server response is invalid",
      unsupported_metadata: "Session metadata format is unsupported",
      unavailable: "Information unavailable",
    } satisfies Record<SessionScope["reason"] & string, string>,
    error: {
      desktop_unavailable: "This view is available only in the PennyPinch desktop app.",
      invalid_response: "The session response could not be verified. Read status again.",
      validation_failed: "The session request could not be validated.",
      busy: "Another session operation is running. Try again shortly.",
      unavailable: "Session information could not be read. Try again.",
      stale_snapshot: "The session snapshot changed. Refresh and try again.",
      launch_failed: "The Codex chat center could not be opened.",
      untrusted_endpoint: "The local server could not be verified; sessions are hidden.",
      unsafe_scope: "The CLI scope could not be safely verified.",
    } satisfies Record<SessionError, string>,
    id: "Record",
    project: "Project",
    model: "Model",
    provider: "Provider",
    reasoning: "Reasoning",
    parent: "Parent record",
    created: "Created",
    updated: "Record updated",
    staleOpen: "Refresh status first.",
  },
} as const;

const activeStates = new Set<SessionRow["state"]>(["working", "waiting", "open", "loaded"]);
const priorStates = new Set<SessionRow["state"]>(["stored", "ended"]);

function formatTime(value: string | null, language: PreferenceLanguage): string {
  if (!value) return language === "tr" ? "Bilinmiyor" : "Unknown";
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return language === "tr" ? "Bilinmiyor" : "Unknown";
  return new Intl.DateTimeFormat(language === "tr" ? "tr-TR" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function compareRows(a: SessionRow, b: SessionRow): number {
  const stateOrder = priority[a.state] - priority[b.state];
  if (stateOrder !== 0) return stateOrder;
  return (b.updated_at ?? "").localeCompare(a.updated_at ?? "");
}

function ScopeRow({ clientKind, scope, language }: { clientKind: SessionClient; scope: SessionScope | undefined; language: PreferenceLanguage }) {
  const text = copy[language];
  const client = text.client[clientKind];
  const summary = !scope || scope.state === "unknown" || scope.loaded_count === null
    ? text.scopeUnknown
    : text.count(scope.loaded_count, scope.partial);
  return <li className={`sessions__scope-row${scope?.state === "unknown" ? " is-unknown" : ""}`}>
    <span className="sessions__scope-name">{client}</span>
    <span className="sessions__scope-value">{summary}</span>
    {scope?.state === "unknown" && scope.reason && <span className="sessions__scope-reason">{text.scopeReason[scope.reason]}</span>}
  </li>;
}

function SessionItem({ row, language }: { row: SessionRow; language: PreferenceLanguage }) {
  const text = copy[language];
  const stateClass = row.state.replaceAll("_", "-");
  return <li className="sessions__item">
    <article className="sessions__record" aria-label={`${text.client[row.client_kind]} · ${text.id} ${row.id_hint}`}>
      <header className="sessions__record-header">
        <div className="sessions__record-id">
          <span>{text.client[row.client_kind]}</span>
          <code>{row.id_hint}</code>
        </div>
        <span className={`sessions__state sessions__state--${stateClass}`}>{text.state[row.state]}</span>
      </header>
      <dl className="sessions__facts">
        <div><dt>{language === "tr" ? "Rol" : "Role"}</dt><dd>{text.role[row.role]}</dd></div>
        {row.parent_key && <div><dt>{text.parent}</dt><dd><code>{row.parent_key.slice(0, 8)}</code></dd></div>}
        {row.project && <div><dt>{text.project}</dt><dd>{row.project}</dd></div>}
        {row.model && <div><dt>{text.model}</dt><dd>{row.model}</dd></div>}
        {row.provider && <div><dt>{text.provider}</dt><dd>{row.provider}</dd></div>}
        {row.reasoning && <div><dt>{text.reasoning}</dt><dd>{row.reasoning}</dd></div>}
        {row.created_at && <div><dt>{text.created}</dt><dd>{formatTime(row.created_at, language)}</dd></div>}
        {row.updated_at && <div><dt>{text.updated}</dt><dd>{formatTime(row.updated_at, language)}</dd></div>}
      </dl>
    </article>
  </li>;
}

export function SessionList({
  language,
  snapshot,
  pending,
  error,
  stale,
  opening,
  onRefresh,
  onOpenCodex,
}: SessionListProps) {
  const text = copy[language];
  const scopes = snapshot?.clients ?? [];
  const rows = [...(snapshot?.rows ?? [])].sort(compareRows);
  const visibleRows = rows.filter((row) => !priorStates.has(row.state));
  const previousRows = rows.filter((row) => priorStates.has(row.state));
  const activeRows = visibleRows.filter((row) => activeStates.has(row.state));
  const unknownRows = visibleRows.filter((row) => !activeStates.has(row.state));
  const unknownScope = scopes.some((scope) => scope.state === "unknown");
  const canOpen = Boolean(snapshot?.can_open_codex && !pending && !opening && !stale);
  const openNote = stale ? text.staleOpen : pending ? text.waitForRefresh
    : !snapshot ? text.noSnapshot : !snapshot.can_open_codex ? text.openUnavailable : null;

  return <section className="sessions" aria-labelledby="sessions-title" aria-busy={pending || opening}>
    <header className="sessions__header">
      <div className="sessions__heading">
        <h2 id="sessions-title">{text.title}</h2>
        <p className="sessions__intro">{text.intro}</p>
      </div>
      <div className="sessions__actions">
        <button className="button" type="button" onClick={onRefresh} disabled={pending || opening}>
          {pending ? text.refreshing : text.refresh}
        </button>
        <button className="button button-primary" type="button" onClick={onOpenCodex} disabled={!canOpen} aria-describedby={openNote ? "sessions-open-note" : undefined}>
          {opening ? text.opening : text.openCodex}
        </button>
      </div>
    </header>

    <p className="sessions__privacy">{text.privacy}</p>
    <p className="sessions__open-explanation">{text.openExplanation}</p>

    {error && <p className="sessions__notice sessions__notice--error" role="alert">{text.error[error]}</p>}
    {stale && <p className="sessions__notice sessions__notice--warning" role="status">{text.stale}</p>}
    {pending && <p className="sessions__status" role="status" aria-live="polite">{text.pending}</p>}
    {opening && <p className="sessions__status" role="status" aria-live="polite">{text.opening}</p>}

    <div className="sessions__scope">
      <h3>{text.scopeHeading}</h3>
      <ul className="sessions__scope-list">
        {clients.map((client) => <ScopeRow key={client} clientKind={client} scope={scopes.find((scope) => scope.client_kind === client)} language={language} />)}
      </ul>
      {snapshot && <p className="sessions__observed">{text.observedAt}: {formatTime(snapshot.observed_at_utc, language)}</p>}
    </div>

    {openNote && <p id="sessions-open-note" className="sessions__open-note">{openNote}</p>}

    {snapshot ? <div className="sessions__records">
      {activeRows.length > 0 && <section aria-labelledby="sessions-records-title">
        <h3 id="sessions-records-title" className="sessions__section-title">{text.sessionHeading}</h3>
        <ul className="sessions__list">{activeRows.map((row) => <SessionItem key={row.key} row={row} language={language} />)}</ul>
      </section>}
      {unknownRows.length > 0 && <section aria-labelledby="sessions-unknown-title">
        <h3 id="sessions-unknown-title" className="sessions__section-title">{text.unknownHeading}</h3>
        <ul className="sessions__list">{unknownRows.map((row) => <SessionItem key={row.key} row={row} language={language} />)}</ul>
      </section>}
      {activeRows.length === 0 && unknownRows.length === 0 && <div className="sessions__empty">
        <p>{text.empty}</p>
        {unknownScope && <p>{text.emptyWithUnknown}</p>}
      </div>}
      {previousRows.length > 0 && <details className="sessions__previous">
        <summary>{text.oldHeading}</summary>
        <ul className="sessions__list">{previousRows.map((row) => <SessionItem key={row.key} row={row} language={language} />)}</ul>
      </details>}
    </div> : <p className="sessions__empty">{pending ? text.pending : text.noSnapshot}</p>}
  </section>;
}
