import * as vscode from "vscode";
import * as net from "net";

const DEBUG_TYPE = "structured-text";

type IoEntry = {
  name?: string;
  address: string;
  value: string;
};

type IoState = {
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
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
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
  const active = vscode.debug.activeDebugSession;
  if (active && active.type === DEBUG_TYPE) {
    return active;
  }
  for (const session of structuredTextSessions.values()) {
    return session;
  }
  return undefined;
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

let panel: vscode.WebviewPanel | undefined;

export function registerIoPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.debug.openIoPanel", () => {
      showPanel(context);
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
          payload: body ?? { inputs: [], outputs: [], memory: [] },
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
      void sendRuntimeStatus();
    })
  );


  context.subscriptions.push(
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      if (panel) {
        void requestIoState();
      }
      if (session && session.type === DEBUG_TYPE) {
        trackStructuredTextSession(session);
      }
      void sendRuntimeStatus();
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        event.affectsConfiguration("trust-lsp.runtime.controlEndpoint") ||
        event.affectsConfiguration("trust-lsp.runtime.controlEndpointEnabled") ||
        event.affectsConfiguration("trust-lsp.runtime.inlineValuesEnabled") ||
        event.affectsConfiguration("trust-lsp.runtime.mode")
      ) {
        void sendRuntimeStatus();
      }
    })
  );

}

function showPanel(context: vscode.ExtensionContext): void {
  if (panel) {
    panel.reveal();
    void requestIoState();
    void sendRuntimeStatus();
    return;
  }

  panel = vscode.window.createWebviewPanel(
    "trust-io-panel",
    "Structured Text Runtime",
    vscode.ViewColumn.Two,
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

  context.subscriptions.push(panel);
}

function postPanelStatus(message: string): void {
  panel?.webview.postMessage({
    type: "status",
    payload: message,
  });
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
      console.error("Runtime panel webview error:", detail, message.stack || "");
      postPanelStatus(`Runtime panel error: ${detail}`);
      break;
    }
    case "webviewReady":
      console.info("Runtime panel webview ready.");
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
  const config = vscode.workspace.getConfiguration("trust-lsp");
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
  const config = vscode.workspace.getConfiguration("trust-lsp");
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

function runtimeConfigTarget(): vscode.Uri | undefined {
  const activeSession = getStructuredTextSession();
  if (activeSession?.workspaceFolder) {
    return activeSession.workspaceFolder.uri;
  }
  const editor = vscode.window.activeTextEditor;
  if (editor) {
    const folder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
    if (folder) {
      return folder.uri;
    }
  }
  return vscode.workspace.workspaceFolders?.[0]?.uri;
}

function runtimeConfigScope(target: vscode.Uri | undefined): vscode.ConfigurationTarget {
  return target ? vscode.ConfigurationTarget.WorkspaceFolder : vscode.ConfigurationTarget.Workspace;
}

async function runtimeStatusPayload(): Promise<RuntimeStatusPayload> {
  const target = runtimeConfigTarget();
  const config = vscode.workspace.getConfiguration("trust-lsp", target);
  const endpoint = (config.get<string>("runtime.controlEndpoint") ?? "").trim();
  const authToken = (config.get<string>("runtime.controlAuthToken") ?? "").trim();
  const endpointConfigured = endpoint.length > 0;
  const endpointEnabled = config.get<boolean>(
    "runtime.controlEndpointEnabled",
    true
  );
  const inlineValuesEnabled = config.get<boolean>(
    "runtime.inlineValuesEnabled",
    true
  );
  const runtimeMode = config.get<"simulate" | "online">(
    "runtime.mode",
    "simulate"
  );
  const session = getStructuredTextSession();
  const running = !!session;
  let runtimeState: RuntimeStatusPayload["runtimeState"] = "stopped";
  let endpointReachable = false;

  if (running) {
    const request = session?.configuration?.request;
    runtimeState = request === "attach" ? "connected" : "running";
  }
  if (!running && runtimeMode === "online" && endpointConfigured && endpointEnabled) {
    endpointReachable = await probeEndpointReachable(endpoint);
    if (endpointReachable) {
      const state = await fetchRuntimeState(endpoint, authToken || undefined);
      if (state) {
        runtimeState = state;
      }
    }
  }

  return {
    running,
    inlineValuesEnabled,
    runtimeMode,
    runtimeState,
    endpoint,
    endpointConfigured,
    endpointEnabled,
    endpointReachable,
  };
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
  const session = getStructuredTextSession();
  if (!session) {
    panel?.webview.postMessage({
      type: "status",
      payload: "No active Structured Text debug session.",
    });
    return;
  }

  try {
    await session.customRequest("stIoState");
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O state request failed: ${message}`,
    });
  }
}

async function writeInput(address: string, value: string): Promise<void> {
  const session = getStructuredTextSession();
  if (!session) {
    panel?.webview.postMessage({
      type: "status",
      payload: "No active Structured Text debug session.",
    });
    return;
  }
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  try {
    await session.customRequest("stIoWrite", { address, value });
    panel?.webview.postMessage({
      type: "status",
      payload: `Input write queued for ${address}.`,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O write failed: ${message}`,
    });
  }
}

async function forceInput(address: string, value: string): Promise<void> {
  const session = getStructuredTextSession();
  if (!session) {
    panel?.webview.postMessage({
      type: "status",
      payload: "No active Structured Text debug session.",
    });
    return;
  }
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  try {
    await session.customRequest("setExpression", {
      expression: address,
      value: `force: ${value}`,
    });
    panel?.webview.postMessage({
      type: "status",
      payload: `Input forced at ${address}.`,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O force failed: ${message}`,
    });
  }
}

async function releaseInput(address: string): Promise<void> {
  const session = getStructuredTextSession();
  if (!session) {
    panel?.webview.postMessage({
      type: "status",
      payload: "No active Structured Text debug session.",
    });
    return;
  }
  if (!address) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Missing I/O address.",
    });
    return;
  }

  try {
    await session.customRequest("setExpression", {
      expression: address,
      value: "release",
    });
    panel?.webview.postMessage({
      type: "status",
      payload: `Input released at ${address}.`,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `I/O release failed: ${message}`,
    });
  }
}


async function stopDebugging(): Promise<void> {
  const session = getStructuredTextSession();
  if (!session) {
    panel?.webview.postMessage({
      type: "status",
      payload: "No active Structured Text debug session.",
    });
    return;
  }
  try {
    await vscode.debug.stopDebugging(session);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    panel?.webview.postMessage({
      type: "status",
      payload: `Stop debugging failed: ${message}`,
    });
  }
}

async function startDebugging(programOverride?: string): Promise<void> {
  try {
    await vscode.commands.executeCommand(
      "trust-lsp.debug.start",
      programOverride
    );
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
  const normalized = mode === "online" ? "online" : "simulate";
  const target = runtimeConfigTarget();
  const config = vscode.workspace.getConfiguration("trust-lsp", target);
  await config.update("runtime.mode", normalized, runtimeConfigScope(target));
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
  const status = await runtimeStatusPayload();
  const target = runtimeConfigTarget();
  const config = vscode.workspace.getConfiguration("trust-lsp", target);
  const mode = config.get<"simulate" | "online">(
    "runtime.mode",
    "simulate"
  );

  if (mode === "simulate") {
    await compileActiveProgram({ startDebugAfter: true });
    return;
  }

  const endpoint = status.endpoint;
  if (!status.endpointConfigured) {
    panel?.webview.postMessage({
      type: "status",
      payload: "Runtime endpoint not set.",
    });
    void sendRuntimeStatus();
    return;
  }

  if (!status.endpointEnabled) {
    await config.update(
      "runtime.controlEndpointEnabled",
      true,
      runtimeConfigScope(target)
    );
  }

  const reachable = await probeEndpointReachable(endpoint);
  if (reachable) {
    const authToken = config.get<string>("runtime.controlAuthToken") ?? "";
    await startAttachDebugging(endpoint, authToken || undefined);
    void sendRuntimeStatus();
    return;
  }


  panel?.webview.postMessage({
    type: "status",
    payload: `Runtime not reachable: ${endpoint}`,
  });
  void sendRuntimeStatus();
}

async function handleRuntimeStop(): Promise<void> {
  const activeSession = getStructuredTextSession();
  if (activeSession) {
    await stopDebugging();
    return;
  }
  const status = await runtimeStatusPayload();
  if (status.runtimeState === "connected") {
    const target = runtimeConfigTarget();
    const config = vscode.workspace.getConfiguration("trust-lsp", target);
    await config.update(
      "runtime.controlEndpointEnabled",
      false,
      runtimeConfigScope(target)
    );
    void sendRuntimeStatus();
  }
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
  const config = vscode.workspace.getConfiguration("trust-lsp");
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
        runtimeMessage = "Runtime reload succeeded.";
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
      statusMessage = "Compile ok. Starting debug session...";
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
    <title>Structured Text Runtime</title>
    <link href="${codiconUri}" rel="stylesheet" />
    <style>
      :root {
        color-scheme: light dark;
        --bg: var(--vscode-sideBar-background);
        --text: var(--vscode-sideBar-foreground);
        --muted: var(--vscode-descriptionForeground);
        --border: var(--vscode-sideBar-border, var(--vscode-panel-border));
        --panel: var(--vscode-editor-background);
        --table-header: var(--vscode-sideBarSectionHeader-background, var(--vscode-sideBar-background));
        --table-header-text: var(--vscode-sideBarSectionHeader-foreground, var(--vscode-sideBar-foreground));
        --row-hover: var(--vscode-list-hoverBackground);
        --row-alt: var(--vscode-list-inactiveSelectionBackground);
        --button-bg: var(--vscode-button-background);
        --button-fg: var(--vscode-button-foreground);
        --button-hover: var(--vscode-button-hoverBackground);
        --input-bg: var(--vscode-input-background);
        --input-fg: var(--vscode-input-foreground);
        --input-border: var(--vscode-input-border);
        --error: var(--vscode-errorForeground, #f14c4c);
        --warning: var(--vscode-editorWarning-foreground, #cca700);
      }

      * {
        box-sizing: border-box;
      }

      body {
        font-family: var(--vscode-font-family);
        font-size: var(--vscode-font-size);
        margin: 0;
        padding: 0;
        color: var(--text);
        background: var(--bg);
      }

      header {
        position: sticky;
        top: 0;
        z-index: 10;
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 8px;
        background: var(--bg);
        border-bottom: 1px solid var(--border);
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
      }

      .runtime-status {
        display: flex;
        align-items: center;
        gap: 12px;
        font-size: 12px;
        color: var(--muted);
        flex-wrap: wrap;
      }

      .mode-toggle {
        display: inline-flex;
        align-items: center;
        border: 1px solid var(--border);
        border-radius: 999px;
        overflow: hidden;
      }

      .mode-button {
        background: transparent;
        border: none;
        color: var(--text);
        padding: 4px 10px;
        font-size: 11px;
        font-weight: 600;
        cursor: pointer;
      }

      .mode-button.active {
        background: var(--button-bg);
        color: var(--button-fg);
      }

      .mode-button:disabled {
        cursor: default;
        opacity: 0.5;
      }

      .mode-subtitle {
        font-size: 11px;
        color: var(--muted);
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
        border: 1px solid var(--border);
        background: var(--row-alt);
        color: var(--text);
        white-space: nowrap;
      }

      .status-pill.on,
      .status-pill.running {
        background: var(--button-bg);
        color: var(--button-fg);
        border-color: transparent;
      }

      .status-pill.off {
        opacity: 0.7;
      }

      .status-pill.connected {
        border-color: var(--button-bg);
      }

      .status-pill.disconnected {
        opacity: 0.7;
      }

      .status-action {
        border: 1px solid var(--border);
        background: transparent;
        color: var(--text);
        padding: 2px 8px;
        border-radius: 999px;
        font-size: 11px;
      }

      .status-action:hover {
        background: var(--row-alt);
      }

      .status-action:disabled {
        cursor: default;
        opacity: 0.5;
      }

      input#filter {
        padding: 4px 8px;
        border: 1px solid var(--input-border);
        border-radius: 4px;
        min-width: 220px;
        background: var(--input-bg);
        color: var(--input-fg);
      }

      input#filter::placeholder {
        color: rgba(76, 86, 106, 0.7);
      }

      button {
        background: var(--button-bg);
        border: none;
        color: var(--button-fg);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-weight: 600;
      }

      button:hover {
        background: var(--button-hover);
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
        border: 1px solid var(--border);
        background: transparent;
        color: var(--text);
        display: inline-flex;
        align-items: center;
        justify-content: center;
      }

      .icon-btn .codicon {
        font-size: 16px;
        line-height: 1;
      }

      .icon-btn:hover {
        background: var(--row-hover);
      }

      .icon-btn:active {
        background: var(--row-alt);
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
        background: var(--button-bg);
        color: var(--button-fg);
      }

      .icon-btn.primary:hover {
        background: var(--button-hover);
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
        color: var(--text);
      }

      details.tree-node > summary:hover {
        background: var(--row-hover);
      }

      details.tree-node > summary::-webkit-details-marker {
        display: none;
      }

      details.tree-node > summary::before {
        content: "▸";
        display: inline-block;
        width: 12px;
        color: var(--muted);
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

      .rows {
        display: flex;
        flex-direction: column;
        gap: 2px;
        padding: 2px 6px 2px 18px;
      }

      .row {
        display: grid;
        grid-template-columns: minmax(120px, 1fr) auto auto;
        align-items: center;
        gap: 8px;
        padding: 2px 4px;
        border-radius: 4px;
        font-size: 12px;
      }

      .row:hover {
        background: var(--row-hover);
      }

      .row .name {
        display: flex;
        flex-direction: column;
        gap: 2px;
      }

      .row .name .type {
        font-size: 10px;
        color: var(--muted);
      }

      .row .name .address {
        font-size: 10px;
        color: var(--muted);
      }

      .row .value {
        color: var(--text);
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
      }

      .row .actions {
        display: flex;
        align-items: center;
        gap: 4px;
      }

      .value-input {
        width: 70px;
        padding: 2px 4px;
        border: 1px solid var(--input-border);
        border-radius: 3px;
        background: var(--input-bg);
        color: var(--input-fg);
        font-family: var(--vscode-editor-font-family);
        font-size: 11px;
      }

      .value-input:disabled {
        opacity: 0.55;
        cursor: not-allowed;
      }

      .mini-btn {
        width: 18px;
        height: 18px;
        padding: 0;
        border-radius: 3px;
        font-size: 11px;
        font-weight: 600;
        border: 1px solid var(--input-border);
        background: var(--button-bg);
        color: var(--button-fg);
        display: inline-flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
      }

      .mini-btn:hover {
        background: var(--button-hover);
      }

      .mini-btn:disabled {
        opacity: 0.55;
        cursor: not-allowed;
      }

      .empty {
        font-size: 11px;
        color: var(--muted);
        padding: 2px 6px 2px 24px;
      }

      .status {
        margin-top: 10px;
        color: var(--muted);
        font-size: 12px;
      }

      .diagnostics {
        margin-top: 12px;
        border: 1px solid var(--border);
        border-radius: 6px;
        background: var(--panel);
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
        color: var(--muted);
      }

      .diagnostics-runtime {
        font-size: 11px;
        color: var(--muted);
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
        background: var(--row-alt);
        border-left: 3px solid transparent;
      }

      .diagnostic-item.error {
        border-left-color: var(--error);
      }

      .diagnostic-item.warning {
        border-left-color: var(--warning);
      }

      .diagnostic-message {
        font-size: 12px;
      }

      .diagnostic-meta {
        font-size: 11px;
        color: var(--muted);
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
        border: 1px solid var(--border);
        border-radius: 8px;
        background: var(--panel);
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
        color: var(--muted);
        margin-top: 2px;
      }

      .settings-grid {
        display: grid;
        gap: 12px;
      }

      .settings-section {
        border: 1px solid var(--border);
        border-radius: 6px;
        padding: 10px;
        background: var(--row-alt);
      }

      .settings-section h2 {
        margin: 0 0 8px;
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.4px;
        color: var(--muted);
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
        color: var(--muted);
      }

      .settings-row input,
      .settings-row textarea,
      .settings-row select {
        width: 100%;
        padding: 4px 6px;
        border: 1px solid var(--input-border);
        border-radius: 4px;
        background: var(--input-bg);
        color: var(--input-fg);
        font-family: var(--vscode-editor-font-family);
        font-size: 12px;
      }

      .settings-row textarea {
        min-height: 56px;
        resize: vertical;
      }

      .settings-help {
        font-size: 11px;
        color: var(--muted);
        margin-top: 4px;
      }

      .settings-actions {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .button-ghost {
        background: transparent;
        border: 1px solid var(--border);
        color: var(--text);
      }

      .button-ghost:hover {
        background: var(--row-hover);
      }
    </style>
  </head>
  <body>
    <header>
      <div class="header-top">
        <div class="toolbar">
          <div class="mode-toggle" role="group" aria-label="Runtime mode">
            <button id="modeSimulate" class="mode-button" type="button" title="Use the local runtime started by the debugger." aria-label="Use the local runtime started by the debugger">Local</button>
            <button id="modeOnline" class="mode-button" type="button" title="Connect to a running runtime at the configured endpoint." aria-label="Connect to a running runtime at the configured endpoint">External</button>
          </div>
          <button id="runtimeStart" type="button" title="Start or stop the selected runtime." aria-label="Start or stop the selected runtime">Start</button>
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
      <div class="header-search">
        <input id="filter" placeholder="Filter by name or address" />
      </div>
    </header>

    <div class="panel">
      <div id="runtimeView" class="runtime-view">
        <div id="sections" class="tree"></div>
        <div class="diagnostics" id="diagnostics">
          <div class="diagnostics-header">
            <div class="diagnostics-title">Compile Diagnostics</div>
            <div class="diagnostics-summary" id="diagnosticsSummary">
              No compile run yet
            </div>
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
      <div class="status" id="status">Runtime panel loading...</div>
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
