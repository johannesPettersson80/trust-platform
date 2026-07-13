import { affectsTrustConfiguration, getTrustConfiguration } from "./configuration";
import * as vscode from "vscode";
import * as net from "net";

import { ioPanelHtml } from "./io-panel/html";

import {
  EMPTY_ADS_LIVE_VALUES_STATE,
  normalizeAdsLiveValuesState,
} from "./adsLiveValuesModel";
import {
  summarizeAdsStatus,
  type AdsStatusReport,
  type AdsStatusSummary,
} from "./adsStatusSummary";
import { sendRuntimeControlRequest } from "./runtimeControlClient";
import {
  isStructuralRuntimeLifecycleChange,
  normalizeIoState,
  runtimeLifecycleService,
  type RuntimeLifecycleResult,
} from "./runtimeLifecycle";
import { sameRuntimeDebugSession } from "./runtimeSessionAuthority";
import { LatestOnlyRevision } from "./latestOnlyRevision";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
} from "./selectedRuntime";
import {
  remoteLabelFromEndpoint,
  SIMULATOR_RUNTIME_ID,
} from "./trustHomeModel";

const DEBUG_TYPE = "structured-text";
const PRAGMA_SCAN_LINES = 20;

type IoEntry = {
  name?: string;
  address: string;
  source?: string;
  value: string;
  forced?: boolean;
};

type IoState = {
  scan?: number;
  inputs: IoEntry[];
  outputs: IoEntry[];
  memory: IoEntry[];
};

type CompileIssue = {
  file: string;
  line: number;
  column: number;
  severity: "error" | "warning";
  message: string;
  code?: string;
  source?: string;
};

type CompileResult = {
  target: string;
  dirty: boolean;
  errors: number;
  warnings: number;
  issues: CompileIssue[];
  runtimeStatus: "ok" | "error" | "skipped";
  runtimeMessage?: string;
};

type RuntimeSourceOptions = {
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeRoot?: string;
};

type RuntimeStatusPayload = {
  running: boolean;
  inlineValuesEnabled: boolean;
  runtimeMode: "simulate" | "online";
  runtimeState: "running" | "connected" | "stopped";
  targetLabel?: string;
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
  access?: RuntimeAccessPayload;
  ads?: AdsStatusSummary;
};

type RuntimeAccessPayload = {
  role?: string;
  allowWrite: boolean;
  allowForce: boolean;
  allowRelease: boolean;
  reason?: string;
};

const ENDPOINT_PROBE_TTL_MS = 2000;
const ENDPOINT_PROBE_TIMEOUT_MS = 400;

type ParsedEndpoint =
  | { kind: "tcp"; host: string; port: number }
  | { kind: "unix"; path: string };

let endpointProbeCache:
  | { endpoint: string; reachable: boolean; checkedAt: number }
  | undefined;

function getStructuredTextSession(): vscode.DebugSession | undefined {
  return runtimeLifecycleService.acceptedDebugSession();
}


function parseControlEndpoint(endpoint: string): ParsedEndpoint | undefined {
  if (endpoint.startsWith("tcp://")) {
    try {
      const url = new URL(endpoint);
      const port = Number(url.port);
      if (!url.hostname || !Number.isFinite(port)) {
        return undefined;
      }
      return { kind: "tcp", host: url.hostname, port };
    } catch {
      return undefined;
    }
  }
  if (endpoint.startsWith("unix://")) {
    if (process.platform === "win32") {
      return undefined;
    }
    const path = endpoint.slice("unix://".length);
    if (!path) {
      return undefined;
    }
    return { kind: "unix", path };
  }
  return undefined;
}

function isLocalEndpoint(endpoint: string): boolean {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return false;
  }
  if (parsed.kind === "unix") {
    return true;
  }
  const host = parsed.host.toLowerCase();
  return host === "127.0.0.1" || host === "localhost" || host === "::1";
}

async function probeEndpointReachable(endpoint: string): Promise<boolean> {
  const now = Date.now();
  if (
    endpointProbeCache &&
    endpointProbeCache.endpoint === endpoint &&
    now - endpointProbeCache.checkedAt < ENDPOINT_PROBE_TTL_MS
  ) {
    return endpointProbeCache.reachable;
  }
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    endpointProbeCache = { endpoint, reachable: false, checkedAt: now };
    return false;
  }
  const reachable = await new Promise<boolean>((resolve) => {
    let settled = false;
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });
    const finish = (value: boolean) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(ENDPOINT_PROBE_TIMEOUT_MS, () => finish(false));
    socket.once("error", () => finish(false));
    socket.once("connect", () => finish(true));
  });
  endpointProbeCache = { endpoint, reachable, checkedAt: Date.now() };
  return reachable;
}

async function fetchRuntimeState(endpoint: string, authToken?: string): Promise<"running" | "stopped" | undefined> {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return undefined;
  }
  return new Promise((resolve) => {
    let settled = false;
    let buffer = "";
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });
    const finish = (value: "running" | "stopped" | undefined) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(ENDPOINT_PROBE_TIMEOUT_MS, () => finish(undefined));
    socket.once("error", () => finish(undefined));
    socket.once("connect", () => {
      const request = { id: 1, type: "status", auth: authToken || undefined };
      socket.write(JSON.stringify(request) + "\n");
    });
    socket.on("data", (chunk: Buffer | string) => {
      buffer += chunk.toString();
      const idx = buffer.indexOf("\n");
      if (idx == -1) {
        return;
      }
      const line = buffer.slice(0, idx).trim();
      if (!line) {
        finish(undefined);
        return;
      }
      try {
        const response = JSON.parse(line) as { ok?: boolean; result?: { state?: string } };
        if (response.ok && response.result && typeof response.result.state === "string") {
          const state = response.result.state.toLowerCase();
          finish(state === "running" ? "running" : "stopped");
          return;
        }
      } catch {
        // ignore parse errors
      }
      finish(undefined);
    });
  });
}

async function fetchAdsStatusSummary(
  endpoint: string,
  authToken?: string
): Promise<AdsStatusSummary | undefined> {
  try {
    const report = await sendRuntimeControlRequest<AdsStatusReport>(
      endpoint,
      authToken,
      "ads.status",
      undefined,
      { timeoutMs: 750 }
    );
    return summarizeAdsStatus(report);
  } catch {
    return undefined;
  }
}

let panel: vscode.WebviewPanel | undefined;
const liveValuesRevision = new LatestOnlyRevision();

export function registerIoPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.openIoPanel", () => {
      showPanel(context);
    })
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.openIoPanelSettings", () => {
      showPanel(context, { openSettings: true });
    })
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange((change) => {
      if (!panel || !isStructuralRuntimeLifecycleChange(change)) {
        return;
      }
      void refreshLiveValuesForLifecycle();
    })
  );
  context.subscriptions.push(
    onDidChangeSelectedRuntime(() => {
      if (panel) {
        void refreshLiveValuesForLifecycle();
      }
    })
  );

  context.subscriptions.push(
    vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
      if (event.event !== "stIoState" && event.event !== "stAdsState") {
        return;
      }
      if (event.session.type !== DEBUG_TYPE) {
        return;
      }
      if (!panel) {
        return;
      }
      const accepted = runtimeLifecycleService.acceptedDebugSession();
      if (!liveValuesEventIsAccepted(accepted, event.session)) {
        return;
      }
      if (event.event === "stIoState") {
        const body = event.body as IoState | undefined;
        panel.webview.postMessage({
          type: "ioState",
          payload: normalizeIoState(body),
        });
      } else {
        panel.webview.postMessage({
          type: "adsState",
          payload: normalizeAdsLiveValuesState(event.body),
        });
      }
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        affectsTrustConfiguration(event, "runtime.controlEndpoint") ||
        affectsTrustConfiguration(event, "runtime.controlEndpointEnabled") ||
        affectsTrustConfiguration(event, "runtime.inlineValuesEnabled") ||
        affectsTrustConfiguration(event, "runtime.mode")
      ) {
        void sendRuntimeStatus();
      }
    })
  );

}

type ShowPanelOptions = {
  openSettings?: boolean;
};

function liveValuesViewColumn(): vscode.ViewColumn {
  const activeTab = vscode.window.tabGroups.activeTabGroup.activeTab;
  if (activeTab?.label === "Devices & Connections") {
    return vscode.ViewColumn.Active;
  }
  return vscode.ViewColumn.Two;
}

function showPanel(
  context: vscode.ExtensionContext,
  options: ShowPanelOptions = {}
): void {
  if (panel) {
    panel.reveal();
    void requestLiveValuesState();
    void sendRuntimeStatus();
    if (options.openSettings) {
      panel.webview.postMessage({ type: "openSettings" });
    }
    return;
  }

  panel = vscode.window.createWebviewPanel(
    "trust-io-panel",
    "Live Values",
    liveValuesViewColumn(),
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [
        vscode.Uri.joinPath(context.extensionUri, "media"),
        vscode.Uri.joinPath(context.extensionUri, "node_modules"),
      ],
    }
  );

  panel.webview.html = ioPanelHtml(panel.webview, context.extensionUri);
  panel.onDidDispose(() => {
    liveValuesRevision.invalidate();
    panel = undefined;
  });

  panel.webview.onDidReceiveMessage(handleWebviewMessage);

  void requestLiveValuesState();
  void sendRuntimeStatus();
  if (options.openSettings) {
    panel.webview.postMessage({ type: "openSettings" });
  }

  context.subscriptions.push(panel);
}

function postPanelStatus(message: string): void {
  panel?.webview.postMessage({
    type: "status",
    payload: userFacingIoStatus(message),
  });
}

function userFacingIoStatus(message: string): string {
  if (isNoActiveSessionMessage(message)) {
    return "Start the Simulator to see live values.";
  }
  if (isIoStateTransportFailureMessage(message)) {
    return "Live Values lost connection to the runtime. Restart or reconnect the runtime, then retry.";
  }
  return message;
}

function liveValuesUnavailableMessage(
  status: RuntimeStatusPayload | undefined,
  selectedTargetId = getSelectedRuntimeId(),
): string {
  if (
    selectedTargetId !== SIMULATOR_RUNTIME_ID &&
    selectedTargetUsesControlEndpoint(selectedTargetId)
  ) {
    return "Connect to the selected runtime to see live values.";
  }
  if (
    selectedTargetId !== SIMULATOR_RUNTIME_ID ||
    (status?.runtimeMode === "online" && status.runtimeState !== "connected")
  ) {
    return "Start the selected runtime to see live values.";
  }
  return "Start the Simulator to see live values.";
}

function selectedTargetUsesControlEndpoint(targetId: string): boolean {
  return /^(?:tcp|unix):\/\//i.test(targetId.trim());
}

function statusForSelectedTarget(
  status: RuntimeStatusPayload,
  selectedTargetId = getSelectedRuntimeId(),
): RuntimeStatusPayload {
  if (selectedTargetId === SIMULATOR_RUNTIME_ID) {
    return {
      ...status,
      running: false,
      runtimeMode: "simulate",
      runtimeState: "stopped",
      targetLabel: "Simulator",
      endpoint: "",
      endpointConfigured: false,
      endpointReachable: false,
    };
  }
  if (selectedTargetUsesControlEndpoint(selectedTargetId)) {
    return {
      ...status,
      running: false,
      runtimeMode: "online",
      runtimeState: "stopped",
      targetLabel: remoteLabelFromEndpoint(selectedTargetId),
      endpoint: selectedTargetId,
      endpointConfigured: true,
      endpointReachable: false,
    };
  }
  return {
    ...status,
    running: false,
    runtimeMode: "simulate",
    runtimeState: "stopped",
    targetLabel: selectedTargetId,
    endpointReachable: false,
  };
}

export function __testLiveValuesUnavailableMessage(
  status: RuntimeStatusPayload | undefined,
  selectedTargetId: string,
): string {
  return liveValuesUnavailableMessage(status, selectedTargetId);
}

export function __testStatusForSelectedTarget(
  status: RuntimeStatusPayload,
  selectedTargetId: string,
): RuntimeStatusPayload {
  return statusForSelectedTarget(status, selectedTargetId);
}

function isNoActiveSessionMessage(message: string): boolean {
  return (
    /No active Structured Text debug session/i.test(message) ||
    /No debugger available/i.test(message) ||
    /can\s+not\s+send\s+['"]?stIoState['"]?/i.test(message) ||
    /I\/O state request failed:\s*Canceled/i.test(message)
  );
}

function isIoStateTransportFailureMessage(message: string): boolean {
  return (
    /I\/O state request failed:/i.test(message) &&
    /cancelled|connection|socket|ECONNRESET|ECONNREFUSED|EPIPE|closed|terminated|timed?\s*out|timeout|not connected|disconnected/i.test(
      message
    )
  );
}

function postEmptyIoState(): void {
  panel?.webview.postMessage({
    type: "ioState",
    payload: { inputs: [], outputs: [], memory: [] },
  });
}

function postEmptyAdsState(): void {
  panel?.webview.postMessage({
    type: "adsState",
    payload: EMPTY_ADS_LIVE_VALUES_STATE,
  });
}

function postUnavailableLiveValues(
  status?: RuntimeStatusPayload,
  message?: string
): void {
  const statusMessage = message || liveValuesUnavailableMessage(status);
  if (status) {
    panel?.webview.postMessage({
      type: "runtimeStatus",
      payload: statusForSelectedTarget(status),
    });
  }
  postEmptyIoState();
  postEmptyAdsState();
  panel?.webview.postMessage({
    type: "status",
    payload: statusMessage,
  });
}

function handleWebviewMessage(message: any): void {
  const type = typeof message?.type === "string" ? message.type : "";
  switch (type) {
    case "refresh":
      void requestLiveValuesState();
      break;
    case "writeInput":
      void writeInput(String(message.address || ""), String(message.value || ""));
      break;
    case "forceInput":
      void forceInput(String(message.address || ""), String(message.value || ""));
      break;
    case "releaseInput":
      void releaseInput(String(message.address || ""));
      break;
    case "releaseAllForces":
      void releaseAllForces(
        Array.isArray(message.addresses)
          ? message.addresses.map((a: unknown) => String(a))
          : []
      );
      break;
    case "requestSettings":
      panel?.webview.postMessage({
        type: "settings",
        payload: collectSettingsSnapshot(),
      });
      break;
    case "saveSettings":
      void applySettingsUpdate(message.payload);
      break;
    case "webviewError": {
      const detail =
        typeof message.message === "string" ? message.message : "Unknown error";
      console.error("Live Values webview error:", detail, message.stack || "");
      postPanelStatus(`Live Values error: ${detail}`);
      break;
    }
    case "webviewReady":
      console.info("Live Values webview ready.");
      void sendRuntimeStatus();
      void requestLiveValuesState();
      break;
    default:
      break;
  }
}

type SettingsPayload = {
  serverPath?: string;
  traceServer?: string;
  debugAdapterPath?: string;
  debugAdapterArgs?: string[];
  debugAdapterEnv?: Record<string, string>;
  runtimeControlEndpoint?: string;
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeInlineValuesEnabled?: boolean;
};

function collectSettingsSnapshot(): SettingsPayload {
  const config = getTrustConfiguration();
  return {
    serverPath: config.get<string>("server.path") ?? "",
    traceServer: config.get<string>("trace.server") ?? "off",
    debugAdapterPath: config.get<string>("debug.adapter.path") ?? "",
    debugAdapterArgs: config.get<string[]>("debug.adapter.args") ?? [],
    debugAdapterEnv: config.get<Record<string, string>>("debug.adapter.env") ?? {},
    runtimeControlEndpoint: config.get<string>("runtime.controlEndpoint") ?? "",
    runtimeIncludeGlobs: config.get<string[]>("runtime.includeGlobs") ?? [],
    runtimeExcludeGlobs: config.get<string[]>("runtime.excludeGlobs") ?? [],
    runtimeIgnorePragmas: config.get<string[]>("runtime.ignorePragmas") ?? [],
    runtimeInlineValuesEnabled:
      config.get<boolean>("runtime.inlineValuesEnabled") ?? true,
  };
}

async function applySettingsUpdate(payload: SettingsPayload | undefined): Promise<void> {
  if (!payload) {
    return;
  }
  const config = getTrustConfiguration();
  const settingsUpdates: Array<{ key: string; value: unknown }> = [
    { key: "server.path", value: payload.serverPath?.trim() || undefined },
    { key: "trace.server", value: payload.traceServer?.trim() || "off" },
    {
      key: "debug.adapter.path",
      value: payload.debugAdapterPath?.trim() || undefined,
    },
    { key: "debug.adapter.args", value: payload.debugAdapterArgs ?? [] },
    { key: "debug.adapter.env", value: payload.debugAdapterEnv ?? {} },
    {
      key: "runtime.controlEndpoint",
      value: payload.runtimeControlEndpoint?.trim() || undefined,
    },
    { key: "runtime.includeGlobs", value: payload.runtimeIncludeGlobs ?? [] },
    { key: "runtime.excludeGlobs", value: payload.runtimeExcludeGlobs ?? [] },
    { key: "runtime.ignorePragmas", value: payload.runtimeIgnorePragmas ?? [] },
    {
      key: "runtime.inlineValuesEnabled",
      value: payload.runtimeInlineValuesEnabled ?? true,
    },
  ];
  for (const update of settingsUpdates) {
    await config.update(
      update.key,
      update.value,
      vscode.ConfigurationTarget.Workspace
    );
  }

  postPanelStatus("Settings saved.");
}

export async function __testApplySettingsUpdate(
  payload: SettingsPayload | undefined
): Promise<void> {
  await applySettingsUpdate(payload);
}

export function __testCollectSettingsSnapshot(): SettingsPayload {
  return collectSettingsSnapshot();
}

export function __testUserFacingIoStatus(message: string): string {
  return userFacingIoStatus(message);
}

export function liveValuesEventIsAccepted(
  accepted: vscode.DebugSession | undefined,
  candidate: vscode.DebugSession,
): boolean {
  return sameRuntimeDebugSession(accepted, candidate);
}

function runtimeConfigTarget(): vscode.Uri | undefined {
  return runtimeLifecycleService.runtimeConfigTarget();
}

function runtimeConfigScope(target: vscode.Uri | undefined): vscode.ConfigurationTarget {
  return runtimeLifecycleService.runtimeConfigScope(target);
}

async function runtimeStatusPayload(): Promise<RuntimeStatusPayload> {
  return (await runtimeLifecycleService.snapshot()).status;
}

async function sendRuntimeStatus(): Promise<void> {
  const panelRef = panel;
  if (!panelRef) {
    return;
  }
  const revision = liveValuesRevision.begin();
  const rawPayload = await runtimeStatusPayload();
  if (!liveValuesRevision.isCurrent(revision) || panel !== panelRef) {
    return;
  }
  const payload = runtimeLifecycleService.acceptedDebugSession()
    ? rawPayload
    : statusForSelectedTarget(rawPayload);
  panelRef.webview.postMessage({
    type: "runtimeStatus",
    payload,
  });
}

async function refreshLiveValuesForLifecycle(): Promise<void> {
  const panelRef = panel;
  if (!panelRef) {
    return;
  }
  const revision = liveValuesRevision.begin();
  const status = await runtimeStatusPayload();
  if (!liveValuesRevision.isCurrent(revision) || panel !== panelRef) {
    return;
  }
  if (!runtimeLifecycleService.acceptedDebugSession()) {
    postUnavailableLiveValues(status);
    return;
  }
  panelRef.webview.postMessage({ type: "runtimeStatus", payload: status });
  await requestLiveValuesState();
}

async function requestLiveValuesState(): Promise<void> {
  const result = await runtimeLifecycleService.requestLiveValuesState();
  await handleIoStateRequestResult(result);
}

async function requestIoStateAfterScan(previousScan: number | undefined): Promise<void> {
  const result = await runtimeLifecycleService.requestLiveValuesStateAfterScan(previousScan);
  await handleIoStateRequestResult(result);
}

async function currentIoScan(): Promise<number | undefined> {
  return (await runtimeLifecycleService.snapshot()).ioState.scan;
}

async function handleIoStateRequestResult(result: RuntimeLifecycleResult): Promise<void> {
  if (!result.ok) {
    if (/^ADS state request failed:/i.test(result.failure.message)) {
      postEmptyAdsState();
      panel?.webview.postMessage({
        type: "status",
        payload:
          "ADS variables could not be refreshed. Restart or reconnect the runtime, then retry.",
      });
      return;
    }
    if (isNoActiveSessionMessage(result.failure.message)) {
      const status = await runtimeStatusPayload().catch(() => undefined);
      postUnavailableLiveValues(status);
      return;
    }
    if (isIoStateTransportFailureMessage(result.failure.message)) {
      const status = await runtimeStatusPayload().catch(() => undefined);
      postUnavailableLiveValues(status, userFacingIoStatus(result.failure.message));
      return;
    }
    panel?.webview.postMessage({
      type: "status",
      payload: userFacingIoStatus(result.failure.message),
    });
    return;
  }
}

async function writeInput(address: string, value: string): Promise<void> {
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  try {
    const previousScan = await currentIoScan();
    await vscode.commands.executeCommand("trust-lsp.debug.io.write", {
      address,
      value,
    });
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O write queued for ${address}.`,
    });
    void requestIoStateAfterScan(previousScan);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O write failed: ${message}`,
    });
  }
}

async function forceInput(address: string, value: string): Promise<void> {
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  // Force works on the simulator AND on remote attach (the adapter forwards io.force; the runtime
  // authorizes by role and surfaces any error, which the catch below reports).
  try {
    const previousScan = await currentIoScan();
    await vscode.commands.executeCommand("trust-lsp.debug.io.force", {
      address,
      value,
    });
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O force active at ${address}.`,
    });
    void requestIoStateAfterScan(previousScan);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O force failed: ${message}`,
    });
  }
}

async function releaseInput(address: string): Promise<void> {
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  try {
    const previousScan = await currentIoScan();
    await vscode.commands.executeCommand("trust-lsp.debug.io.release", {
      address,
    });
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O force released at ${address}.`,
    });
    void requestIoStateAfterScan(previousScan);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O release failed: ${message}`,
    });
  }
}

// §0.5.16 — "Release all forces": one click clears every force on the target (simulator or remote
// attach). The webview sends the currently-forced addresses (it renders them); we release each.
async function releaseAllForces(addresses: string[]): Promise<void> {
  const previousScan = await currentIoScan();
  let released = 0;
  for (const address of addresses) {
    if (!address) {
      continue;
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.io.release", {
        address,
      });
      released += 1;
    } catch {
      // Release the rest even if one fails.
    }
  }
  panel?.webview.postMessage({
    type: "status",
    payload:
      released > 0
        ? `Released ${released} force${released === 1 ? "" : "s"}.`
        : "No forces to release.",
  });
  void requestIoStateAfterScan(previousScan);
}


function diagnosticCodeLabel(
  code: string | number | { value: string | number; target?: vscode.Uri } | undefined
): string | undefined {
  if (code === undefined) {
    return undefined;
  }
  if (typeof code === "string" || typeof code === "number") {
    return String(code);
  }
  if (typeof code === "object" && "value" in code) {
    return String(code.value);
  }
  return undefined;
}

async function readStructuredText(
  uri: vscode.Uri
): Promise<string | undefined> {
  const openDoc = vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString()
  );
  if (openDoc) {
    return openDoc.getText();
  }
  try {
    const data = await vscode.workspace.fs.readFile(uri);
    return new TextDecoder("utf-8").decode(data);
  } catch {
    return undefined;
  }
}

function containsConfiguration(source: string): boolean {
  return /\bCONFIGURATION\b/i.test(source);
}

async function sourcesContainConfiguration(
  uris: vscode.Uri[]
): Promise<boolean> {
  for (const uri of uris) {
    const text = await readStructuredText(uri);
    if (text && containsConfiguration(text)) {
      return true;
    }
  }
  return false;
}

async function collectRuntimeSources(
  targetDoc?: vscode.TextDocument
): Promise<vscode.Uri[]> {
  const runtimeOptions = runtimeSourceOptions(targetDoc?.uri);
  const includeGlobs = runtimeOptions.runtimeIncludeGlobs ?? [];
  const excludeGlobs = runtimeOptions.runtimeExcludeGlobs ?? [];
  const ignorePragmas = runtimeOptions.runtimeIgnorePragmas ?? [];
  const runtimeRoot =
    runtimeOptions.runtimeRoot ??
    (targetDoc
      ? vscode.workspace.getWorkspaceFolder(targetDoc.uri)?.uri.fsPath
      : vscode.workspace.workspaceFolders?.[0]?.uri.fsPath);
  if (!runtimeRoot) {
    return [];
  }

  const baseUri = vscode.Uri.file(runtimeRoot);
  const excludePattern = buildGlobAlternation(excludeGlobs);
  const exclude = excludePattern
    ? new vscode.RelativePattern(baseUri, excludePattern)
    : undefined;

  const candidates: vscode.Uri[] = [];
  for (const include of includeGlobs) {
    const pattern = new vscode.RelativePattern(baseUri, include);
    const matches = await vscode.workspace.findFiles(pattern, exclude);
    candidates.push(...matches);
  }

  const unique = new Map<string, vscode.Uri>();
  for (const candidate of candidates) {
    unique.set(candidate.fsPath, candidate);
  }
  if (targetDoc?.uri.fsPath) {
    unique.set(targetDoc.uri.fsPath, targetDoc.uri);
  }

  if (ignorePragmas.length === 0) {
    return Array.from(unique.values());
  }

  const filtered: vscode.Uri[] = [];
  for (const candidate of unique.values()) {
    if (
      targetDoc &&
      candidate.fsPath === targetDoc.uri.fsPath
    ) {
      filtered.push(candidate);
      continue;
    }
    if (await hasRuntimeIgnorePragma(candidate, ignorePragmas)) {
      continue;
    }
    filtered.push(candidate);
  }
  return filtered;
}

function buildGlobAlternation(globs: string[]): string | undefined {
  const normalized = globs.map((glob) => glob.trim()).filter(Boolean);
  if (normalized.length === 0) {
    return undefined;
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return `{${normalized.join(",")}}`;
}

async function hasRuntimeIgnorePragma(
  uri: vscode.Uri,
  pragmas: string[]
): Promise<boolean> {
  if (pragmas.length === 0) {
    return false;
  }
  const text = await readStructuredText(uri);
  if (!text) {
    return false;
  }
  const lines = text.split(/\r?\n/).slice(0, PRAGMA_SCAN_LINES);
  for (const line of lines) {
    for (const pragma of pragmas) {
      if (pragma && line.includes(pragma)) {
        return true;
      }
    }
  }
  return false;
}

function runtimeSourceOptions(target?: vscode.Uri): RuntimeSourceOptions {
  const config = getTrustConfiguration();
  const includeGlobs = normalizeStringArray(
    config.get<unknown>("runtime.includeGlobs")
  );
  const effectiveIncludeGlobs =
    includeGlobs.length > 0 ? includeGlobs : ["**/*.{st,ST,pou,POU}"];
  const excludeGlobs = normalizeStringArray(
    config.get<unknown>("runtime.excludeGlobs")
  );
  const ignorePragmas = normalizeStringArray(
    config.get<unknown>("runtime.ignorePragmas")
  );
  const folder = target
    ? vscode.workspace.getWorkspaceFolder(target)
    : vscode.workspace.workspaceFolders?.[0];
  const runtimeRoot = folder?.uri.fsPath;
  return {
    runtimeIncludeGlobs: effectiveIncludeGlobs,
    runtimeExcludeGlobs: excludeGlobs,
    runtimeIgnorePragmas: ignorePragmas,
    runtimeRoot,
  };
}

function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((item) => (typeof item === "string" ? item.trim() : ""))
    .filter((item) => item.length > 0);
}

async function compileActiveProgram(): Promise<void> {
  if (!panel) {
    return;
  }

  panel.webview.postMessage({
    type: "status",
    payload: "Compiling...",
  });

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    panel.webview.postMessage({
      type: "status",
      payload: "Open a workspace folder to compile.",
    });
    panel.webview.postMessage({
      type: "compileResult",
      payload: {
        target: "",
        dirty: false,
        errors: 0,
        warnings: 0,
        issues: [],
        runtimeStatus: "skipped",
        runtimeMessage: "No workspace folder open.",
      } satisfies CompileResult,
    });
    return;
  }

  const sourceUris = await collectRuntimeSources();
  const hasConfiguration = await sourcesContainConfiguration(sourceUris);
  if (sourceUris.length === 0) {
    panel.webview.postMessage({
      type: "status",
      payload: "No Structured Text files found in the workspace.",
    });
    panel.webview.postMessage({
      type: "compileResult",
      payload: {
        target: workspaceFolder.uri.fsPath,
        dirty: false,
        errors: 0,
        warnings: 0,
        issues: [],
        runtimeStatus: "skipped",
        runtimeMessage: "No Structured Text files found.",
      } satisfies CompileResult,
    });
    return;
  }

  let runtimeStatus: CompileResult["runtimeStatus"] = "skipped";
  let runtimeMessage: string | undefined;
  const session = getStructuredTextSession();
  if (session) {
    const program =
      typeof session.configuration?.program === "string"
        ? session.configuration.program
        : undefined;
    if (!program) {
      runtimeStatus = "error";
      runtimeMessage = "Active debug session missing entry configuration.";
    } else {
      runtimeStatus = "ok";
      try {
        const runtimeOptions = runtimeSourceOptions(vscode.Uri.file(program));
        await session.customRequest("stReload", {
          program,
          ...runtimeOptions,
        });
        runtimeMessage = "Runtime update succeeded.";
      } catch (err) {
        runtimeStatus = "error";
        const message = err instanceof Error ? err.message : String(err);
        runtimeMessage = `Runtime compile failed: ${message}`;
      }
    }
  }

  const issues: CompileIssue[] = [];
  for (const uri of sourceUris) {
    const fileDiagnostics = vscode.languages.getDiagnostics(uri);
    for (const diagnostic of fileDiagnostics) {
      if (
        diagnostic.severity !== vscode.DiagnosticSeverity.Error &&
        diagnostic.severity !== vscode.DiagnosticSeverity.Warning
      ) {
        continue;
      }
      issues.push({
        file: uri.fsPath,
        line: diagnostic.range.start.line + 1,
        column: diagnostic.range.start.character + 1,
        severity:
          diagnostic.severity === vscode.DiagnosticSeverity.Error
            ? "error"
            : "warning",
        message: diagnostic.message,
        code: diagnosticCodeLabel(diagnostic.code),
        source: diagnostic.source,
      });
    }
  }

  const errors = issues.filter((issue) => issue.severity === "error").length;
  const warnings = issues.filter((issue) => issue.severity === "warning").length;
  const dirty = workspaceHasDirtyStructuredText();
  const runtimeTarget =
    session && session.type === DEBUG_TYPE
      ? typeof session.configuration?.program === "string"
        ? session.configuration.program
        : undefined
      : undefined;

  panel.webview.postMessage({
    type: "compileResult",
    payload: {
      target: runtimeTarget ?? workspaceFolder.uri.fsPath,
      dirty,
      errors,
      warnings,
      issues,
      runtimeStatus,
      runtimeMessage:
        runtimeMessage ??
        (!hasConfiguration && runtimeStatus === "skipped"
          ? "No CONFIGURATION found. Debugging will prompt to create one."
          : undefined),
    } satisfies CompileResult,
  });

  let statusMessage = `Compile finished: ${errors} error(s), ${warnings} warning(s).`;
  if (runtimeStatus === "error" && runtimeMessage) {
    statusMessage = runtimeMessage;
  }
  if (!hasConfiguration && runtimeStatus === "skipped" && errors === 0) {
    statusMessage +=
      " No CONFIGURATION found; debugging will prompt to create one.";
    const create = await vscode.window.showInformationMessage(
      "No CONFIGURATION found. Create one now?",
      "Create",
      "Not now"
    );
    if (create === "Create") {
      await vscode.commands.executeCommand(
        "trust-lsp.debug.ensureConfiguration"
      );
    }
  }
  panel.webview.postMessage({
    type: "status",
    payload: statusMessage,
  });

}

function workspaceHasDirtyStructuredText(): boolean {
  return vscode.workspace.textDocuments.some(
    (doc) => doc.languageId === "structured-text" && doc.isDirty
  );
}
