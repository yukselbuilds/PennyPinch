import React from "react";
import { createRoot, type Root } from "react-dom/client";
import { flushSync } from "react-dom";
import { createRetainedVersionsClient, type RetainedClientKind, type RetainedScope } from "../../src/lib/retainedVersions";
import { RetainedVersions } from "../../src/features/integrations/RetainedVersions";
import "../../src/styles.css";

type Scenario = "normal" | "unsupported" | "unresolved" | "limit" | "error";
type Language = "tr" | "en";

declare global {
  interface Window { RetainedVersionsFixture: typeof fixtureApi; __retainedNativeCalls?: number }
}

let root: Root | null = null;
let host: HTMLElement | null = null;
let clientKind: RetainedClientKind = "claude_code";
let scope: RetainedScope = "user_config";
let language: Language = "tr";
let scenario: Scenario = "normal";
let disabled = false;
let requestNumber = 0;
let archiveGate: { promise: Promise<void>; release: () => void } | null = null;
const readGates = new Map<RetainedScope, { promise: Promise<void>; release: () => void }>();
const calls: Array<{ command: string; request: Record<string, unknown> }> = [];
let archivedCallbacks = 0;
let busyTransitions: boolean[] = [];

function deferred() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => { release = resolve; });
  return { promise, release };
}

function responseFor(command: string, request: Record<string, unknown>) {
  if (command === "get_retained_config_versions" && scenario === "error") {
    return {
      schema_version: 1, request_id: request.request_id, status: "error",
      error: { code: "busy", retryable: true, message_key: "retained.busy", diagnostic_id: null },
    };
  }

  const archived = command === "archive_retained_config_versions";
  const unsupported = scenario === "unsupported";
  const selected = unsupported || archived ? 0 : request.scope === "project_local_config" ? 2 : 1;
  const active = unsupported ? 0 : archived ? 1 : request.scope === "project_local_config" ? 3 : 2;
  const unresolved = scenario === "unresolved" && !archived ? 1 : 0;
  const completed = scenario === "limit" ? 64 : archived ? 1 : 0;
  const canArchive = !unsupported && !archived && scenario !== "limit" && unresolved === 0;
  return {
    schema_version: 1,
    request_id: request.request_id,
    status: archived ? "archived" : "snapshot",
    client_kind: request.client_kind,
    scope: request.scope,
    supported: !unsupported,
    review_fingerprint: unsupported ? null : "c".repeat(64),
    active_records: active,
    selected_records: selected,
    unresolved_records: unresolved,
    changed_versions: unsupported || archived ? 0 : 1,
    completed_archives: completed,
    record_limit: 64,
    archive_limit: 64,
    can_archive: canArchive,
  };
}

async function invoke(command: string, args?: Record<string, any>) {
  const request = args?.request ?? {};
  calls.push({ command, request: { ...request } });
  if (command === "get_retained_config_versions") {
    const gate = readGates.get(request.scope as RetainedScope);
    if (gate) {
      await gate.promise;
      if (readGates.get(request.scope as RetainedScope) === gate) readGates.delete(request.scope as RetainedScope);
    }
  } else if (command === "archive_retained_config_versions") {
    if (archiveGate) {
      await archiveGate.promise;
      archiveGate = null;
    }
  } else {
    throw new Error("fixture_only_supported_retained_commands");
  }
  return responseFor(command, request);
}

const client = createRetainedVersionsClient({
  isNative: () => true,
  createRequestId: () => `123e4567-e89b-42d3-a456-${(426614174000 + requestNumber++).toString(16).padStart(12, "0")}`,
  invoke,
});

function render() {
  if (!root) return;
  flushSync(() => root?.render(<RetainedVersions
    clientKind={clientKind}
    scope={scope}
    language={language}
    disabled={disabled}
    client={client}
    onArchived={() => { archivedCallbacks += 1; }}
    onBusyChange={(busy) => { busyTransitions.push(busy); }}
  />));
}

const fixtureApi = {
  mount(options: { clientKind?: RetainedClientKind; scope?: RetainedScope; language?: Language; scenario?: Scenario; disabled?: boolean; heldReads?: RetainedScope[] } = {}) {
    root?.unmount();
    host?.remove();
    host = document.createElement("div");
    host.id = "retained-root";
    document.body.append(host);
    root = createRoot(host);
    clientKind = options.clientKind ?? "claude_code";
    scope = options.scope ?? "user_config";
    language = options.language ?? "tr";
    scenario = options.scenario ?? "normal";
    disabled = options.disabled ?? false;
    requestNumber = 0;
    calls.length = 0;
    readGates.clear();
    for (const held of options.heldReads ?? []) readGates.set(held, deferred());
    archiveGate = null;
    archivedCallbacks = 0;
    busyTransitions = [];
    window.__retainedNativeCalls = 0;
    document.documentElement.lang = language;
    document.documentElement.dataset.theme = "dark";
    render();
  },
  setContext(next: { clientKind?: RetainedClientKind; scope?: RetainedScope; language?: Language }) {
    clientKind = next.clientKind ?? clientKind;
    scope = next.scope ?? scope;
    language = next.language ?? language;
    document.documentElement.lang = language;
    render();
  },
  setScenario(next: Scenario) { scenario = next; },
  setDisabled(next: boolean) { disabled = next; render(); },
  holdRead(nextScope: RetainedScope) { readGates.set(nextScope, deferred()); },
  releaseRead(nextScope: RetainedScope) { readGates.get(nextScope)?.release(); },
  holdArchive() { archiveGate = deferred(); },
  releaseArchive() { archiveGate?.release(); },
  calls() { return calls.map((call) => ({ command: call.command, request: { ...call.request } })); },
  archivedCallbacks() { return archivedCallbacks; },
  busyTransitions() { return [...busyTransitions]; },
  nativeCalls() { return window.__retainedNativeCalls ?? 0; },
};

window.RetainedVersionsFixture = fixtureApi;
export { fixtureApi as retainedVersionsFixtureApi };
