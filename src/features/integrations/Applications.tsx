import { useEffect, useRef, useState } from "react";
import {
  discoverySession,
  DiscoveryRequestSequence,
  isDiscoveryAvailable,
  type ClientKind,
  type DiscoveryResult,
  type DiscoverySnapshot,
  type InstallationSummary,
  type InstallationSource,
} from "../../lib/integrations";
import type { PreferenceLanguage } from "../../lib/preferences";
import { ConnectDialog } from "./ConnectDialog";
import { ConnectionStatus } from "./ConnectionStatus";

type Copy = {
  intro: string;
  scan: string;
  scanning: string;
  scanAgain: string;
  loading: string;
  desktopRequired: string;
  unavailableTitle: string;
  unavailableBody: string;
  readFailed: string;
  invalidResponse: string;
  busy: string;
  blocked: string;
  validation: string;
  previousResults: string;
  partialScope: string;
  sessionOnly: string;
  pickerCancelled: string;
  pickerBlocked: string;
  pickerInvalid: string;
  fileGuide: string;
  notScanned: string;
  noResult: string;
  detected: string;
  notFound: string;
  found: string;
  unknownVersion: string;
  connectionUnverified: string;
  connect: string;
  addFile: string;
  selectingFile: string;
  actionUnavailable: string;
  trCli: string;
  trDesktop: string;
  sources: Record<InstallationSource, string>;
};

const copy: Record<PreferenceLanguage, Copy> = {
  tr: {
    intro: "Headroom yalnızca sınırlı yerel konumları tarar. Bulunmaması, istemcinin desteklenmediği anlamına gelmez.",
    scan: "Taramayı başlat", scanning: "Taranıyor…", scanAgain: "Yeniden tara", loading: "Yerel kurulumlar kontrol ediliyor…",
    desktopRequired: "İstemci keşfi masaüstü uygulamasında kullanılabilir.",
    unavailableTitle: "Masaüstü bağlantısı gerekli", unavailableBody: "Bu önizleme yerel kurulumları taramaz ve istemci sonucu göstermez.",
    readFailed: "İstemci bilgileri okunamadı. Yeniden deneyebilirsin.", invalidResponse: "Yerel keşif yanıtı doğrulanamadı.",
    busy: "Başka bir keşif işlemi sürüyor. Biraz sonra yeniden dene.",
    blocked: "Yerel keşif bu ortamda kullanılamıyor.", validation: "Keşif isteği doğrulanamadı.",
    previousResults: "İşlem başarısız oldu; son tamamlanan taramanın sonuçları korunuyor.",
    partialScope: "Tarama bilinen yerel konumlarla sınırlıdır. Yokluk, desteklenmediği anlamına gelmez.",
    sessionOnly: "Dosyadan seçilen kurulumlar Headroom kapanana kadar bellekte tutulur. Bağlantı doğrulanmaz.",
    pickerCancelled: "Dosya seçimi iptal edildi; mevcut sonuçlar korundu.",
    pickerBlocked: "Dosya seçici açılamadı. İstemciye ait komut dosyasını yerel sabit diskten seç.",
    pickerInvalid: "Yanıt doğrulanamadı. İstemciye ait komut dosyasını yerel sabit diskten seç.",
    fileGuide: "Kabul edilen dosya adları: {files}. Yerel sabit diskten bir dosya seç.",
    notScanned: "Tarama henüz tamamlanmadı.", noResult: "Sonuç alınamadı", detected: "Kurulum bulundu",
    notFound: "Bilinen konumlarda bulunamadı", found: "Bulunan kurulumlar", unknownVersion: "Sürüm bilinmiyor",
    connectionUnverified: "Bağlantı doğrulanmadı", connect: "Bağlantıyı yönet", addFile: "Dosyadan ekle",
    selectingFile: "Dosya seçiliyor…", actionUnavailable: "Bu işlem henüz kullanılamıyor.",
    trCli: "Komut satırı", trDesktop: "Masaüstü",
    sources: {
      path: "PATH", user_local_npm: "Kullanıcıya ait npm", user_local_native: "Kullanıcıya ait kurulum",
      scoop: "Scoop", windows_app_package: "Windows uygulama paketi", manual_file: "Selected file",
    },
  },
  en: {
    intro: "Headroom scans a limited set of local locations. An installation not found here does not mean the client is unsupported.",
    scan: "Scan for clients", scanning: "Scanning…", scanAgain: "Scan again", loading: "Checking local installations…",
    desktopRequired: "Client discovery is available in the desktop app.",
    unavailableTitle: "Desktop app required", unavailableBody: "This preview does not scan local installations or show client results.",
    readFailed: "Client information could not be read. You can try again.", invalidResponse: "The local discovery response could not be validated.",
    busy: "Another discovery operation is in progress. Try again shortly.",
    blocked: "Local discovery is unavailable in this environment.", validation: "The discovery request could not be validated.",
    previousResults: "The operation failed; results from the last completed scan are preserved.",
    partialScope: "Scanning is limited to known local locations. Absence here does not mean unsupported.",
    sessionOnly: "Files selected here stay in memory until Headroom closes. A connection is not verified.",
    pickerCancelled: "File selection was cancelled; current results were kept.",
    pickerBlocked: "The file picker could not open. Choose the client command file from a local fixed drive.",
    pickerInvalid: "The response could not be validated. Choose the client command file from a local fixed drive.",
    fileGuide: "Accepted file names: {files}. Choose a file on a local fixed drive.",
    notScanned: "The scan has not completed yet.", noResult: "No result available", detected: "Installation found",
    notFound: "Not found in known locations", found: "Detected installations", unknownVersion: "Version unknown",
    connectionUnverified: "Connection not verified", connect: "Manage connection", addFile: "Add from file",
    selectingFile: "Selecting file…", actionUnavailable: "This action is not available yet.",
    trCli: "Command line", trDesktop: "Desktop",
    sources: {
      path: "PATH", user_local_npm: "User-local npm", user_local_native: "User-local install",
      scoop: "Scoop", windows_app_package: "Windows app package", manual_file: "Selected file",
    },
  },
};

type OperationSource = "initial" | "scan" | { kind: "picker"; clientKind: ClientKind };
type OperationIssue = { result: DiscoveryResult; source: OperationSource };
type ViewState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "error"; issue: OperationIssue }
  | { kind: "ready"; snapshot: DiscoverySnapshot; issue?: OperationIssue };

const clients: { kind: ClientKind; title: string; mark: string }[] = [
  { kind: "codex", title: "Codex", mark: "C" },
  { kind: "claude_code", title: "Claude Code", mark: "Cl" },
  { kind: "opencode", title: "OpenCode", mark: "O" },
];

const commandFileNames: Record<ClientKind, string> = {
  codex: "codex.exe, codex.cmd, codex.bat",
  claude_code: "claude.exe, claude.cmd, claude.bat",
  opencode: "opencode.exe, opencode.cmd, opencode.bat",
};

export function Applications({ language }: { language: PreferenceLanguage }) {
  const text = copy[language];
  const [state, setState] = useState<ViewState>(() => isDiscoveryAvailable() ? { kind: "loading" } : { kind: "unavailable" });
  const [refreshing, setRefreshing] = useState(false);
  const [selectingClient, setSelectingClient] = useState<ClientKind | null>(null);
  const [connectionClient, setConnectionClient] = useState<ClientKind | null>(null);
  const [notice, setNotice] = useState<"picker_cancelled" | null>(null);
  const mounted = useRef(false);
  const actionLocked = useRef(false);
  const requests = useRef(new DiscoveryRequestSequence());

  function showResult(result: DiscoveryResult, preserveLastScan: boolean, source: OperationSource) {
    setNotice(null);
    if (result.kind === "snapshot") {
      setState({ kind: "ready", snapshot: result.snapshot });
    } else if (result.kind === "desktop_unavailable") {
      setState({ kind: "unavailable" });
    } else {
      const issue = { result, source };
      setState((current) => preserveLastScan && current.kind === "ready"
        ? { ...current, issue }
        : { kind: "error", issue });
    }
  }

  async function loadInitial() {
    const request = requests.current.next();
    const result = await discoverySession.loadInitial();
    if (!mounted.current || !requests.current.isCurrent(request)) return;
    showResult(result, false, "initial");
  }

  async function scanAgain() {
    if (actionLocked.current || !isDiscoveryAvailable()) return;
    actionLocked.current = true;
    setNotice(null);
    setRefreshing(true);
    const request = requests.current.next();
    try {
      const result = await discoverySession.scanDiscovery();
      if (!mounted.current || !requests.current.isCurrent(request)) return;
      showResult(result, true, "scan");
    } finally {
      actionLocked.current = false;
      if (mounted.current && requests.current.isCurrent(request)) setRefreshing(false);
    }
  }

  async function addFromFile(clientKind: ClientKind) {
    if (actionLocked.current || !isDiscoveryAvailable()) return;
    actionLocked.current = true;
    setNotice(null);
    setSelectingClient(clientKind);
    const request = requests.current.next();
    try {
      const result = await discoverySession.selectClientFile(clientKind);
      if (!mounted.current || !requests.current.isCurrent(request)) return;
      if (result.kind === "cancelled") {
        setNotice("picker_cancelled");
        return;
      }
      showResult(result, true, { kind: "picker", clientKind });
    } finally {
      actionLocked.current = false;
      if (mounted.current && requests.current.isCurrent(request)) setSelectingClient(null);
    }
  }

  function openConnectionPreview(clientKind: ClientKind) {
    if (actionLocked.current || !isDiscoveryAvailable()) return;
    actionLocked.current = true;
    setConnectionClient(clientKind);
  }

  function closeConnectionPreview() {
    actionLocked.current = false;
    setConnectionClient(null);
  }

  useEffect(() => {
    mounted.current = true;
    void loadInitial();
    return () => {
      mounted.current = false;
      requests.current.invalidate();
    };
  }, []);

  const scanning = state.kind === "loading" || refreshing;
  const mutationPending = scanning || selectingClient !== null || connectionClient !== null;
  const snapshot = state.kind === "ready" ? state.snapshot : null;
  const readyState = state.kind === "ready";
  const issue = state.kind === "error" ? state.issue : state.kind === "ready" ? state.issue : undefined;

  return <section className="applications" aria-label={language === "tr" ? "İstemci keşfi" : "Client discovery"}>
    <div className="applications-toolbar">
      <div>
        <div className="panel-kicker">{language === "tr" ? "YEREL KEŞİF" : "LOCAL DISCOVERY"}</div>
        <p className="applications-intro">{text.intro}</p>
      </div>
      <button className="button applications-scan" type="button" onClick={() => void scanAgain()}
        disabled={!isDiscoveryAvailable() || mutationPending}
        aria-label={scanning ? (refreshing ? text.scanning : text.loading) : snapshot ? text.scanAgain : text.scan}>
        <span aria-hidden="true">⌕</span>{scanning ? (refreshing ? text.scanning : text.loading) : snapshot ? text.scanAgain : text.scan}
      </button>
    </div>

    <div className="applications-scope" role="note">
      <span className="applications-scope__mark" aria-hidden="true">i</span>{text.sessionOnly}
    </div>

    {notice === "picker_cancelled" && <div className="applications-notice" role="status" aria-live="polite">
      <span className="applications-notice__mark" aria-hidden="true">i</span><div>{text.pickerCancelled}</div>
    </div>}

    {state.kind === "unavailable" && <div className="applications-notice" role="status">
      <span className="applications-notice__mark" aria-hidden="true">i</span>
      <div><strong>{text.unavailableTitle}</strong><p>{text.unavailableBody}</p></div>
    </div>}

    {state.kind === "loading" && <div className="applications-notice" role="status">
      <span className="applications-spinner" aria-hidden="true" />{text.loading}
    </div>}

    {issue && <div className="applications-notice applications-notice--error" role="alert">
      <span className="applications-notice__mark" aria-hidden="true">!</span>
      <div><strong>{errorMessage(issue, text)}</strong>
        {snapshot && <p>{text.previousResults}</p>}
      </div>
    </div>}

    {snapshot?.status === "partial" && <div className="applications-scope" role="note">
      <span className="applications-scope__mark" aria-hidden="true">i</span>{text.partialScope}
    </div>}

    {snapshot?.status === "not_scanned" && <div className="applications-notice" role="status">{text.notScanned}</div>}

    <div className="applications-grid" role="list" aria-label={language === "tr" ? "İstemci kurulumları" : "Client installations"}>
      {clients.map((client) => {
        const found = snapshot?.status === "partial" ? snapshot.installations.filter((item) => item.client_kind === client.kind) : [];
        const currentState = !isDiscoveryAvailable() ? "unavailable" : state.kind === "error" ? "unknown" :
          !readyState ? "pending" : snapshot?.status !== "partial" ? "unknown" : found.length > 0 ? "detected" : "not_found";
        return <ClientCard
          key={client.kind}
          client={client}
          found={found}
          state={currentState}
          text={text}
          mutationPending={mutationPending}
          selectingFile={selectingClient === client.kind}
          onAddFile={addFromFile}
          onConnect={openConnectionPreview}
        />;
      })}
    </div>

    {snapshot?.status === "partial" && !mutationPending && snapshot.installations.some((item) => item.client_kind === "codex") && <ConnectionStatus
      key={`${snapshot.scan_generation}:${snapshot.installations.filter((item) => item.client_kind === "codex").map((item) => `${item.installation_id}-${item.surface}-${item.version ?? "unknown"}`).join(",")}`}
      language={language}
      installations={snapshot.installations.filter((item) => item.client_kind === "codex")}
      scanGeneration={snapshot.scan_generation}
    />}

    {connectionClient && <ConnectDialog
      language={language}
      clientKind={connectionClient}
      installations={snapshot?.status === "partial" ? snapshot.installations.filter((item) => item.client_kind === connectionClient) : []}
      scanGeneration={snapshot?.scan_generation ?? 0}
      onClose={closeConnectionPreview}
    />}

    <div className="applications-footer" role="status" aria-live="polite">
      <span className={`applications-status-dot ${snapshot ? "is-scanned" : ""}`} aria-hidden="true" />
      {state.kind === "unavailable" ? text.desktopRequired : snapshot ? `${text.found}: ${snapshot.installations.length}` :
        state.kind === "error" ? text.readFailed : selectingClient ? text.selectingFile : text.loading}
    </div>
  </section>;
}

function ClientCard({
  client,
  found,
  state,
  text,
  mutationPending,
  selectingFile,
  onAddFile,
  onConnect,
}: {
  client: (typeof clients)[number];
  found: InstallationSummary[];
  state: "unavailable" | "pending" | "unknown" | "detected" | "not_found";
  text: Copy;
  mutationPending: boolean;
  selectingFile: boolean;
  onAddFile: (clientKind: ClientKind) => void;
  onConnect: (clientKind: ClientKind) => void;
}) {
  const status = state === "detected" ? text.detected : state === "not_found" ? text.notFound :
    state === "unavailable" ? text.desktopRequired : state === "unknown" ? text.noResult : text.loading;
  const fileGuide = text.fileGuide.replace("{files}", commandFileNames[client.kind]);
  const connectDisabled = mutationPending || !isDiscoveryAvailable();
  return <article role="listitem" className={`application-card ${state === "detected" ? "is-detected" : ""}`}>
    <header className="application-card__header">
      <span className={`application-mark application-mark--${client.kind}`} aria-hidden="true">{client.mark}</span>
      <div className="application-card__heading"><h2>{client.title}</h2><span className={`application-card__status ${state === "detected" ? "is-detected" : ""}`}>{status}</span></div>
    </header>
    {state === "detected" ? <ul className="application-installations">
      {found.map((installation) => <li key={installation.installation_id}>
        <div className="application-installation__top"><span>{installation.surface === "desktop" ? text.trDesktop : text.trCli}</span>
          <span className="application-version">{installation.version ?? text.unknownVersion}</span></div>
        <div className="application-installation__source">{text.sources[installation.source]}</div>
        <div className="application-connection"><span className="application-connection__dot" aria-hidden="true" />{text.connectionUnverified}</div>
      </li>)}
    </ul> : <p className="application-card__empty">
      {state === "not_found" ? text.notFound : state === "unavailable" ? text.unavailableBody :
        state === "unknown" ? text.noResult : text.loading}
    </p>}
    <div className="application-card__actions">
      <button className="button button-primary" type="button" disabled={connectDisabled} title={connectDisabled ? text.actionUnavailable : undefined} onClick={() => onConnect(client.kind)}>{text.connect}</button>
      <button
        className="button"
        type="button"
        disabled={!isDiscoveryAvailable() || mutationPending}
        title={fileGuide}
        aria-label={`${text.addFile}: ${client.title}`}
        onClick={() => onAddFile(client.kind)}
      >{selectingFile ? text.selectingFile : text.addFile}</button>
    </div>
    <p className="application-card__file-hint">{fileGuide}</p>
  </article>;
}

function errorMessage(issue: OperationIssue, text: Copy): string {
  const result = issue.result;
  if (issue.source !== "initial" && result.kind === "busy") return text.busy;
  if (issue.source !== "initial" && typeof issue.source === "object") {
    const fileGuide = text.fileGuide.replace("{files}", commandFileNames[issue.source.clientKind]);
    if (result.kind === "discovery_error") {
      switch (result.error.error.code) {
        case "busy": return text.busy;
        case "validation_failed": return fileGuide;
        case "environment_blocked": return `${text.pickerBlocked} ${fileGuide}`;
      }
    }
    if (result.kind === "invalid_response") return `${text.pickerInvalid} ${fileGuide}`;
    if (result.kind === "request_failed") return `${text.pickerBlocked} ${fileGuide}`;
  }
  if (result.kind === "discovery_error") {
    switch (result.error.error.code) {
      case "busy": return text.busy;
      case "environment_blocked": return text.blocked;
      case "validation_failed": return text.validation;
    }
  }
  if (result.kind === "busy") return text.busy;
  return result.kind === "invalid_response" ? text.invalidResponse : text.readFailed;
}
