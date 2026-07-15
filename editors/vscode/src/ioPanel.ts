import * as net from "net";
import * as vscode from "vscode";

import { affectsTrustConfiguration, getTrustConfiguration } from "./configuration";
import { liveValueActionTarget } from "./liveValueActionTarget";

import {
  summarizeAdsStatus,
  type AdsStatusReport,
  type AdsStatusSummary,
} from "./adsStatusSummary";
import { sendRuntimeControlRequest } from "./runtimeControlClient";
import {
  normalizeIoState,
  runtimeLifecycleService,
  type RuntimeLifecycleResult,
} from "./runtimeLifecycle";

const DEBUG_TYPE = "structured-text";

type IoEntry = {
  name?: string;
  address: string;
  source?: string;
  value: string;
  forced?: boolean;
  writable?: boolean;
};

type IoState = {
  scan?: number;
  inputs: IoEntry[];
  outputs: IoEntry[];
  memory: IoEntry[];
  ads: IoEntry[];
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

const PRAGMA_SCAN_LINES = 20;
const ENDPOINT_PROBE_TTL_MS = 2000;
const ENDPOINT_PROBE_TIMEOUT_MS = 400;

type ParsedEndpoint =
  | { kind: "tcp"; host: string; port: number }
  | { kind: "unix"; path: string };

let endpointProbeCache:
  | { endpoint: string; reachable: boolean; checkedAt: number }
  | undefined;

const structuredTextSessions = new Map<string, vscode.DebugSession>();

function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

function trackStructuredTextSession(session: vscode.DebugSession): void {
  structuredTextSessions.set(structuredTextSessionKey(session), session);
}

function untrackStructuredTextSession(session: vscode.DebugSession): void {
  structuredTextSessions.delete(structuredTextSessionKey(session));
}

function getStructuredTextSession(): vscode.DebugSession | undefined {
  return runtimeLifecycleService.getStructuredTextSession();
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
    runtimeLifecycleService.onDidChange(() => {
      if (!panel) {
        return;
      }
      void sendRuntimeStatus();
    })
  );

  const activeSession = vscode.debug.activeDebugSession;
  if (activeSession && activeSession.type === DEBUG_TYPE) {
    trackStructuredTextSession(activeSession);
  }

  context.subscriptions.push(
    vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
      if (event.event !== "stIoState") {
        return;
      }
      if (event.session.type !== DEBUG_TYPE) {
        return;
      }
      if (!panel) {
        return;
      }
      if (event.event === "stIoState") {
        const body = event.body as IoState | undefined;
        panel.webview.postMessage({
          type: "ioState",
          payload: normalizeIoState(body),
        });
      }
    })
  );

  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      if (session.type !== DEBUG_TYPE) {
        return;
      }
      trackStructuredTextSession(session);
      void requestIoState();
      void sendRuntimeStatus();
    })
  );

  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      if (session.type !== DEBUG_TYPE) {
        return;
      }
      untrackStructuredTextSession(session);
      postUnavailableLiveValues(terminatedSessionStatus(session));
    })
  );


  context.subscriptions.push(
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      if (!session || session.type !== DEBUG_TYPE) {
        postUnavailableLiveValues();
        return;
      }
      if (panel) {
        void requestIoState();
      }
      trackStructuredTextSession(session);
      void sendRuntimeStatus();
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
    void requestIoState();
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

  panel.webview.html = getHtml(panel.webview, context.extensionUri);
  panel.onDidDispose(() => {
    panel = undefined;
  });

  panel.webview.onDidReceiveMessage(handleWebviewMessage);

  void requestIoState();
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
    return "Start the runtime to see live values.";
  }
  if (isIoStateTransportFailureMessage(message)) {
    return "Live Values lost connection to the runtime. Restart or reconnect the runtime, then retry.";
  }
  return message;
}

function liveValuesUnavailableMessage(
  status: RuntimeStatusPayload | undefined
): string {
  if (status?.runtimeMode === "online" && status.runtimeState !== "connected") {
    return "Connect to the selected runtime to see live values.";
  }
  return "Start the runtime to see live values.";
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
    payload: { inputs: [], outputs: [], memory: [], ads: [] },
  });
}

function postUnavailableLiveValues(
  status?: RuntimeStatusPayload,
  message?: string
): void {
  const publish = () => {
    const statusMessage = message || liveValuesUnavailableMessage(status);
    if (status) {
      panel?.webview.postMessage({
        type: "runtimeStatus",
        payload: status,
      });
    }
    postEmptyIoState();
    panel?.webview.postMessage({
      type: "status",
      payload: statusMessage,
    });
  };
  publish();
  setTimeout(publish, 100);
  setTimeout(publish, 500);
}

function terminatedSessionStatus(session: vscode.DebugSession): RuntimeStatusPayload {
  const request = session.configuration?.request;
  const isAttach = request === "attach";
  const endpoint =
    typeof session.configuration?.endpoint === "string"
      ? session.configuration.endpoint.trim()
      : typeof session.configuration?.controlEndpoint === "string"
        ? session.configuration.controlEndpoint.trim()
        : "";
  const targetLabel =
    typeof session.configuration?.targetLabel === "string" &&
    session.configuration.targetLabel.trim()
      ? session.configuration.targetLabel.trim()
      : undefined;
  return {
    running: false,
    inlineValuesEnabled: true,
    runtimeMode: isAttach ? "online" : "simulate",
    runtimeState: "stopped",
    targetLabel,
    endpoint,
    endpointConfigured: endpoint.length > 0,
    endpointEnabled: true,
    endpointReachable: false,
    access: {
      allowWrite: false,
      allowForce: false,
      allowRelease: false,
    },
  };
}

function handleWebviewMessage(message: any): void {
  const type = typeof message?.type === "string" ? message.type : "";
  switch (type) {
    case "refresh":
      void requestIoState();
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
    case "startDebug":
      void startDebugging();
      break;
    case "compile":
      void compileActiveProgram();
      break;
    case "compileAndStart":
      void compileActiveProgram({ startDebugAfter: true });
      break;
    case "stopDebug":
      void stopDebugging();
      break;
    case "runtimeStart":
      void handleRuntimePrimary();
      break;
    case "runtimeSetMode":
      void setRuntimeMode(message.mode);
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
  runtimeControlAuthToken?: string;
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
    runtimeControlAuthToken: config.get<string>("runtime.controlAuthToken") ?? "",
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
    {
      key: "runtime.controlAuthToken",
      value: payload.runtimeControlAuthToken?.trim() || undefined,
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
  if (!panel) {
    return;
  }
  const payload = await runtimeStatusPayload();
  panel.webview.postMessage({
    type: "runtimeStatus",
    payload,
  });
}

async function requestIoState(): Promise<void> {
  const result = await runtimeLifecycleService.requestIoState();
  await handleIoStateRequestResult(result);
}

async function requestIoStateAfterScan(previousScan: number | undefined): Promise<void> {
  const result = await runtimeLifecycleService.requestIoStateAfterScan(previousScan);
  await handleIoStateRequestResult(result);
}

async function currentIoScan(): Promise<number | undefined> {
  return (await runtimeLifecycleService.snapshot()).ioState.scan;
}

async function handleIoStateRequestResult(result: RuntimeLifecycleResult): Promise<void> {
  if (!result.ok) {
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
    await executeLiveValueAction("write", address, value);
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
    await executeLiveValueAction("force", address, value);
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
    await executeLiveValueAction("release", address);
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
      await executeLiveValueAction("release", address);
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

async function executeLiveValueAction(
  action: "write" | "force" | "release",
  address: string,
  value?: string,
): Promise<void> {
  const target = liveValueActionTarget(address);
  const commands = {
    write: {
      global: "trust-lsp.debug.expr.write",
      io: "trust-lsp.debug.io.write",
    },
    force: {
      global: "trust-lsp.debug.expr.force",
      io: "trust-lsp.debug.io.force",
    },
    release: {
      global: "trust-lsp.debug.expr.release",
      io: "trust-lsp.debug.io.release",
    },
  } as const;
  if (target.kind === "global") {
    await vscode.commands.executeCommand(commands[action].global, {
      expression: target.name,
      value,
    });
    return;
  }
  await vscode.commands.executeCommand(commands[action].io, {
    address: target.address,
    value,
  });
}


async function stopDebugging(): Promise<void> {
  try {
    const stopped = await vscode.commands.executeCommand<boolean>(
      "trust-lsp.debug.stop"
    );
    if (!stopped) {
      panel?.webview.postMessage({
        type: "status",
        payload: "Start the runtime to see live values.",
      });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: userFacingIoStatus(`Stop debugging failed: ${message}`),
    });
  }
}

async function startDebugging(programOverride?: string): Promise<void> {
  try {
    const started = await vscode.commands.executeCommand<boolean>(
      "trust-lsp.debug.start",
      programOverride
    );
    if (!started) {
      panel?.webview.postMessage({
        type: "status",
        payload: "Start debugging did not launch a session.",
      });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `Start debugging failed: ${message}`,
    });
  }
}

async function startAttachDebugging(
  endpoint: string,
  authToken?: string
): Promise<boolean> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  const runtimeOptions = runtimeSourceOptions();
  const config: vscode.DebugConfiguration = {
    type: DEBUG_TYPE,
    request: "attach",
    name: "Attach Structured Text",
    endpoint,
    authToken,
    internalConsoleOptions: "neverOpen",
    ...runtimeOptions,
  };
  if (folder) {
    config.cwd = folder.uri.fsPath;
  }
  try {
    const started = await vscode.debug.startDebugging(folder, config);
    if (!started) {
      panel?.webview.postMessage({
        type: "status",
        payload: "Attach failed to start.",
      });
    }
    return started;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `Attach failed: ${message}`,
    });
    return false;
  }
}


async function setRuntimeMode(mode: unknown): Promise<void> {
  await runtimeLifecycleService.setRuntimeMode(mode);
  void sendRuntimeStatus();
}



async function handleRuntimePrimary(): Promise<void> {
  const status = await runtimeStatusPayload();
  if (status.running || status.runtimeState === "connected") {
    await handleRuntimeStop();
    return;
  }
  await handleRuntimeStart();
}

async function handleRuntimeStart(): Promise<void> {
  const result = await runtimeLifecycleService.startRuntime();
  postRuntimeLifecycleResult(result);
  void sendRuntimeStatus();
}

async function handleRuntimeStop(): Promise<void> {
  const result = await runtimeLifecycleService.stopRuntime();
  postRuntimeLifecycleResult(result);
  void sendRuntimeStatus();
}

function postRuntimeLifecycleResult(result: RuntimeLifecycleResult): void {
  panel?.webview.postMessage({
    type: "status",
    payload: result.ok ? result.message : result.failure.message,
  });
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

type CompileOptions = {
  startDebugAfter?: boolean;
};

async function compileActiveProgram(options: CompileOptions = {}): Promise<void> {
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
  if (options.startDebugAfter) {
    if (errors > 0) {
      statusMessage = `Compile blocked: ${errors} error(s). Fix errors before starting.`;
    } else if (dirty) {
      statusMessage = "Save all Structured Text files before starting the runtime.";
    } else {
      // No authoritative whole-project compile yet (phase 8) — diagnostics + reload only.
      statusMessage = "No known errors. Starting debug session...";
    }
  } else if (!hasConfiguration && runtimeStatus === "skipped" && errors === 0) {
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

  if (options.startDebugAfter && errors === 0 && !dirty) {
    await startDebugging();
  }
}

function workspaceHasDirtyStructuredText(): boolean {
  return vscode.workspace.textDocuments.some(
    (doc) => doc.languageId === "structured-text" && doc.isDirty
  );
}

function getHtml(webview: vscode.Webview, extensionUri: vscode.Uri): string {
  const nonce = getNonce();
  const codiconUri = webview.asWebviewUri(
    vscode.Uri.joinPath(
      extensionUri,
      "node_modules",
      "@vscode",
      "codicons",
      "dist",
      "codicon.css"
    )
  );
  const scriptUri = webview.asWebviewUri(
    vscode.Uri.joinPath(extensionUri, "media", "ioPanel.js")
  );
  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${
      webview.cspSource
    } 'unsafe-inline'; font-src ${webview.cspSource}; script-src ${
      webview.cspSource
    } 'nonce-${nonce}';" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Live Values</title>
    <link href="${codiconUri}" rel="stylesheet" />
    <style>
      :root {
        color-scheme: light dark;
        /* Keep Live Values on the same product token layer as Devices & Connections and
           the visual editors. Every role ends in a hard fallback so missing VS Code theme
           variables cannot collapse to browser-default black text. */
        --trust-canvas: var(--vscode-editor-background, #0f1116);
        --trust-surface: var(--vscode-editorWidget-background, #1b1f28);
        --trust-surface-raised: var(--vscode-editorHoverWidget-background, #222732);
        --trust-text: var(--vscode-foreground, #cfd6e0);
        --trust-text-muted: var(--vscode-descriptionForeground, #949cab);
        --trust-text-subtle: var(--vscode-disabledForeground, #6b7480);
        --trust-on-accent: var(--vscode-button-foreground, #ffffff);
        --trust-mono: var(--vscode-editor-font-family, ui-monospace, SFMono-Regular, Menlo, monospace);
        --trust-border: var(--vscode-editorWidget-border, var(--vscode-panel-border, #2a2f3a));
        --trust-accent: var(--vscode-focusBorder, #4a9eff);
        --trust-ok: var(--vscode-charts-green, var(--vscode-testing-iconPassed, #46c265));
        --trust-warn: var(--vscode-charts-yellow, var(--vscode-editorWarning-foreground, #e0b341));
        --trust-danger: var(--vscode-charts-red, var(--vscode-errorForeground, #f0584f));
        --trust-input-bg: var(--vscode-input-background, #10141b);
        --trust-input-border: var(--vscode-input-border, var(--vscode-editorWidget-border, #343b47));
        --trust-selected-bg: color-mix(in srgb, var(--trust-accent) 18%, transparent);
        --trust-selected-strong-bg: color-mix(in srgb, var(--trust-accent) 28%, transparent);
        --trust-radius-sm: 4px;
        --trust-radius: 6px;
        --trust-radius-lg: 8px;
        --trust-pill: 999px;
      }

      * {
        box-sizing: border-box;
      }

      body {
        font-family: var(--vscode-font-family);
        font-size: var(--vscode-font-size);
        margin: 0;
        padding: 0;
        color: var(--trust-text);
        background: var(--trust-canvas);
      }

      header {
        position: sticky;
        top: 0;
        z-index: 10;
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 8px;
        background: var(--trust-canvas);
        border-bottom: 1px solid var(--trust-border);
      }

      h1 {
        margin: 0;
        font-size: 13px;
        font-weight: 600;
      }

      .header-top {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
      }

      .header-search {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .runtime-status {
        display: flex;
        align-items: center;
        gap: 12px;
        font-size: 12px;
        color: var(--trust-text-muted);
        flex-wrap: wrap;
      }

      .target-strip {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        min-height: 22px;
        color: var(--trust-text-muted);
        font-size: 11px;
      }

      .target-label {
        color: var(--trust-text);
        font-weight: 600;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .scan-label {
        color: var(--trust-text-muted);
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
      }

      .mode-toggle {
        display: inline-flex;
        align-items: center;
        border: 1px solid var(--trust-border);
        border-radius: 999px;
        overflow: hidden;
      }

      .mode-button {
        background: transparent;
        border: none;
        color: var(--trust-text);
        padding: 4px 10px;
        font-size: 11px;
        font-weight: 600;
        cursor: pointer;
      }

      .mode-button.active {
        background: var(--trust-accent);
        color: var(--trust-on-accent);
      }

      .mode-button:disabled {
        cursor: default;
        opacity: 0.5;
      }

      .mode-subtitle {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-right: 8px;
      }

      .status-group {
        display: flex;
        align-items: center;
        gap: 6px;
      }

      .status-pill {
        padding: 2px 8px;
        border-radius: 999px;
        border: 1px solid var(--trust-border);
        background: var(--trust-surface);
        color: var(--trust-text);
        white-space: nowrap;
      }

      .status-pill.on,
      .status-pill.running {
        background: var(--trust-accent);
        color: var(--trust-on-accent);
        border-color: transparent;
      }

      .status-pill.off {
        opacity: 0.7;
      }

      .status-pill.connected {
        border-color: var(--trust-accent);
      }

      .status-pill.disconnected {
        opacity: 0.7;
      }

      .status-action {
        border: 1px solid var(--trust-border);
        background: transparent;
        color: var(--trust-text);
        padding: 2px 8px;
        border-radius: 999px;
        font-size: 11px;
      }

      .status-action:hover {
        background: var(--trust-surface);
      }

      .status-action:disabled {
        cursor: default;
        opacity: 0.5;
      }

      input#filter {
        flex: 1 1 auto;
        min-width: 0;
        padding: 4px 8px;
        border: 1px solid var(--trust-input-border);
        border-radius: 4px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
      }

      /* Focus uses the panel accent (blue), not the browser default (amber = reads as a warning). */
      input#filter:focus {
        outline: none;
        border-color: var(--trust-accent);
        box-shadow: 0 0 0 1px var(--trust-accent);
      }

      input#filter::placeholder {
        color: var(--vscode-input-placeholderForeground, var(--trust-text-muted));
      }

      .numeric-format {
        display: inline-flex;
        align-items: center;
        gap: 3px;
        flex: 0 0 auto;
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        padding: 2px;
        background: var(--trust-surface);
      }

      .numeric-format-label {
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        padding: 0 4px;
        text-transform: uppercase;
      }

      .format-toggle {
        min-width: 34px;
        height: 22px;
        padding: 0 6px;
        border: 1px solid transparent;
        border-radius: 4px;
        background: transparent;
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        line-height: 1;
      }

      .format-toggle:hover {
        background: var(--trust-selected-bg);
        color: var(--trust-text);
      }

      .format-toggle.active {
        background: var(--trust-selected-bg);
        border-color: var(--trust-input-border);
        color: var(--trust-text);
      }

      .forced-filter {
        height: 24px;
        flex: 0 0 auto;
        padding: 0 8px;
        border-radius: 999px;
        border: 1px solid var(--trust-input-border);
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
        color: var(--vscode-button-secondaryForeground, var(--trust-text));
        font-size: 11px;
        font-weight: 700;
        line-height: 1;
        white-space: nowrap;
      }

      .forced-filter:hover {
        background: var(--vscode-button-secondaryHoverBackground, var(--trust-selected-bg));
      }

      .forced-filter.active {
        border-color: var(--trust-warn);
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }

      button {
        background: var(--trust-accent);
        border: none;
        color: var(--trust-on-accent);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-weight: 600;
      }

      button:hover {
        background: var(--trust-selected-strong-bg);
      }

      button:disabled {
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
        border: 1px solid var(--trust-border);
        color: var(--trust-text-subtle);
        cursor: not-allowed;
        opacity: 1;
      }

      button:disabled:hover {
        background: var(--vscode-button-secondaryBackground, var(--trust-surface));
      }

      .panel {
        background: transparent;
        border: none;
        border-radius: 0;
        padding: 8px;
      }

      .toolbar {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .icon-btn {
        width: 28px;
        height: 28px;
        padding: 0;
        border-radius: 6px;
        border: 1px solid var(--trust-border);
        background: transparent;
        color: var(--trust-text);
        display: inline-flex;
        align-items: center;
        justify-content: center;
      }

      .icon-btn .codicon {
        font-size: 16px;
        line-height: 1;
      }

      .icon-btn:hover {
        background: var(--trust-selected-bg);
      }

      .icon-btn:active {
        background: var(--trust-surface);
      }

      .icon-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .icon-btn:disabled:hover {
        background: transparent;
      }

      .icon-btn.primary {
        border-color: transparent;
        background: var(--trust-accent);
        color: var(--trust-on-accent);
      }

      .icon-btn.primary:hover {
        background: var(--trust-selected-strong-bg);
      }

      .tree {
        display: flex;
        flex-direction: column;
        gap: 4px;
      }

      details.tree-node > summary {
        list-style: none;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 12px;
        font-weight: 600;
        color: var(--trust-text);
      }

      details.tree-node > summary:hover {
        background: var(--trust-selected-bg);
      }

      details.tree-node > summary::-webkit-details-marker {
        display: none;
      }

      details.tree-node > summary::before {
        content: "▸";
        display: inline-block;
        width: 12px;
        color: var(--trust-text-muted);
        transform: translateY(-1px);
      }

      details.tree-node[open] > summary::before {
        content: "▾";
      }

      .tree-node.level-1 {
        padding-left: 12px;
      }

      .tree-node.level-2 {
        padding-left: 22px;
      }

      .tree-node.level-3 {
        padding-left: 32px;
      }

      .rows.aligned-root-rows {
        margin-left: 22px;
      }

      .write-hint {
        margin: 2px 4px 6px 10px;
        color: var(--trust-text-muted);
        font-size: 11px;
        line-height: 1.35;
      }

      .force-policy {
        margin: 4px 12px 8px;
        padding: 5px 8px;
        border: 1px solid var(--trust-border-subtle);
        border-left: 3px solid var(--trust-warn);
        border-radius: 4px;
        background: color-mix(in srgb, var(--trust-warn) 8%, var(--trust-surface));
        color: var(--trust-text);
        font-size: 11px;
        line-height: 1.35;
      }

      .force-policy.armed-target {
        background: color-mix(in srgb, var(--trust-warn) 12%, var(--trust-surface));
      }

      /* One shared grid for the whole section so every row — BOOL or numeric, with or
         without a write-box — lines its VALUE/TYPE/STATE/ACTIONS up under the same headers.
         Rows use subgrid so the column tracks are shared, not re-derived per row. */
		      .rows {
		        display: grid;
		        grid-template-columns:
		          minmax(116px, 1fr)
		          minmax(52px, max-content)
		          minmax(38px, max-content)
		          minmax(64px, max-content)
          minmax(160px, max-content);
        column-gap: 6px;
        row-gap: 2px;
        padding: 2px 4px 2px 10px;
        overflow-x: auto;
      }

      .row,
      .row-header {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: subgrid;
        align-items: center;
        column-gap: 6px;
      }

      .row > *,
      .row-header > * {
        min-width: 0;
      }

      .row {
        padding: 2px 4px;
        border-radius: 4px;
        font-size: 12px;
      }

      .row-header {
        padding: 2px 4px;
        color: var(--trust-text-muted);
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.04em;
      }

      .row-header .actions-heading {
        text-align: right;
      }

      .row:hover,
      .row.pointer-hover {
        background: var(--trust-selected-bg);
      }

      /* A forced value is ALWAYS visibly marked (§0.5.5/§0.5.16): subtle amber wash + an amber
         left accent bar so overridden rows are unmistakable without shifting the columns. */
      .row.forced {
        background: color-mix(in srgb, var(--trust-warn) 13%, transparent);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }
		      .state-cell,
		      .type-cell {
		        color: var(--trust-text-muted);
		        font-size: 11px;
		        white-space: nowrap;
		      }

          .source-subtitle {
            color: var(--trust-text-muted);
            font-size: 10px;
            line-height: 1.2;
            overflow-wrap: anywhere;
            white-space: normal;
          }

      .state-badge {
        display: inline-block;
        min-width: 64px;
        box-sizing: border-box;
        text-align: center;
        padding: 1px 6px;
        border-radius: 6px;
        border: 1px solid var(--trust-border);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.04em;
        line-height: 1.4;
      }

      .state-badge.live {
        color: var(--trust-text-muted);
        text-transform: uppercase;
      }

      /* A forced value is an operator OVERRIDE, not a healthy state (ISA-101 / TwinCAT / CODESYS
         convention): mark it amber (caution), never green. */
      .state-badge.forced {
        color: #161616;
        background: var(--trust-warn);
        border-color: var(--trust-warn);
      }
      /* Release clears an override → a restorative secondary action. Same ghost treatment per-row
         and for "Release all" so the two read as one control, distinct from the primary buttons. */
      .release-all,
      .mini-btn.release {
        background: transparent;
        color: var(--trust-text-muted);
        border: 1px solid var(--trust-input-border);
      }
      .release-all:hover,
      .mini-btn.release:hover {
        background: var(--trust-selected-bg);
        color: var(--trust-text);
        border-color: var(--trust-text-subtle);
      }

      .row .name {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        overflow: hidden;
      }

      .row .name > div {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .row .name .type {
        font-size: 10px;
        color: var(--trust-text-muted);
      }

      .row .name .address {
        font-size: 10px;
        color: var(--trust-text-muted);
      }

      .row .value {
        color: var(--trust-text);
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .row .actions {
        display: flex;
        align-items: center;
	        gap: 4px;
        justify-content: flex-end;
        flex-wrap: nowrap;
      }

      .value-input {
	        width: 46px;
        height: 24px;
        padding: 2px 4px;
        border: 1px solid var(--trust-input-border);
        border-radius: 3px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
      }

      .value-input:disabled {
        opacity: 0.55;
        cursor: not-allowed;
      }

      .value-input.bool-toggle {
        cursor: pointer;
        font-weight: 700;
        text-align: center;
      }

      .value-input.bool-toggle[aria-pressed="true"] {
        border-color: var(--trust-accent);
        background: var(--trust-selected-bg);
        color: var(--trust-text);
      }

      .mini-btn {
	        min-width: 42px;
        height: 24px;
	        padding: 0 4px;
        border-radius: 3px;
        font-size: 11px;
        font-weight: 600;
        border: 1px solid var(--trust-input-border);
        background: var(--vscode-button-secondaryBackground, var(--trust-surface-raised));
        color: var(--vscode-button-secondaryForeground, var(--trust-text));
        display: inline-flex;
        align-items: center;
        justify-content: center;
        line-height: 1;
        white-space: nowrap;
        cursor: pointer;
      }

      /* The force/release control keeps a fixed width so its label can change between
         "Force", "Arm force" and "Release" without resizing — and so every section's
         actions column stays the same width, keeping the tables aligned across sections. */
      .mini-btn.force-slot {
	        width: 62px;
      }

      .mini-btn:hover {
        background: var(--vscode-button-secondaryHoverBackground, var(--trust-selected-bg));
      }

      .mini-btn.active {
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        border-color: var(--trust-warn);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }

      .mini-btn.armed {
        background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface));
        color: var(--trust-text);
        border-color: var(--trust-warn);
        box-shadow: inset 2px 0 0 var(--trust-warn);
      }

      .mini-btn:disabled {
        background: var(--trust-input-bg);
        border-color: var(--trust-input-border);
        color: var(--trust-text-subtle);
        box-shadow: none;
        opacity: 1;
        cursor: not-allowed;
      }

      .mini-btn:disabled:hover {
        background: var(--trust-input-bg);
      }

      .empty {
        grid-column: 1 / -1;
        font-size: 11px;
        color: var(--trust-text-muted);
        padding: 2px 6px 2px 24px;
      }

      .status {
        display: block;
        box-sizing: border-box;
        height: 27px;
        overflow: hidden;
        visibility: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        color: var(--trust-text);
        font-size: 12px;
        line-height: 1.35;
        padding: 4px 8px;
        border: 1px solid var(--trust-border);
        border-radius: 4px;
        background: var(--trust-surface);
      }

      .status:not(:empty) {
        visibility: visible;
      }

      .status.status-ok {
        border-color: var(--trust-ok);
        background: color-mix(in srgb, var(--trust-ok) 12%, var(--trust-surface));
      }

      .status.status-warn {
        border-color: var(--trust-warn);
        background: color-mix(in srgb, var(--trust-warn) 12%, var(--trust-surface));
      }

      .status.status-error {
        border-color: var(--trust-danger);
        background: color-mix(in srgb, var(--trust-danger) 12%, var(--trust-surface));
      }

      .diagnostics {
        margin-top: 12px;
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        background: var(--trust-surface);
        padding: 8px;
      }

      .diagnostics-header {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: 8px;
        margin-bottom: 6px;
      }

      .diagnostics-title {
        font-size: 12px;
        font-weight: 600;
      }

      .diagnostics-summary {
        font-size: 11px;
        color: var(--trust-text-muted);
      }

      .diagnostics-runtime {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-bottom: 6px;
      }

      .diagnostics-list {
        display: flex;
        flex-direction: column;
        gap: 6px;
      }

      .diagnostic-item {
        padding: 6px 8px;
        border-radius: 4px;
        background: var(--trust-surface);
        border-left: 3px solid transparent;
      }

      .diagnostic-item.error {
        border-left-color: var(--trust-danger);
      }

      .diagnostic-item.warning {
        border-left-color: var(--trust-warn);
      }

      .diagnostic-message {
        font-size: 12px;
      }

      .diagnostic-meta {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 2px;
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      .runtime-view.hidden {
        display: none;
      }

      .settings-panel {
        display: none;
        border: 1px solid var(--trust-border);
        border-radius: 8px;
        background: var(--trust-surface);
        padding: 12px;
      }

      .settings-panel.open {
        display: block;
      }

      .settings-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 12px;
        margin-bottom: 12px;
      }

      .settings-title {
        font-size: 13px;
        font-weight: 600;
      }

      .settings-subtitle {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 2px;
      }

      .settings-grid {
        display: grid;
        gap: 12px;
      }

      .settings-section {
        border: 1px solid var(--trust-border);
        border-radius: 6px;
        padding: 10px;
        background: var(--trust-surface);
      }

      .settings-section h2 {
        margin: 0 0 8px;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.4px;
        color: var(--trust-text-muted);
      }

      .settings-row {
        display: grid;
        grid-template-columns: 160px 1fr;
        gap: 8px;
        align-items: center;
        margin-bottom: 8px;
      }

      .settings-row:last-child {
        margin-bottom: 0;
      }

      .settings-row label {
        font-size: 11px;
        color: var(--trust-text-muted);
      }

      .settings-row input,
      .settings-row textarea,
      .settings-row select {
        width: 100%;
        padding: 4px 6px;
        border: 1px solid var(--trust-input-border);
        border-radius: 4px;
        background: var(--trust-input-bg);
        color: var(--vscode-input-foreground, var(--trust-text));
        font-family: var(--vscode-editor-font-family);
        font-size: 12px;
      }

      .settings-row textarea {
        min-height: 56px;
        resize: vertical;
      }

      .settings-help {
        font-size: 11px;
        color: var(--trust-text-muted);
        margin-top: 4px;
      }

      .settings-actions {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .button-ghost {
        background: transparent;
        border: 1px solid var(--trust-border);
        color: var(--trust-text);
      }

      .button-ghost:hover {
        background: var(--trust-selected-bg);
      }
    </style>
  </head>
  <body>
    <header>
      <div class="header-top">
        <div class="toolbar">
          <button id="releaseAllForces" type="button" class="release-all" style="display:none" title="Release every forced value on this target" aria-label="Release all forces">Release all forces</button>
          <button
            id="settings"
            class="icon-btn"
            title="Open runtime settings"
            aria-label="Open runtime settings"
            type="button"
          >
            <span class="codicon codicon-settings-gear" aria-hidden="true"></span>
          </button>
        </div>
        <div class="runtime-status">
          <span id="runtimeStatusText" class="status-pill disconnected">Stopped</span>
        </div>
      </div>
      <div class="target-strip" aria-label="Active Live Values target">
        <span>Target</span>
        <span id="targetLabel" class="target-label" title="Simulator">Simulator</span>
        <span id="scanLabel" class="scan-label" title="No runtime scan has been received yet">scan --</span>
      </div>
      <div
        id="forcePolicy"
        class="force-policy"
        aria-live="polite"
      >Force policy: simulator pins immediately; managed/remote targets require Arm force first.</div>
      <div class="header-search">
        <input id="filter" placeholder="Filter by name or address" />
        <button id="forcedFilter" class="forced-filter" type="button" style="display:none" aria-pressed="false" title="No forced values">Forced</button>
        <div class="numeric-format" aria-label="Numeric display format">
          <span class="numeric-format-label">Format</span>
          <button class="format-toggle active" type="button" data-numeric-format="dec" aria-pressed="true" title="Show numeric values as decimal">DEC</button>
          <button class="format-toggle" type="button" data-numeric-format="hex" aria-pressed="false" title="Show BYTE/WORD/DWORD values as IEC hex literals">HEX</button>
          <button class="format-toggle" type="button" data-numeric-format="bin" aria-pressed="false" title="Show BYTE/WORD/DWORD values as IEC binary literals">BIN</button>
        </div>
      </div>
      <div class="status" id="status">Live Values loading...</div>
    </header>

    <div class="panel">
      <div id="runtimeView" class="runtime-view">
        <div id="sections" class="tree"></div>
        <div class="diagnostics" id="diagnostics" style="display:none">
          <div class="diagnostics-header">
            <div class="diagnostics-title">Runtime diagnostics</div>
            <div class="diagnostics-summary" id="diagnosticsSummary"></div>
          </div>
          <div class="diagnostics-runtime" id="diagnosticsRuntime"></div>
          <div class="diagnostics-list" id="diagnosticsList"></div>
        </div>
      </div>
      <div id="settingsPanel" class="settings-panel">
        <div class="settings-header">
          <div>
            <div class="settings-title">Runtime Settings</div>
            <div class="settings-subtitle">
              Stored in workspace settings for this project.
            </div>
          </div>
          <div class="settings-actions">
            <button id="settingsSave" title="Save runtime settings" aria-label="Save runtime settings">Save</button>
            <button id="settingsCancel" class="button-ghost" title="Close without saving" aria-label="Close without saving">Close</button>
          </div>
        </div>
        <div class="settings-grid">
          <section class="settings-section">
            <h2>Runtime Control</h2>
            <div class="settings-row">
              <label for="runtimeControlEndpoint">Endpoint</label>
              <input
                id="runtimeControlEndpoint"
                type="text"
                placeholder="unix:///tmp/trust-debug.sock or tcp://127.0.0.1:9901"
                autocomplete="off"
              />
            </div>
            <div class="settings-row">
              <label for="runtimeControlAuthToken">Auth token</label>
              <input
                id="runtimeControlAuthToken"
                type="password"
                placeholder="Optional"
                autocomplete="off"
              />
            </div>
            <div class="settings-row">
              <label for="runtimeInlineValuesEnabled">Inline values</label>
              <input
                id="runtimeInlineValuesEnabled"
                type="checkbox"
              />
            </div>
            <div class="settings-help">
              Inline values show live runtime values in the editor.
            </div>
          </section>
          <section class="settings-section">
            <h2>Runtime Sources</h2>
            <div class="settings-row">
              <label for="runtimeIncludeGlobs">Include globs</label>
              <textarea
                id="runtimeIncludeGlobs"
                placeholder="**/*.{st,ST,pou,POU}"
              ></textarea>
            </div>
            <div class="settings-row">
              <label for="runtimeExcludeGlobs">Exclude globs</label>
              <textarea id="runtimeExcludeGlobs"></textarea>
            </div>
            <div class="settings-row">
              <label for="runtimeIgnorePragmas">Ignore pragmas</label>
              <textarea
                id="runtimeIgnorePragmas"
                placeholder="@trustlsp:runtime-ignore"
              ></textarea>
            </div>
            <div class="settings-help">
              One entry per line. Leave blank to use defaults.
            </div>
          </section>
          <section class="settings-section">
            <h2>Debug Adapter</h2>
            <div class="settings-row">
              <label for="debugAdapterPath">Adapter path</label>
              <input id="debugAdapterPath" type="text" autocomplete="off" />
            </div>
            <div class="settings-row">
              <label for="debugAdapterArgs">Adapter args</label>
              <textarea id="debugAdapterArgs"></textarea>
            </div>
            <div class="settings-row">
              <label for="debugAdapterEnv">Adapter env</label>
              <textarea
                id="debugAdapterEnv"
                placeholder="KEY=VALUE"
              ></textarea>
            </div>
            <div class="settings-help">
              Env entries can be KEY=VALUE per line or JSON.
            </div>
          </section>
          <section class="settings-section">
            <h2>Language Server</h2>
            <div class="settings-row">
              <label for="serverPath">Server path</label>
              <input id="serverPath" type="text" autocomplete="off" />
            </div>
            <div class="settings-row">
              <label for="traceServer">Trace level</label>
              <select id="traceServer">
                <option value="off">Off</option>
                <option value="messages">Messages</option>
                <option value="verbose">Verbose</option>
              </select>
            </div>
          </section>
        </div>
      </div>
    </div>

    <script nonce="${nonce}" src="${scriptUri}"></script>
  </body>
</html>`;
}

function getNonce(): string {
  let text = "";
  const possible =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i += 1) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
