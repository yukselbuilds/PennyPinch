import type { PreferenceLanguage } from "../../lib/preferences";
import type {
  ClaudeContextScope,
  ClaudeContextState,
  ClaudeLayerSummary,
  ClaudeSettingsSnapshot,
} from "../../lib/claudeContext";

type Props = {
  language: PreferenceLanguage;
  context: ClaudeContextState | null;
  settings: ClaudeSettingsSnapshot | null;
  loading: boolean;
  pickerPending: boolean;
  disabled: boolean;
  error: string | null;
  onSelectScope: (scope: ClaudeContextScope) => void;
};

const copy = {
  tr: {
    title: "Claude ayar kapsamı",
    user: "Kullanıcı ayarları",
    project: "Proje klasörü seç",
    userTarget: "CLAUDE_CONFIG_DIR/settings.json veya %USERPROFILE%/.claude/settings.json",
    projectTarget: "Hedef dosya",
    projectMissing: "Geri alma için aynı proje klasörünü yeniden seçin. Proje seçimi uygulama yeniden açıldığında saklanmaz.",
    parentLimit: "İlk kurulumda .claude klasörü önceden var olmalıdır; uygulama klasör oluşturmaz.",
    oneTarget: "Aynı anda yalnızca bir Claude hedefi yönetilir; hedefi değiştirmeden önce mevcut yönlendirmeyi geri alın.",
    loading: "Claude kapsamı ve ayar katmanları okunuyor…",
    error: "Claude kapsamı okunamadı.",
    picker: "Klasör seçici açık…",
    observed: "Gözlenen ayar dosyaları",
    source: "Kaynak",
    availability: "Dosya",
    route: "Yönlendirme bildirimi",
    model: "Model bildirimi",
    reasoning: "Reasoning bildirimi",
    auth: "Kimlik doğrulama bildirimi",
    hooks: "Hook bildirimi",
    present: "Var",
    missing: "Yok",
    notObserved: "Gözlenmedi",
    absent: "Yok",
    unknown: "Bilinmiyor",
    headroom: "Headroom endpoint'i",
    otherRoute: "Başka yönlendirme (gizli)",
    hookPresent: "Var; değiştirilmeden kalır",
    aggregate: "Gözlenen dosyalardaki bildirimler",
    runtime: "Çalışma anında etkin ayar bilinmiyor. Uzak/registry managed policy ve diğer oturumlar da bilinmiyor.",
    processEnv: "Headroom sürecinin ortamı; tüm oturumların ortamı değildir.",
  },
  en: {
    title: "Claude settings scope",
    user: "User settings",
    project: "Choose project folder",
    userTarget: "CLAUDE_CONFIG_DIR/settings.json or %USERPROFILE%/.claude/settings.json",
    projectTarget: "Target file",
    projectMissing: "Select the same project folder again to undo. The project selection is not saved when the app closes.",
    parentLimit: "The .claude folder must already exist for first setup; the app will not create it.",
    oneTarget: "Only one Claude target is managed at a time; undo the current routing before changing targets.",
    loading: "Reading Claude scope and settings layers…",
    error: "Claude scope could not be read.",
    picker: "Folder picker is open…",
    observed: "Observed settings files",
    source: "Source",
    availability: "File",
    route: "Route declaration",
    model: "Model declaration",
    reasoning: "Reasoning declaration",
    auth: "Authentication declaration",
    hooks: "Hook declaration",
    present: "Present",
    missing: "Missing",
    notObserved: "Not observed",
    absent: "Absent",
    unknown: "Unknown",
    headroom: "Headroom endpoint",
    otherRoute: "Another route (redacted)",
    hookPresent: "Present; unchanged",
    aggregate: "Declarations in observed files",
    runtime: "Effective runtime settings are unknown. Remote or registry managed policy and other sessions are also unknown.",
    processEnv: "Headroom process environment; this does not describe every session.",
  },
} as const;

type ScopeCopy = typeof copy["en"] | typeof copy["tr"];

function declaration(value: "absent" | "present" | "unknown", text: ScopeCopy) {
  return value === "present" ? text.present : value === "absent" ? text.absent : text.unknown;
}

function layerSource(layer: ClaudeLayerSummary, language: PreferenceLanguage): string {
  const labels = {
    tr: {
      user_settings: "Kullanıcı settings.json",
      shared_project_settings: "Proje settings.json",
      local_project_settings: "Proje settings.local.json",
      local_managed_settings: "Yerelde gözlenen managed settings dosyası",
      process_environment: "Headroom process environment",
    },
    en: {
      user_settings: "User settings.json",
      shared_project_settings: "Project settings.json",
      local_project_settings: "Project settings.local.json",
      local_managed_settings: "Locally observed managed settings file",
      process_environment: "Headroom process environment",
    },
  };
  return labels[language][layer.source];
}

function layerValue(layer: ClaudeLayerSummary, field: "availability" | "route" | "model" | "reasoning" | "auth" | "hooks", language: PreferenceLanguage): string {
  const text = copy[language];
  if (field === "availability") {
    return layer.availability === "present" ? text.present : layer.availability === "missing" ? text.missing : text.notObserved;
  }
  if (field === "route") {
    return layer.route === "headroom_endpoint" ? text.headroom
      : layer.route === "other_configured_redacted" ? text.otherRoute
        : layer.route === "absent" ? text.absent : text.unknown;
  }
  if (field === "model" || field === "reasoning") return declaration(layer[field], text);
  if (field === "auth") {
    return layer.auth === "secret_configured" ? (language === "tr" ? "Kimlik bilgisi tanımlı" : "Credential declared")
      : layer.auth === "helper_declared" ? (language === "tr" ? "Yardımcı tanımlı" : "Helper declared") : text.unknown;
  }
  return layer.hooks === "present_unchanged" ? text.hookPresent
    : layer.hooks === "absent" ? text.absent : text.unknown;
}

export function ClaudeScopeSection({ language, context, settings, loading, pickerPending, disabled, error, onSelectScope }: Props) {
  const text = copy[language];
  const scope = context?.scope ?? "user_config";
  const target = scope === "project_local_config" && context?.project_directory
    ? `${context.project_directory}\\.claude\\settings.local.json`
    : text.userTarget;
  const rows: Array<[keyof Pick<ClaudeLayerSummary, "availability" | "route" | "model" | "reasoning" | "auth" | "hooks">, string]> = [
    ["availability", text.availability], ["route", text.route], ["model", text.model],
    ["reasoning", text.reasoning], ["auth", text.auth], ["hooks", text.hooks],
  ];

  return <section className="connection-dialog__section claude-scope" aria-labelledby="claude-scope-title">
    <div className="claude-scope__heading">
      <h3 id="claude-scope-title">{text.title}</h3>
      <div className="claude-scope__controls">
        <button className="button" type="button" aria-pressed={scope === "user_config"} disabled={disabled || context?.scope === "user_config"} onClick={() => onSelectScope("user_config")}>{text.user}</button>
        <button className="button" type="button" aria-pressed={scope === "project_local_config"} disabled={disabled || pickerPending} onClick={() => onSelectScope("project_local_config")}>{pickerPending ? text.picker : text.project}</button>
      </div>
    </div>
    {loading && <p className="connection-dialog__notice" role="status"><span className="applications-spinner" aria-hidden="true" />{text.loading}</p>}
    {error && <p className="connection-dialog__notice connection-dialog__notice--error" role="alert">{error || text.error}</p>}
    {!loading && context && <>
      <p className="claude-scope__location"><strong>{text.projectTarget}:</strong> <code>{target}</code></p>
      <p className="connection-dialog__detail-note">{text.oneTarget}</p>
      {scope === "project_local_config" && <>
        <p className="connection-dialog__detail-note">{text.projectMissing}</p>
        <p className="connection-dialog__detail-note">{text.parentLimit}</p>
      </>}
      {settings && <>
        <h4 className="claude-scope__subheading">{text.observed}</h4>
        <div className="claude-scope__layers">
          {settings.layers.map((layer) => <section className="claude-scope__layer" key={layer.source}>
            <h5>{layerSource(layer, language)}</h5>
            <dl>
              {rows.map(([field, label]) => <div key={field}><dt>{label}</dt><dd>{layerValue(layer, field, language)}</dd></div>)}
            </dl>
          </section>)}
        </div>
        <h4 className="claude-scope__subheading">{text.aggregate}</h4>
        <dl className="connection-dialog__details claude-scope__aggregate">
          <div><dt>{text.model}</dt><dd>{declaration(settings.model_declared_in_observed_files, text)}</dd></div>
          <div><dt>{text.reasoning}</dt><dd>{declaration(settings.reasoning_declared_in_observed_files, text)}</dd></div>
          <div><dt>{text.auth}</dt><dd>{settings.auth_declared_in_observed_files === "secret_configured" ? (language === "tr" ? "Kimlik bilgisi tanımlı" : "Credential declared") : settings.auth_declared_in_observed_files === "helper_declared" ? (language === "tr" ? "Yardımcı tanımlı" : "Helper declared") : text.unknown}</dd></div>
          <div><dt>{text.hooks}</dt><dd>{settings.hooks_declared_in_observed_files === "present_unchanged" ? text.hookPresent : settings.hooks_declared_in_observed_files === "absent" ? text.absent : text.unknown}</dd></div>
        </dl>
      </>}
      <p className="connection-dialog__detail-note">{text.runtime}</p>
      {settings?.layers.some((layer) => layer.source === "process_environment") && <p className="connection-dialog__detail-note">{text.processEnv}</p>}
    </>}
  </section>;
}
