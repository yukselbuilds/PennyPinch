import { useEffect, useMemo, useRef, useState } from "react";
import type { PreferenceLanguage } from "../../lib/preferences";
import type { ClientKind, InstallationSummary } from "../../lib/integrations";
import { accountClient, type Account, type AccountResult, type LoginFlow } from "../../lib/accounts";
import "./accounts.css";

type ReadState =
  | { kind: "loading"; installationId: string }
  | { kind: "ready"; installationId: string; account: Account }
  | { kind: "error"; installationId: string; code: Extract<AccountResult, { kind: "error" }>["code"] };

type SelectionMap = Record<ClientKind, string | null>;
type ReadMap = Partial<Record<ClientKind, ReadState>>;
type AccountErrorCode = Extract<AccountResult, { kind: "error" }>["code"];
type FlowNotice = Exclude<LoginFlow["state"], "pending">;

function unresolved(flow: LoginFlow | null | undefined): boolean {
  return flow?.state === "pending" || flow?.state === "cleanup_failed";
}

function terminalNotice(state: LoginFlow["state"]): FlowNotice | null {
  return state === "pending" ? null : state;
}
const clientKinds: readonly ClientKind[] = ["codex", "claude_code", "opencode"];

const copy = {
  tr: {
    title: "Hesaplar",
    intro: "Yalnız seçilen CLI profilindeki yerel giriş kaydı görüntülenir; sağlayıcıya bağlantı doğrulanmaz.",
    environment: "Bu profil, Headroom'un açıldığı ortamın CLI kaydıdır. Desktop uygulamasındaki veya diğer açık sohbetlerdeki hesapları doğrulamaz.",
    refresh: "Durumu yenile",
    refreshing: "Yenileniyor…",
    installation: "Kurulum",
    chooseInstallation: "Kurulum seçin",
    versionUnknown: "sürüm bilinmiyor",
    noInstallations: "Tarama henüz tamamlanmadı.",
    noClientInstallations: "Taramada bu istemci için kurulum bulunmadı. Bu, istemcinin desteklenmediği anlamına gelmez.",
    noSelection: "Durum bilinmiyor; devam etmek için bir kurulum seçin.",
    cli: "Komut satırı",
    desktop: "Masaüstü",
    statePresent: "Yerel giriş kaydı var",
    stateAbsent: "Yerel giriş kaydı yok",
    stateUnknown: "Hesap durumu bilinmiyor",
    loading: "Yerel kayıt okunuyor…",
    readError: "Yerel hesap kaydı okunamadı.",
    login: "Resmî girişi aç",
    loginUnavailable: "Bu kurulum için resmî giriş akışı desteklenmiyor.",
    officialCli: "Resmî CLI kaydı",
    localStore: "Yerel kayıt",
    method: "Kayıt türü",
    providers: "Sağlayıcı kayıtları",
    unknownReason: {
      unsupported: "Bu kurulum veya sürüm için hesap durumu desteklenmiyor.",
      environment_auth: "CLI ortam değişkeniyle kimlik kullanıyor; yerel giriş kaydı bilinmiyor.",
      read_not_passive: "Bu CLI'de durum sorgusu yerel ayarı değiştirebilir; kayıt okunmadı.",
      read_failed: "Yerel giriş kaydı okunamadı.",
      timeout: "Yerel giriş kaydı sorgusu zaman aşımına uğradı.",
      cleanup_failed: "Önceki giriş akışının kapanışı doğrulanamadı.",
      unavailable: "Yerel hesap durumu şu anda kullanılamıyor.",
      unsafe_store: "Yerel kayıt güvenli biçimde okunamadı.",
      invalid_store: "Yerel kayıt biçimi tanınmıyor.",
      none: "Hesap durumu bilinmiyor.",
    },
    consentTitle: "Resmî CLI girişini aç?",
    consentScope: "Seçilen profil",
    consentBody: "Resmî istemci mevcut oturumunu kapatabilir veya değiştirebilir. İptal etmek eski oturumu geri getirmeyebilir.",
    consentPassword: "Parola veya doğrulama kodunu Headroom'a girmeyin; bunları yalnız resmî istemci ekranında kullanın.",
    consentClose: "Headroom'dan tamamen çıkınca bu giriş akışı durur.",
    cancel: "Vazgeç",
    confirm: "Resmî girişi aç",
    starting: "Resmî istemci açılıyor…",
    flowPending: "Resmî giriş akışı açık. Hesap durumu doğrulanmış sayılmaz.",
    flowCancel: "Giriş akışını durdur",
    flowCancelling: "Akış durduruluyor…",
    flowFinished: "Resmî giriş akışı kapandı. Yerel kayıt yeniden okundu; sağlayıcıya giriş doğrulanmadı.",
    flowCancelled: "Resmî giriş akışı durduruldu. Yerel kayıt yeniden okundu.",
    flowFailed: "Resmî giriş akışı tamamlanmadı. Hesap durumu bilinmiyor.",
    flowTimedOut: "Resmî giriş akışının süresi doldu. Hesap durumu bilinmiyor.",
    flowCleanup: "Resmî giriş akışının kapanışı doğrulanamadı.",
    error: {
      desktop_unavailable: "Bu işlem Headroom masaüstü uygulamasında kullanılabilir.",
      invalid_response: "Hesap durumu yanıtı doğrulanamadı.",
      validation_failed: "Seçim artık geçerli değil. Kurulumları yenileyip tekrar deneyin.",
      busy: "Başka bir hesap işlemi sürüyor.",
      stale_selection: "Kurulum tarama sırasında değişti. Durumu yenileyin.",
      untrusted_installation: "Bu kurulum güvenli biçimde doğrulanamadı.",
      unsupported: "Bu kurulum için resmî giriş akışı desteklenmiyor.",
      unavailable: "Hesap işlemi şu anda kullanılamıyor.",
      launch_failed: "Resmî istemci açılamadı.",
      not_found: "Seçilen kurulum artık bulunamıyor. Taramayı yenileyin.",
    },
    methodNames: {
      chatgpt: "ChatGPT oturum kaydı",
      api_key: "API anahtarı kaydı",
      oauth: "OAuth kaydı",
      access_token: "Erişim belirteci kaydı",
      provider_credentials: "Sağlayıcı kimlik bilgisi kaydı",
      none: "Kayıt yok",
      unknown: "Bilinmiyor",
    },
    providerNames: {
      openai: "OpenAI",
      anthropic: "Anthropic",
      google: "Google",
      opencode: "OpenCode",
      "github-copilot": "GitHub Copilot",
      "amazon-bedrock": "Amazon Bedrock",
      azure: "Azure",
      other: "Diğer sağlayıcı",
    },
    providerMethods: { api_key: "API anahtarı", oauth: "OAuth", provider_token: "Sağlayıcı oturumu" },
    sourceUnknown: "Kaynak bilinmiyor",
  },
  en: {
    title: "Accounts",
    intro: "Only the local sign-in record for the selected CLI profile is shown; no provider connection is verified.",
    environment: "This profile belongs to the CLI environment where Headroom was opened. It does not verify accounts in the Desktop app or other open chats.",
    refresh: "Refresh status",
    refreshing: "Refreshing…",
    installation: "Installation",
    chooseInstallation: "Choose an installation",
    versionUnknown: "version unknown",
    noInstallations: "The scan has not completed yet.",
    noClientInstallations: "No installation for this client appeared in the scan. This does not mean the client is unsupported.",
    noSelection: "Status is unknown; choose an installation to continue.",
    cli: "Command line",
    desktop: "Desktop",
    statePresent: "Local sign-in record found",
    stateAbsent: "No local sign-in record",
    stateUnknown: "Account status is unknown",
    loading: "Reading local record…",
    readError: "The local account record could not be read.",
    login: "Open official sign-in",
    loginUnavailable: "Official sign-in is not supported for this installation.",
    officialCli: "Official CLI record",
    localStore: "Local store",
    method: "Record type",
    providers: "Provider records",
    unknownReason: {
      unsupported: "Account status is not supported for this installation or version.",
      environment_auth: "The CLI uses environment credentials; its local sign-in record is unknown.",
      read_not_passive: "Checking status could change local settings for this CLI; its record was not read.",
      read_failed: "The local sign-in record could not be read.",
      timeout: "The local sign-in record check timed out.",
      cleanup_failed: "The previous sign-in flow could not be confirmed closed.",
      unavailable: "Local account status is currently unavailable.",
      unsafe_store: "The local record could not be read safely.",
      invalid_store: "The local record format is not recognized.",
      none: "Account status is unknown.",
    },
    consentTitle: "Open official CLI sign-in?",
    consentScope: "Selected profile",
    consentBody: "The official client may sign out or change its current session. Cancelling may not restore the previous session.",
    consentPassword: "Do not enter a password or verification code in Headroom; use only the official client screen.",
    consentClose: "Exiting Headroom stops this sign-in flow.",
    cancel: "Cancel",
    confirm: "Open official sign-in",
    starting: "Opening the official client…",
    flowPending: "The official sign-in flow is open. This does not verify account status.",
    flowCancel: "Stop sign-in flow",
    flowCancelling: "Stopping flow…",
    flowFinished: "The official sign-in flow closed. The local record was read again; provider sign-in is not verified.",
    flowCancelled: "The official sign-in flow was stopped. The local record was read again.",
    flowFailed: "The official sign-in flow did not complete. Account status is unknown.",
    flowTimedOut: "The official sign-in flow timed out. Account status is unknown.",
    flowCleanup: "The official sign-in flow could not be confirmed closed.",
    error: {
      desktop_unavailable: "This action is available in the Headroom desktop app.",
      invalid_response: "The account status response could not be validated.",
      validation_failed: "The selection is no longer valid. Refresh installations and try again.",
      busy: "Another account action is in progress.",
      stale_selection: "The installation changed during scanning. Refresh its status.",
      untrusted_installation: "This installation could not be verified safely.",
      unsupported: "Official sign-in is not supported for this installation.",
      unavailable: "Account actions are currently unavailable.",
      launch_failed: "The official client could not be opened.",
      not_found: "The selected installation is no longer available. Refresh the scan.",
    },
    methodNames: {
      chatgpt: "ChatGPT session record",
      api_key: "API key record",
      oauth: "OAuth record",
      access_token: "Access token record",
      provider_credentials: "Provider credential record",
      none: "No record",
      unknown: "Unknown",
    },
    providerNames: {
      openai: "OpenAI",
      anthropic: "Anthropic",
      google: "Google",
      opencode: "OpenCode",
      "github-copilot": "GitHub Copilot",
      "amazon-bedrock": "Amazon Bedrock",
      azure: "Azure",
      other: "Other provider",
    },
    providerMethods: { api_key: "API key", oauth: "OAuth", provider_token: "Provider session" },
    sourceUnknown: "Source unknown",
  },
} as const;

const clientNames: Record<ClientKind, { tr: string; en: string }> = {
  codex: { tr: "Codex", en: "Codex" },
  claude_code: { tr: "Claude Code", en: "Claude Code" },
  opencode: { tr: "OpenCode", en: "OpenCode" },
};

function flowNoticeText(state: FlowNotice, text: { flowFinished: string; flowCancelled: string; flowTimedOut: string; flowCleanup: string; flowFailed: string }): string {
  if (state === "finished") return text.flowFinished;
  if (state === "cancelled") return text.flowCancelled;
  if (state === "timed_out") return text.flowTimedOut;
  if (state === "cleanup_failed") return text.flowCleanup;
  return text.flowFailed;
}

function preferredInstallation(items: InstallationSummary[]): InstallationSummary | null {
  const cli = items.filter((item) => item.surface === "cli");
  return cli.find((item) => item.source === "user_local_native") ?? cli[0] ?? items[0] ?? null;
}

function selectedMap(items: InstallationSummary[], current: SelectionMap): SelectionMap {
  const next = {} as SelectionMap;
  for (const kind of clientKinds) {
    const choices = items.filter((item) => item.client_kind === kind);
    const currentId = current[kind];
    if (currentId && choices.some((item) => item.installation_id === currentId)) next[kind] = currentId;
    else next[kind] = preferredInstallation(choices)?.installation_id ?? null;
  }
  return next;
}

export function Accounts({
  language,
  installations,
  scanGeneration,
  disabled = false,
  onBusyChange,
}: {
  language: PreferenceLanguage;
  installations: InstallationSummary[];
  scanGeneration: number;
  disabled?: boolean;
  onBusyChange?: (busy: boolean) => void;
}) {
  const text = copy[language];
  const candidatesByKind = useMemo(() => Object.fromEntries(clientKinds.map((kind) => [
    kind,
    installations.filter((item) => item.client_kind === kind),
  ])) as Record<ClientKind, InstallationSummary[]>, [installations]);
  const candidateKey = useMemo(() => JSON.stringify(installations.map((item) => ({
    installation_id: item.installation_id,
    client_kind: item.client_kind,
    surface: item.surface,
    version: item.version,
    source: item.source,
    location_label: item.location_label,
  })).sort((left, right) => left.installation_id.localeCompare(right.installation_id))), [installations]);
  const [selection, setSelection] = useState<SelectionMap>({ codex: null, claude_code: null, opencode: null });
  const selectedRef = useRef<SelectionMap>({ codex: null, claude_code: null, opencode: null });
  const [reads, setReads] = useState<ReadMap>({});
  const [flow, setFlow] = useState<LoginFlow | null>(null);
  const flowRef = useRef<LoginFlow | null>(null);
  const [issue, setIssue] = useState<AccountErrorCode | null>(null);
  const [flowNotice, setFlowNotice] = useState<FlowNotice | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const confirmOpenRef = useRef(false);
  const [cancelPending, setCancelPending] = useState(false);
  const cancelPendingRef = useRef(false);
  const [starting, setStarting] = useState(false);
  const [initializing, setInitializing] = useState(true);
  const sequence = useRef(0);
  const rowSequence = useRef<Record<ClientKind, number>>({ codex: 0, claude_code: 0, opencode: 0 });
  const contextRef = useRef<{ generation: number; candidateKey: string } | null>(null);
  const mounted = useRef(false);
  const operationCount = useRef(0);
  const notifiedBusy = useRef(false);
  const onBusyRef = useRef(onBusyChange);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);

  function notifyBusy() {
    const next = operationCount.current > 0 || confirmOpenRef.current || unresolved(flowRef.current);
    if (next !== notifiedBusy.current && mounted.current) {
      notifiedBusy.current = next;
      onBusyRef.current?.(next);
    }
  }

  function setCurrentFlow(next: LoginFlow | null) {
    flowRef.current = next;
    setFlow(next);
    notifyBusy();
  }

  function setDialogOpen(next: boolean) {
    confirmOpenRef.current = next;
    setConfirmOpen(next);
    notifyBusy();
  }

  async function serialized<T>(action: () => Promise<T>): Promise<T> {
    const run = queue.current.then(action, action);
    queue.current = run.then(() => undefined, () => undefined);
    return run;
  }

  async function withOperation<T>(action: () => Promise<T>): Promise<T> {
    operationCount.current += 1;
    notifyBusy();
    try {
      return await action();
    } finally {
      operationCount.current = Math.max(0, operationCount.current - 1);
      notifyBusy();
    }
  }

  function isCurrent(contextSequence: number): boolean {
    return mounted.current && sequence.current === contextSequence;
  }

  function setRead(kind: ClientKind, next: ReadState) {
    setReads((previous) => ({ ...previous, [kind]: next }));
  }

  async function readOne(kind: ClientKind, installation: InstallationSummary, generation: number, contextSequence: number) {
    const rowToken = ++rowSequence.current[kind];
    setRead(kind, { kind: "loading", installationId: installation.installation_id });
    const result = await serialized(() => accountClient.read(installation, generation));
    if (!isCurrent(contextSequence) || rowSequence.current[kind] !== rowToken || selectedRef.current[kind] !== installation.installation_id) return;
    if (result.kind === "ok" && result.account) setRead(kind, { kind: "ready", installationId: installation.installation_id, account: result.account });
    else if (result.kind === "ok") setRead(kind, { kind: "error", installationId: installation.installation_id, code: "invalid_response" });
    else if (result.kind === "error") setRead(kind, { kind: "error", installationId: installation.installation_id, code: result.code });
  }

  async function readSelected(contextSequence: number, currentSelection = selectedRef.current, generation = scanGeneration) {
    setReads({});
    for (const kind of clientKinds) {
      if (!isCurrent(contextSequence)) return;
      const id = currentSelection[kind];
      const installation = id ? candidatesByKind[kind].find((item) => item.installation_id === id) : undefined;
      if (!installation) continue;
      await readOne(kind, installation, generation, contextSequence);
    }
  }

  useEffect(() => {
    onBusyRef.current = onBusyChange;
    if (notifiedBusy.current) onBusyRef.current?.(true);
  }, [onBusyChange]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      sequence.current += 1;
      rowSequence.current = { codex: rowSequence.current.codex + 1, claude_code: rowSequence.current.claude_code + 1, opencode: rowSequence.current.opencode + 1 };
      if (notifiedBusy.current) {
        notifiedBusy.current = false;
        onBusyRef.current?.(false);
      }
      const dialog = dialogRef.current;
      if (dialog?.open) dialog.close();
      if (returnFocus.current?.isConnected) returnFocus.current.focus();
    };
  }, []);

  useEffect(() => {
    if (disabled) {
      contextRef.current = null;
      setReads({});
      setInitializing(false);
      setDialogOpen(false);
      return;
    }
    const previous = contextRef.current;
    if (previous && previous.generation === scanGeneration && previous.candidateKey === candidateKey) return;

    const contextSequence = ++sequence.current;
    const inspectExistingFlow = previous === null || previous.generation !== scanGeneration;
    const nextSelection = selectedMap(installations, selectedRef.current);
    contextRef.current = { generation: scanGeneration, candidateKey };
    selectedRef.current = nextSelection;
    setSelection(nextSelection);
    setReads({});
    setIssue(null);
    setFlowNotice(null);
    setDialogOpen(false);
    setInitializing(true);
    if (inspectExistingFlow) setCurrentFlow(null);

    void withOperation(async () => {
      if (inspectExistingFlow) {
        const active = await serialized(() => accountClient.active());
        if (!isCurrent(contextSequence)) return;
        if (active.kind === "error") {
          setIssue(active.code);
        } else if (unresolved(active.flow)) {
          setCurrentFlow(active.flow);
          return;
        } else if (active.flow) {
          setFlowNotice(terminalNotice(active.flow.state));
        }
      } else if (unresolved(flowRef.current)) {
        return;
      }
      await readSelected(contextSequence, nextSelection, scanGeneration);
    }).finally(() => {
      if (isCurrent(contextSequence)) setInitializing(false);
    });

    return () => { if (sequence.current === contextSequence) sequence.current += 1; };
  }, [scanGeneration, candidateKey, disabled]);

  useEffect(() => {
    if (flow?.state !== "pending") return;
    let inFlight = false;
    const flowId = flow.flow_id;
    const timer = window.setInterval(() => {
      if (inFlight || !mounted.current || flowRef.current?.flow_id !== flowId) return;
      inFlight = true;
      const contextSequence = sequence.current;
      void withOperation(async () => {
        const result = await serialized(() => accountClient.poll(flow));
        if (!isCurrent(contextSequence) || flowRef.current?.flow_id !== flowId) return;
        if (result.kind === "error") {
          setIssue(result.code);
          return;
        }
        setIssue(null);
        const nextFlow = result.flow;
        if (!nextFlow) return;
        if (unresolved(nextFlow)) {
          setCurrentFlow(nextFlow);
          return;
        }
        setCurrentFlow(null);
        setIssue(null);
        setFlowNotice(terminalNotice(nextFlow.state));
        await readSelected(contextSequence);
      }).finally(() => { inFlight = false; });
    }, 1000);
    return () => window.clearInterval(timer);
  }, [flow]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!confirmOpen || !dialog) return;
    if (!dialog.open) dialog.showModal();
    cancelButtonRef.current?.focus();
    return () => {
      if (dialog.open) dialog.close();
      if (returnFocus.current?.isConnected) returnFocus.current.focus();
    };
  }, [confirmOpen]);

  function installationLabel(installation: InstallationSummary): string {
    const surface = installation.surface === "cli" ? text.cli : text.desktop;
    const version = installation.version ?? text.versionUnknown;
    return `${surface} · ${version}`;
  }

  function accountStatus(kind: ClientKind, hasCandidate: boolean, read: ReadState | undefined): string {
    if (!hasCandidate) return candidatesByKind[kind].length ? text.noSelection : scanGeneration < 1 ? text.noInstallations : text.noClientInstallations;
    if (read?.kind === "loading" || initializing) return text.loading;
    if (!read) return text.stateUnknown;
    if (read.kind === "error") return text.error[read.code];
    if (read.account.state === "present") return text.statePresent;
    if (read.account.state === "absent") return text.stateAbsent;
    return text.stateUnknown;
  }

  function handleSelect(kind: ClientKind, id: string) {
    const next = { ...selectedRef.current, [kind]: id || null };
    selectedRef.current = next;
    setSelection(next);
    rowSequence.current[kind] += 1;
    if (id) setRead(kind, { kind: "loading", installationId: id });
    setIssue(null);
    setFlowNotice(null);
    setDialogOpen(false);
    if (!id) {
      setReads((previous) => {
        const nextReads = { ...previous };
        delete nextReads[kind];
        return nextReads;
      });
      return;
    }
    const installation = candidatesByKind[kind].find((item) => item.installation_id === id);
    if (!installation || unresolved(flowRef.current)) return;
    const contextSequence = sequence.current;
    void withOperation(() => readOne(kind, installation, scanGeneration, contextSequence));
  }

  async function refreshAll() {
    if (disabled || operationCount.current > 0 || unresolved(flowRef.current)) return;
    const contextSequence = sequence.current;
    setIssue(null);
    setFlowNotice(null);
    await withOperation(async () => {
      // A launch may have succeeded even if its IPC reply was lost.
      const active = await serialized(() => accountClient.active());
      if (!isCurrent(contextSequence)) return;
      if (active.kind === "error") { setIssue(active.code); return; }
      if (unresolved(active.flow)) { setReads({}); setCurrentFlow(active.flow); return; }
      await readSelected(contextSequence);
    });
  }

  function openConfirmation(kind: ClientKind) {
    if (disabled || confirmOpenRef.current || unresolved(flowRef.current) || operationCount.current > 0) return;
    const id = selectedRef.current[kind];
    const candidate = id ? candidatesByKind[kind].find((item) => item.installation_id === id) : undefined;
    const read = reads[kind];
    if (!candidate || candidate.surface !== "cli" || read?.kind !== "ready" || !read.account.login_supported) return;
    returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setDialogOpen(true);
  }

  async function confirmLogin() {
    if (disabled || starting || operationCount.current > 0 || unresolved(flowRef.current)) return;
    const candidateId = selectedRef.current[dialogClientKindRef.current];
    const kind = dialogClientKindRef.current;
    const candidate = candidateId ? candidatesByKind[kind].find((item) => item.installation_id === candidateId) : undefined;
    const read = reads[kind];
    if (!candidate || candidate.surface !== "cli" || read?.kind !== "ready" || !read.account.login_supported) {
      setDialogOpen(false);
      setIssue("stale_selection");
      return;
    }
    const contextSequence = sequence.current;
    setStarting(true);
    setIssue(null);
    setFlowNotice(null);
    void withOperation(async () => {
      setDialogOpen(false);
      const result = await serialized(() => accountClient.start(candidate, scanGeneration, true));
      if (!isCurrent(contextSequence) || selectedRef.current[kind] !== candidate.installation_id) return;
      if (result.kind === "error") {
        setIssue(result.code);
        return;
      }
      if (result.flow?.state === "pending") { setReads({}); setCurrentFlow(result.flow); }
      else if (result.flow) {
        setCurrentFlow(null);
        setFlowNotice(result.flow.state);
        await readSelected(contextSequence);
      }
    }).finally(() => { if (isCurrent(contextSequence)) setStarting(false); });
  }

  const dialogClientKindRef = useRef<ClientKind>("codex");
  async function cancelFlow() {
    const active = flowRef.current;
    if (!active || !unresolved(active) || cancelPendingRef.current) return;
    const contextSequence = sequence.current;
    cancelPendingRef.current = true;
    setCancelPending(true);
    setIssue(null);
    await withOperation(async () => {
      const result = await serialized(() => accountClient.cancel(active));
      if (!isCurrent(contextSequence) || flowRef.current?.flow_id !== active.flow_id) return;
      if (result.kind === "error") {
        setIssue(result.code);
        return;
      }
      if (unresolved(result.flow)) {
        setCurrentFlow(result.flow);
        setIssue(null);
        return;
      }
      setCurrentFlow(null);
      setFlowNotice(terminalNotice(result.flow?.state ?? "cancelled"));
      await readSelected(contextSequence);
    }).finally(() => {
      cancelPendingRef.current = false;
      if (isCurrent(contextSequence)) setCancelPending(false);
    });
  }

  return <section className="accounts" aria-labelledby="accounts-title">
    <header className="accounts__header">
      <div>
        <p className="accounts__eyebrow">{language === "tr" ? "YEREL DURUM" : "LOCAL STATUS"}</p>
        <h2 id="accounts-title">{text.title}</h2>
        <p className="accounts__intro">{text.intro}</p>
      </div>
      <button className="button accounts__refresh" type="button" disabled={disabled || initializing || operationCount.current > 0 || unresolved(flow)} onClick={() => void refreshAll()}>
        {initializing || operationCount.current > 0 && flow?.state !== "pending" ? text.refreshing : text.refresh}
      </button>
    </header>

    <p className="accounts__environment">{text.environment}</p>

    {issue && <p className="accounts__notice accounts__notice--error" role="alert">{text.error[issue]}</p>}
    {flowNotice && <p className="accounts__notice" role="status" aria-live="polite">{flowNoticeText(flowNotice, text)}</p>}
    {unresolved(flow) && <div className="accounts__flow" role="status" aria-live="polite">
      <p>{flow?.state === "cleanup_failed" ? text.flowCleanup : text.flowPending}</p>
      <button className="button accounts__cancel-flow" type="button" disabled={cancelPending} onClick={() => void cancelFlow()}>
        {cancelPending ? text.flowCancelling : text.flowCancel}
      </button>
    </div>}

    <div className="accounts__list">
      {clientKinds.map((kind) => {
        const candidates = candidatesByKind[kind];
        const selectedId = selection[kind];
        const candidate = selectedId ? candidates.find((item) => item.installation_id === selectedId) : undefined;
        const storedRead = reads[kind];
        const read = !candidate || !storedRead || storedRead.installationId !== candidate.installation_id
          || (storedRead.kind === "ready" && (storedRead.account.client_kind !== candidate.client_kind || storedRead.account.surface !== candidate.surface))
          ? undefined : storedRead;
        const account = read?.kind === "ready" ? read.account : null;
        const status = accountStatus(kind, Boolean(candidate), read);
        const loading = read?.kind === "loading" || initializing;
        const canLogin = Boolean(candidate && candidate.surface === "cli" && account?.login_supported && !flow && !disabled && !loading && !operationCount.current);
        const tone = account?.state === "present" ? "present" : account?.state === "absent" ? "absent" : "unknown";
        return <article className="accounts__row" key={kind}>
          <div className="accounts__row-main">
            <div className="accounts__row-heading">
              <h3>{clientNames[kind][language]}</h3>
              <span className={`accounts__state accounts__state--${tone}`} aria-live="polite">{status}</span>
            </div>
            {candidates.length > 1 || (!selectedId && candidates.length > 0) ? <label className="accounts__installation">
              <span>{text.installation}</span>
              <select aria-label={`${text.installation}: ${clientNames[kind][language]}`} value={selectedId ?? ""} disabled={disabled || initializing || unresolved(flow) || operationCount.current > 0} onChange={(event) => handleSelect(kind, event.currentTarget.value)}>
                <option value="">{text.chooseInstallation}</option>
                {candidates.map((item) => <option key={item.installation_id} value={item.installation_id}>{installationLabel(item)}</option>)}
              </select>
            </label> : candidate ? <p className="accounts__candidate" title={installationLabel(candidate)}>{installationLabel(candidate)}</p>
              : <p className="accounts__candidate">{candidates.length ? text.noSelection : scanGeneration < 1 ? text.noInstallations : text.noClientInstallations}</p>}
            {candidate && candidates.length > 1 && <p className="accounts__candidate" title={installationLabel(candidate)}>{installationLabel(candidate)}</p>}
            {account && <div className="accounts__metadata">
              {account.state === "present" && <span>{text.method}: {text.methodNames[account.method]}</span>}
              {account.state === "unknown" && <span>{text.unknownReason[account.reason]}</span>}
              {account.state !== "unknown" && <span>{account.source === "official_cli" ? text.officialCli : account.source === "local_store" ? text.localStore : text.sourceUnknown}</span>}
              {account.providers.length > 0 && <span>{text.providers}: {account.providers.map((provider) => `${text.providerNames[provider.provider as keyof typeof text.providerNames] ?? text.providerNames.other} · ${text.providerMethods[provider.method]}`).join(", ")}</span>}
            </div>}
          </div>
          <div className="accounts__row-action">
            {account?.login_supported && candidate?.surface === "cli" ? <button className="button button-primary accounts__login" type="button" disabled={!canLogin} onClick={() => { dialogClientKindRef.current = kind; openConfirmation(kind); }}>
              {starting && dialogClientKindRef.current === kind ? text.starting : text.login}
            </button> : candidate && account && <span className="accounts__unsupported">{text.loginUnavailable}</span>}
          </div>
        </article>;
      })}
    </div>

    <dialog ref={dialogRef} className="accounts-dialog" tabIndex={-1} aria-labelledby="accounts-dialog-title" aria-describedby="accounts-dialog-body accounts-dialog-password accounts-dialog-close"
      onCancel={(event) => { event.preventDefault(); event.stopPropagation(); if (!starting) setDialogOpen(false); }}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        const controls = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not([disabled])"));
        const first = controls[0], last = controls[controls.length - 1];
        if (!first || !last) { event.preventDefault(); event.currentTarget.focus(); }
        else if (!event.currentTarget.contains(document.activeElement)) { event.preventDefault(); (event.shiftKey ? last : first).focus(); }
        else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }}>
      <div className="accounts-dialog__body">
        <p className="accounts-dialog__eyebrow">{clientNames[dialogClientKindRef.current][language]} · {candidateForDialog(selection, dialogClientKindRef.current, candidatesByKind, text)}</p>
        <h2 id="accounts-dialog-title">{text.consentTitle}</h2>
        <p>{dialogClientKindRef.current === "codex" ? (language === "tr" ? "ChatGPT hesabınla giriş açılır; mevcut API anahtarı veya hesap kaydı değişebilir." : "This opens ChatGPT sign-in and may replace an existing API-key or account record.")
          : dialogClientKindRef.current === "claude_code" ? (language === "tr" ? "Claude hesabınla resmî giriş açılır." : "This opens official sign-in with a Claude account.")
          : (language === "tr" ? "Sağlayıcı ve giriş yöntemini OpenCode ekranında seçersin." : "Choose the provider and sign-in method in OpenCode.")}</p>
        <p id="accounts-dialog-body">{text.consentBody}</p>
        <p id="accounts-dialog-password">{text.consentPassword}</p>
        <p id="accounts-dialog-close">{text.consentClose}</p>
        <div className="accounts-dialog__actions">
          <button ref={cancelButtonRef} className="button" type="button" disabled={starting} onClick={() => setDialogOpen(false)}>{text.cancel}</button>
          <button ref={confirmButtonRef} className="button button-primary" type="button" disabled={disabled || starting || operationCount.current > 0 || unresolved(flow)} onClick={() => void confirmLogin()}>{starting ? text.starting : text.confirm}</button>
        </div>
      </div>
    </dialog>
  </section>;
}

function candidateForDialog(
  selection: SelectionMap,
  kind: ClientKind,
  candidates: Record<ClientKind, InstallationSummary[]>,
  text: { cli: string; desktop: string; sourceUnknown: string; versionUnknown: string },
): string {
  const item = candidates[kind].find((candidate) => candidate.installation_id === selection[kind]);
  return item ? `${item.surface === "cli" ? text.cli : text.desktop} · ${item.version ?? text.versionUnknown}` : text.sourceUnknown;
}
