import * as net from "net";
import * as path from "path";
import * as vscode from "vscode";

import { defaultRuntimeControlEndpoint } from "./runtimeDefaults";

const HMI_PANEL_VIEW_TYPE = "trust-hmi-preview";
const HMI_LAYOUT_FILE = [".vscode", "trust-hmi-layout.json"] as const;
const REQUEST_TIMEOUT_MS = 2000;
const DEFAULT_POLL_INTERVAL_MS = 500;
const SEARCH_GLOB = "**/*.{st,ST,pou,POU}";
const SEARCH_EXCLUDE = "**/{.git,node_modules,target,.vscode-test}/**";

type ParsedEndpoint =
  | { kind: "tcp"; host: string; port: number }
  | { kind: "unix"; path: string };

type ControlRequestHandler = (
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params?: unknown
) => Promise<unknown>;

type HmiWidgetLocation = {
  file: string;
  line: number;
  column: number;
};

export type HmiWidgetSchema = {
  id: string;
  path: string;
  label: string;
  data_type: string;
  access: string;
  writable: boolean;
  widget: string;
  source: string;
  page: string;
  group: string;
  order: number;
  unit?: string | null;
  min?: number | null;
  max?: number | null;
  location?: HmiWidgetLocation;
};

type HmiSchemaResult = {
  version: number;
  mode: string;
  read_only: boolean;
  resource: string;
  generated_at_ms: number;
  theme?: {
    style?: string;
    accent?: string;
    background?: string;
    surface?: string;
    text?: string;
  };
  pages: Array<{ id: string; title: string; order: number }>;
  widgets: HmiWidgetSchema[];
};

type HmiValuesResult = {
  connected: boolean;
  timestamp_ms: number;
  freshness_ms?: number | null;
  values: Record<string, { v: unknown; q: string; ts_ms: number }>;
};

type LayoutWidgetOverride = Partial<
  Pick<HmiWidgetSchema, "label" | "page" | "group" | "order" | "widget" | "unit" | "min" | "max">
>;

type LayoutOverrides = Record<string, LayoutWidgetOverride>;

type LayoutFile = {
  version: 1;
  widgets: LayoutOverrides;
  updated_at: string;
};

let panel: vscode.WebviewPanel | undefined;
let pollTimer: NodeJS.Timeout | undefined;
let requestSeq = 1;
let baseSchema: HmiSchemaResult | undefined;
let effectiveSchema: HmiSchemaResult | undefined;
let lastValues: HmiValuesResult | undefined;
let lastStatus = "";
let overrides: LayoutOverrides = {};
let controlRequest: ControlRequestHandler = sendControlRequest;

export function registerHmiPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.hmi.openPreview", async () => {
      await showPanel(context);
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (!panel || !isRelevantForSchemaRefresh(document.uri)) {
        return;
      }
      void refreshSchema();
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (!panel) {
        return;
      }
      if (
        event.affectsConfiguration("trust-lsp.runtime.controlEndpoint") ||
        event.affectsConfiguration("trust-lsp.runtime.controlAuthToken") ||
        event.affectsConfiguration("trust-lsp.runtime.controlEndpointEnabled")
      ) {
        void refreshSchema();
      }
      if (event.affectsConfiguration("trust-lsp.hmi.pollIntervalMs")) {
        startPolling();
      }
    })
  );
}

async function showPanel(context: vscode.ExtensionContext): Promise<void> {
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
    await initializePanel();
    return;
  }

  panel = vscode.window.createWebviewPanel(
    HMI_PANEL_VIEW_TYPE,
    "Structured Text: HMI Preview",
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
    }
  );
  panel.webview.html = getHtml(panel.webview);

  panel.onDidDispose(() => {
    panel = undefined;
    stopPolling();
    baseSchema = undefined;
    effectiveSchema = undefined;
    lastValues = undefined;
  });

  panel.webview.onDidReceiveMessage((message: unknown) => {
    void handleWebviewMessage(message);
  });

  context.subscriptions.push(panel);
  await initializePanel();
}

async function initializePanel(): Promise<void> {
  const folder = pickWorkspaceFolder();
  overrides = folder ? await loadLayoutOverrides(folder.uri) : {};
  await refreshSchema();
  startPolling();
}

async function handleWebviewMessage(message: unknown): Promise<void> {
  if (!isRecord(message)) {
    return;
  }
  const type = typeof message.type === "string" ? message.type : "";
  if (!type) {
    return;
  }

  switch (type) {
    case "ready": {
      if (effectiveSchema) {
        postMessage("schema", effectiveSchema);
      }
      if (lastValues) {
        postMessage("values", lastValues);
      }
      postMessage("status", lastStatus);
      break;
    }
    case "refreshSchema":
      await refreshSchema();
      break;
    case "navigateWidget":
      await handleNavigateMessage(message.payload);
      break;
    case "saveLayout":
      await handleSaveLayoutMessage(message.payload);
      break;
    default:
      break;
  }
}

async function handleNavigateMessage(payload: unknown): Promise<void> {
  if (!isRecord(payload) || typeof payload.id !== "string") {
    return;
  }
  if (!effectiveSchema) {
    return;
  }
  const widget = effectiveSchema.widgets.find((candidate) => candidate.id === payload.id);
  if (!widget) {
    return;
  }
  const location = await resolveWidgetLocation(widget);
  if (!location) {
    setStatus(`Could not resolve source for ${widget.path}`);
    return;
  }
  const editor = await vscode.window.showTextDocument(location.uri, { preview: false });
  const selection = new vscode.Selection(location.range.start, location.range.start);
  editor.selection = selection;
  editor.revealRange(
    new vscode.Range(location.range.start, location.range.start),
    vscode.TextEditorRevealType.InCenterIfOutsideViewport
  );
  setStatus(`Navigated to ${path.basename(location.uri.fsPath)}:${location.range.start.line + 1}`);
}

async function handleSaveLayoutMessage(payload: unknown): Promise<void> {
  const folder = pickWorkspaceFolder();
  if (!folder) {
    setStatus("No workspace folder is open. Cannot persist HMI layout.");
    return;
  }

  try {
    const parsed = validateLayoutSavePayload(payload);
    await saveLayoutOverrides(folder.uri, parsed);
    overrides = parsed;
    if (baseSchema) {
      effectiveSchema = applyLayoutOverrides(baseSchema, overrides);
      postMessage("schema", effectiveSchema);
    }
    setStatus(`Saved HMI layout overrides (${Object.keys(parsed).length} widgets).`);
    postMessage("layoutSaved", { ok: true });
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`Layout save rejected: ${detail}`);
    postMessage("layoutSaved", { ok: false, error: detail });
  }
}

async function refreshSchema(): Promise<void> {
  const endpointSettings = runtimeEndpointSettings();
  try {
    const raw = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.schema.get"
    );
    if (!isHmiSchemaResult(raw)) {
      throw new Error("runtime returned an invalid hmi.schema.get payload");
    }
    baseSchema = raw;
    effectiveSchema = applyLayoutOverrides(raw, overrides);
    postMessage("schema", effectiveSchema);
    setStatus(
      `Schema loaded (${effectiveSchema.widgets.length} widgets, ${effectiveSchema.pages.length} pages).`
    );
    await pollValues();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`HMI schema request failed: ${detail}`);
  }
}

async function pollValues(force = false): Promise<void> {
  if (!panel || !effectiveSchema || (!force && !panel.visible)) {
    return;
  }
  const endpointSettings = runtimeEndpointSettings();
  const ids = effectiveSchema.widgets.map((widget) => widget.id);
  if (ids.length === 0) {
    return;
  }
  try {
    const raw = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.values.get",
      { ids }
    );
    if (!isHmiValuesResult(raw)) {
      throw new Error("runtime returned an invalid hmi.values.get payload");
    }
    lastValues = raw;
    postMessage("values", raw);
    const qualitySuffix = raw.connected ? "connected" : "disconnected";
    setStatus(`Values refreshed (${qualitySuffix}).`);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`HMI values request failed: ${detail}`);
  }
}

function startPolling(): void {
  stopPolling();
  const intervalMs = runtimeEndpointSettings().pollIntervalMs;
  pollTimer = setInterval(() => {
    void pollValues();
  }, intervalMs);
}

function stopPolling(): void {
  if (!pollTimer) {
    return;
  }
  clearInterval(pollTimer);
  pollTimer = undefined;
}

function runtimeEndpointSettings(): {
  endpoint: string;
  authToken: string | undefined;
  pollIntervalMs: number;
} {
  const config = vscode.workspace.getConfiguration("trust-lsp");
  const endpointEnabled = config.get<boolean>("runtime.controlEndpointEnabled", true);
  const configured = endpointEnabled
    ? (config.get<string>("runtime.controlEndpoint") ?? "").trim()
    : "";
  const endpoint = configured || defaultRuntimeControlEndpoint();
  const auth = (config.get<string>("runtime.controlAuthToken") ?? "").trim();
  const poll = config.get<number>("hmi.pollIntervalMs", DEFAULT_POLL_INTERVAL_MS);
  const pollIntervalMs = Number.isFinite(poll) ? Math.max(100, Math.floor(poll)) : DEFAULT_POLL_INTERVAL_MS;
  return {
    endpoint,
    authToken: auth.length > 0 ? auth : undefined,
    pollIntervalMs,
  };
}

function parseControlEndpoint(endpoint: string): ParsedEndpoint | undefined {
  if (endpoint.startsWith("tcp://")) {
    try {
      const url = new URL(endpoint);
      const port = Number(url.port);
      if (!url.hostname || !Number.isFinite(port) || port <= 0) {
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
    const socketPath = endpoint.slice("unix://".length);
    if (!socketPath) {
      return undefined;
    }
    return { kind: "unix", path: socketPath };
  }
  return undefined;
}

async function sendControlRequest(
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params?: unknown
): Promise<unknown> {
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    throw new Error(`invalid control endpoint '${endpoint}'`);
  }
  const id = requestSeq++;
  const requestEnvelope = {
    id,
    type: requestType,
    params,
    auth: authToken,
  };

  return await new Promise<unknown>((resolve, reject) => {
    let settled = false;
    let buffer = "";
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });

    const finish = (fn: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      fn();
    };

    socket.setTimeout(REQUEST_TIMEOUT_MS, () => {
      finish(() => reject(new Error("control request timeout")));
    });
    socket.once("error", (error) => {
      finish(() => reject(error));
    });
    socket.once("connect", () => {
      socket.write(`${JSON.stringify(requestEnvelope)}\n`);
    });
    socket.on("data", (chunk: Buffer | string) => {
      buffer += chunk.toString();
      let newlineIndex = buffer.indexOf("\n");
      while (newlineIndex !== -1) {
        const line = buffer.slice(0, newlineIndex).trim();
        buffer = buffer.slice(newlineIndex + 1);
        if (line.length > 0) {
          try {
            const parsedLine = JSON.parse(line) as {
              ok?: boolean;
              result?: unknown;
              error?: string;
            };
            if (parsedLine.ok) {
              finish(() => resolve(parsedLine.result));
            } else {
              const errorText =
                typeof parsedLine.error === "string" && parsedLine.error.length > 0
                  ? parsedLine.error
                  : "control request rejected";
              finish(() => reject(new Error(errorText)));
            }
            return;
          } catch (error) {
            finish(() => reject(error));
            return;
          }
        }
        newlineIndex = buffer.indexOf("\n");
      }
    });
  });
}

function postMessage(type: string, payload: unknown): void {
  if (!panel) {
    return;
  }
  void panel.webview.postMessage({ type, payload });
}

function setStatus(message: string): void {
  lastStatus = message;
  postMessage("status", message);
}

function pickWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
  const activeEditorUri = vscode.window.activeTextEditor?.document.uri;
  if (activeEditorUri) {
    const byActive = vscode.workspace.getWorkspaceFolder(activeEditorUri);
    if (byActive) {
      return byActive;
    }
  }
  return vscode.workspace.workspaceFolders?.[0];
}

function isRelevantForSchemaRefresh(uri: vscode.Uri): boolean {
  const file = uri.fsPath.toLowerCase();
  if (file.endsWith(".st") || file.endsWith(".pou")) {
    return true;
  }
  return file.endsWith("hmi.toml") || file.endsWith(".hmi.toml");
}

async function layoutFileUri(workspaceUri: vscode.Uri): Promise<vscode.Uri> {
  const rootPath = path.resolve(workspaceUri.fsPath);
  const candidate = vscode.Uri.joinPath(workspaceUri, ...HMI_LAYOUT_FILE);
  const candidatePath = path.resolve(candidate.fsPath);
  const safeRootPrefix = `${rootPath}${path.sep}`;
  if (candidatePath !== rootPath && !candidatePath.startsWith(safeRootPrefix)) {
    throw new Error("unsafe layout file path");
  }
  return candidate;
}

async function loadLayoutOverrides(workspaceUri: vscode.Uri): Promise<LayoutOverrides> {
  try {
    const fileUri = await layoutFileUri(workspaceUri);
    const bytes = await vscode.workspace.fs.readFile(fileUri);
    const parsed = JSON.parse(Buffer.from(bytes).toString("utf8")) as LayoutFile;
    if (!parsed || parsed.version !== 1 || !isRecord(parsed.widgets)) {
      return {};
    }
    return normalizeLayoutOverrides(parsed.widgets);
  } catch {
    return {};
  }
}

async function saveLayoutOverrides(
  workspaceUri: vscode.Uri,
  nextOverrides: LayoutOverrides
): Promise<void> {
  const folderUri = vscode.Uri.joinPath(workspaceUri, HMI_LAYOUT_FILE[0]);
  const fileUri = await layoutFileUri(workspaceUri);
  await vscode.workspace.fs.createDirectory(folderUri);
  const payload: LayoutFile = {
    version: 1,
    widgets: nextOverrides,
    updated_at: new Date().toISOString(),
  };
  const text = `${JSON.stringify(payload, null, 2)}\n`;
  await vscode.workspace.fs.writeFile(fileUri, Buffer.from(text, "utf8"));
}

function normalizeLayoutOverrides(value: unknown): LayoutOverrides {
  if (!isRecord(value)) {
    return {};
  }
  const result: LayoutOverrides = {};
  for (const [widgetPath, rawOverride] of Object.entries(value)) {
    if (!isRecord(rawOverride)) {
      continue;
    }
    const normalized: LayoutWidgetOverride = {};
    if (typeof rawOverride.label === "string" && rawOverride.label.trim()) {
      normalized.label = rawOverride.label.trim();
    }
    if (typeof rawOverride.page === "string" && rawOverride.page.trim()) {
      normalized.page = rawOverride.page.trim();
    }
    if (typeof rawOverride.group === "string" && rawOverride.group.trim()) {
      normalized.group = rawOverride.group.trim();
    }
    if (typeof rawOverride.widget === "string" && rawOverride.widget.trim()) {
      normalized.widget = rawOverride.widget.trim();
    }
    if (typeof rawOverride.unit === "string" && rawOverride.unit.trim()) {
      normalized.unit = rawOverride.unit.trim();
    }
    if (typeof rawOverride.order === "number" && Number.isFinite(rawOverride.order)) {
      normalized.order = rawOverride.order;
    }
    if (typeof rawOverride.min === "number" && Number.isFinite(rawOverride.min)) {
      normalized.min = rawOverride.min;
    }
    if (typeof rawOverride.max === "number" && Number.isFinite(rawOverride.max)) {
      normalized.max = rawOverride.max;
    }
    if (Object.keys(normalized).length > 0) {
      result[widgetPath] = normalized;
    }
  }
  return result;
}

function validateLayoutSavePayload(payload: unknown): LayoutOverrides {
  if (!isRecord(payload) || !isRecord(payload.widgets)) {
    throw new Error("payload.widgets must be an object");
  }
  const parsed = normalizeLayoutOverrides(payload.widgets);
  for (const [widgetPath, override] of Object.entries(parsed)) {
    if (!widgetPath.trim()) {
      throw new Error("widget path must not be empty");
    }
    if (override.order !== undefined && !Number.isInteger(override.order)) {
      throw new Error(`order for '${widgetPath}' must be an integer`);
    }
    if (override.page !== undefined && !/^[A-Za-z0-9._-]+$/.test(override.page)) {
      throw new Error(`page for '${widgetPath}' contains unsupported characters`);
    }
  }
  return parsed;
}

function applyLayoutOverrides(schema: HmiSchemaResult, localOverrides: LayoutOverrides): HmiSchemaResult {
  const widgets = schema.widgets.map((widget) => {
    const override = localOverrides[widget.path];
    if (!override) {
      return { ...widget };
    }
    return {
      ...widget,
      label: override.label ?? widget.label,
      page: override.page ?? widget.page,
      group: override.group ?? widget.group,
      order: override.order ?? widget.order,
      widget: override.widget ?? widget.widget,
      unit: override.unit ?? widget.unit,
      min: override.min ?? widget.min,
      max: override.max ?? widget.max,
    };
  });

  widgets.sort((left, right) => {
    if (left.page !== right.page) {
      return left.page.localeCompare(right.page);
    }
    if (left.group !== right.group) {
      return left.group.localeCompare(right.group);
    }
    if (left.order !== right.order) {
      return left.order - right.order;
    }
    return left.label.localeCompare(right.label);
  });

  const pageOrder = new Map(schema.pages.map((page) => [page.id, page.order]));
  for (const widget of widgets) {
    if (!pageOrder.has(widget.page)) {
      pageOrder.set(widget.page, pageOrder.size * 10);
    }
  }

  const pages = Array.from(pageOrder.entries())
    .map(([id, order]) => {
      const existing = schema.pages.find((page) => page.id === id);
      return {
        id,
        order,
        title: existing?.title ?? titleCase(id),
      };
    })
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));

  return {
    ...schema,
    pages,
    widgets,
  };
}

function titleCase(value: string): string {
  return value
    .split(/[_\-.]+/)
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

async function resolveWidgetLocation(widget: HmiWidgetSchema): Promise<vscode.Location | undefined> {
  if (widget.location) {
    const resolved = await resolveLocationFromSchema(widget.location);
    if (resolved) {
      return resolved;
    }
  }

  const pathInfo = parseWidgetPath(widget.path);
  if (!pathInfo) {
    return undefined;
  }
  if (pathInfo.kind === "program") {
    return await findProgramVariable(pathInfo.program, pathInfo.variable);
  }
  return await findGlobalVariable(pathInfo.name);
}

type ParsedWidgetPath =
  | { kind: "program"; program: string; variable: string }
  | { kind: "global"; name: string };

function parseWidgetPath(widgetPath: string): ParsedWidgetPath | undefined {
  const trimmed = widgetPath.trim();
  if (!trimmed) {
    return undefined;
  }
  if (trimmed.startsWith("global.")) {
    const name = trimmed.slice("global.".length).split(".")[0];
    return name ? { kind: "global", name } : undefined;
  }
  const firstDot = trimmed.indexOf(".");
  if (firstDot <= 0 || firstDot === trimmed.length - 1) {
    return undefined;
  }
  const program = trimmed.slice(0, firstDot);
  const variable = trimmed.slice(firstDot + 1).split(".")[0];
  if (!program || !variable) {
    return undefined;
  }
  return { kind: "program", program, variable };
}

async function resolveLocationFromSchema(
  location: HmiWidgetLocation
): Promise<vscode.Location | undefined> {
  const file = location.file.trim();
  if (!file) {
    return undefined;
  }

  const candidates: vscode.Uri[] = [];
  if (path.isAbsolute(file)) {
    candidates.push(vscode.Uri.file(file));
  } else {
    for (const folder of vscode.workspace.workspaceFolders ?? []) {
      candidates.push(vscode.Uri.joinPath(folder.uri, file));
    }
  }

  for (const candidate of candidates) {
    try {
      await vscode.workspace.fs.stat(candidate);
      const position = new vscode.Position(
        Math.max(0, location.line),
        Math.max(0, location.column)
      );
      return new vscode.Location(candidate, new vscode.Range(position, position));
    } catch {
      // Ignore candidate misses.
    }
  }
  return undefined;
}

async function findProgramVariable(
  programName: string,
  variableName: string
): Promise<vscode.Location | undefined> {
  const files = await vscode.workspace.findFiles(SEARCH_GLOB, SEARCH_EXCLUDE, 2000);
  for (const uri of files) {
    const doc = await vscode.workspace.openTextDocument(uri);
    const position = findProgramVarPosition(doc.getText(), programName, variableName);
    if (position) {
      return new vscode.Location(uri, new vscode.Range(position, position));
    }
  }
  return undefined;
}

async function findGlobalVariable(name: string): Promise<vscode.Location | undefined> {
  const files = await vscode.workspace.findFiles(SEARCH_GLOB, SEARCH_EXCLUDE, 2000);
  for (const uri of files) {
    const doc = await vscode.workspace.openTextDocument(uri);
    const position = findGlobalVarPosition(doc.getText(), name);
    if (position) {
      return new vscode.Location(uri, new vscode.Range(position, position));
    }
  }
  return undefined;
}

function findProgramVarPosition(
  source: string,
  programName: string,
  variableName: string
): vscode.Position | undefined {
  const lines = source.split(/\r?\n/);
  let inProgram = false;
  let inVarBlock = false;
  const programHeader = new RegExp(`^\\s*PROGRAM\\s+${escapeRegex(programName)}\\b`, "i");
  const programEnd = /^\s*END_PROGRAM\b/i;
  const varBlockStart = /^\s*VAR(?:\b|_)/i;
  const varBlockEnd = /^\s*END_VAR\b/i;
  const declaration = new RegExp(`^\\s*${escapeRegex(variableName)}\\b\\s*:`, "i");

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!inProgram) {
      if (programHeader.test(line)) {
        inProgram = true;
      }
      continue;
    }
    if (programEnd.test(line)) {
      inProgram = false;
      inVarBlock = false;
      continue;
    }
    if (!inVarBlock && varBlockStart.test(line)) {
      inVarBlock = true;
      continue;
    }
    if (inVarBlock && varBlockEnd.test(line)) {
      inVarBlock = false;
      continue;
    }
    if (inVarBlock && declaration.test(line)) {
      const first = line.search(/\S/);
      const column = first >= 0 ? first : 0;
      return new vscode.Position(index, column);
    }
  }
  return undefined;
}

function findGlobalVarPosition(source: string, variableName: string): vscode.Position | undefined {
  const lines = source.split(/\r?\n/);
  let inGlobal = false;
  const globalStart = /^\s*VAR_GLOBAL\b/i;
  const varBlockEnd = /^\s*END_VAR\b/i;
  const declaration = new RegExp(`^\\s*${escapeRegex(variableName)}\\b\\s*:`, "i");

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!inGlobal) {
      if (globalStart.test(line)) {
        inGlobal = true;
      }
      continue;
    }
    if (varBlockEnd.test(line)) {
      inGlobal = false;
      continue;
    }
    if (declaration.test(line)) {
      const first = line.search(/\S/);
      const column = first >= 0 ? first : 0;
      return new vscode.Position(index, column);
    }
  }
  return undefined;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isHmiSchemaResult(value: unknown): value is HmiSchemaResult {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.version === "number" &&
    Array.isArray(value.pages) &&
    Array.isArray(value.widgets)
  );
}

function isHmiValuesResult(value: unknown): value is HmiValuesResult {
  return isRecord(value) && typeof value.connected === "boolean" && isRecord(value.values);
}

function nonce(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
  for (let index = 0; index < 32; index += 1) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

function getHtml(webview: vscode.Webview): string {
  const scriptNonce = nonce();
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta
    http-equiv="Content-Security-Policy"
    content="default-src 'none'; img-src ${webview.cspSource} https: data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';"
  />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>HMI Preview</title>
  <style>
    :root {
      color-scheme: light dark;
    }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      color: var(--vscode-editor-foreground);
      background: var(--vscode-editor-background);
    }
    header {
      position: sticky;
      top: 0;
      z-index: 2;
      display: flex;
      gap: 8px;
      align-items: center;
      padding: 10px;
      border-bottom: 1px solid var(--vscode-panel-border);
      background: var(--vscode-editor-background);
    }
    #status {
      margin-left: auto;
      font-size: 12px;
      opacity: 0.85;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    #tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      padding: 10px;
      border-bottom: 1px solid var(--vscode-panel-border);
    }
    .tab {
      border: 1px solid var(--vscode-panel-border);
      background: transparent;
      color: inherit;
      border-radius: 999px;
      padding: 4px 10px;
      cursor: pointer;
    }
    .tab.active {
      border-color: var(--vscode-focusBorder);
      background: color-mix(in srgb, var(--vscode-focusBorder) 20%, transparent);
    }
    #widgets {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
      gap: 10px;
      padding: 10px;
      padding-bottom: 24px;
    }
    .group {
      grid-column: 1 / -1;
      margin-top: 10px;
      font-weight: 700;
      opacity: 0.9;
    }
    .widget {
      border: 1px solid var(--vscode-panel-border);
      border-radius: 8px;
      padding: 8px;
      background: color-mix(in srgb, var(--vscode-editor-background) 90%, var(--vscode-editor-foreground) 10%);
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .widget-title {
      font-weight: 700;
      border: 0;
      background: transparent;
      color: inherit;
      text-align: left;
      cursor: pointer;
      padding: 0;
    }
    .widget-value {
      font-family: var(--vscode-editor-font-family);
      font-size: 13px;
      opacity: 0.95;
      word-break: break-all;
    }
    .widget-meta {
      font-size: 11px;
      opacity: 0.7;
    }
    .edit-row {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 6px;
    }
    .edit-row input {
      width: 100%;
      box-sizing: border-box;
    }
  </style>
</head>
<body>
  <header>
    <button id="refresh">Refresh</button>
    <label><input id="editMode" type="checkbox" /> Edit layout</label>
    <button id="save" disabled>Save layout</button>
    <span id="status">Loading HMI preview...</span>
  </header>
  <div id="tabs"></div>
  <div id="widgets"></div>
  <script nonce="${scriptNonce}">
    const vscode = acquireVsCodeApi();
    const state = {
      schema: null,
      values: null,
      selectedPage: null,
      editMode: false,
      overrides: {},
    };
    const elements = {
      status: document.getElementById("status"),
      tabs: document.getElementById("tabs"),
      widgets: document.getElementById("widgets"),
      refresh: document.getElementById("refresh"),
      editMode: document.getElementById("editMode"),
      save: document.getElementById("save"),
    };

    function setStatus(text) {
      elements.status.textContent = String(text || "");
    }

    function isFiniteNumber(value) {
      return typeof value === "number" && Number.isFinite(value);
    }

    function recordOverride(path, key, value) {
      if (!state.overrides[path]) {
        state.overrides[path] = {};
      }
      if (value === "" || value === null || value === undefined) {
        delete state.overrides[path][key];
      } else {
        state.overrides[path][key] = value;
      }
      if (Object.keys(state.overrides[path]).length === 0) {
        delete state.overrides[path];
      }
      elements.save.disabled = Object.keys(state.overrides).length === 0;
    }

    function toDisplayValue(record) {
      if (!record) {
        return "n/a";
      }
      const value = record.v;
      if (typeof value === "string") {
        return value;
      }
      return JSON.stringify(value);
    }

    function renderTabs() {
      const pages = Array.isArray(state.schema?.pages) ? state.schema.pages : [];
      if (!state.selectedPage && pages.length > 0) {
        state.selectedPage = pages[0].id;
      }
      const validSelected = pages.some((page) => page.id === state.selectedPage);
      if (!validSelected && pages.length > 0) {
        state.selectedPage = pages[0].id;
      }
      elements.tabs.innerHTML = "";
      for (const page of pages) {
        const button = document.createElement("button");
        button.className = "tab" + (page.id === state.selectedPage ? " active" : "");
        button.textContent = page.title || page.id;
        button.addEventListener("click", () => {
          state.selectedPage = page.id;
          render();
        });
        elements.tabs.appendChild(button);
      }
    }

    function renderWidgets() {
      elements.widgets.innerHTML = "";
      if (!state.schema) {
        return;
      }
      const allWidgets = Array.isArray(state.schema.widgets) ? state.schema.widgets : [];
      const visible = state.selectedPage
        ? allWidgets.filter((widget) => widget.page === state.selectedPage)
        : allWidgets;
      let lastGroup = "";
      for (const widget of visible) {
        if (widget.group !== lastGroup) {
          const group = document.createElement("div");
          group.className = "group";
          group.textContent = widget.group;
          elements.widgets.appendChild(group);
          lastGroup = widget.group;
        }
        const card = document.createElement("article");
        card.className = "widget";

        const title = document.createElement("button");
        title.className = "widget-title";
        title.textContent = widget.label;
        title.title = "Open declaration";
        title.addEventListener("click", () => {
          vscode.postMessage({ type: "navigateWidget", payload: { id: widget.id } });
        });
        card.appendChild(title);

        const value = document.createElement("div");
        value.className = "widget-value";
        value.textContent = toDisplayValue(state.values?.values?.[widget.id]);
        card.appendChild(value);

        const meta = document.createElement("div");
        meta.className = "widget-meta";
        meta.textContent =
          widget.path +
          " | " +
          widget.data_type +
          (widget.unit ? " (" + widget.unit + ")" : "");
        card.appendChild(meta);

        if (state.editMode) {
          const rowA = document.createElement("div");
          rowA.className = "edit-row";
          const labelInput = document.createElement("input");
          labelInput.placeholder = "Label";
          labelInput.value = widget.label || "";
          labelInput.addEventListener("change", () => {
            const text = labelInput.value.trim();
            recordOverride(widget.path, "label", text || null);
          });
          const pageInput = document.createElement("input");
          pageInput.placeholder = "Page ID";
          pageInput.value = widget.page || "";
          pageInput.addEventListener("change", () => {
            const text = pageInput.value.trim();
            recordOverride(widget.path, "page", text || null);
          });
          rowA.appendChild(labelInput);
          rowA.appendChild(pageInput);
          card.appendChild(rowA);

          const rowB = document.createElement("div");
          rowB.className = "edit-row";
          const groupInput = document.createElement("input");
          groupInput.placeholder = "Group";
          groupInput.value = widget.group || "";
          groupInput.addEventListener("change", () => {
            const text = groupInput.value.trim();
            recordOverride(widget.path, "group", text || null);
          });
          const orderInput = document.createElement("input");
          orderInput.type = "number";
          orderInput.placeholder = "Order";
          orderInput.value = isFiniteNumber(widget.order) ? String(widget.order) : "";
          orderInput.addEventListener("change", () => {
            const text = orderInput.value.trim();
            if (!text) {
              recordOverride(widget.path, "order", null);
              return;
            }
            const numeric = Number(text);
            if (!Number.isFinite(numeric)) {
              return;
            }
            recordOverride(widget.path, "order", Math.trunc(numeric));
          });
          rowB.appendChild(groupInput);
          rowB.appendChild(orderInput);
          card.appendChild(rowB);
        }

        elements.widgets.appendChild(card);
      }
    }

    function render() {
      if (!state.schema) {
        elements.tabs.innerHTML = "";
        elements.widgets.innerHTML = "<div style='padding:10px;'>No HMI schema available.</div>";
        return;
      }
      renderTabs();
      renderWidgets();
    }

    window.addEventListener("message", (event) => {
      const message = event.data;
      if (!message || typeof message.type !== "string") {
        return;
      }
      if (message.type === "schema") {
        state.schema = message.payload || null;
        state.overrides = {};
        elements.save.disabled = true;
        render();
        return;
      }
      if (message.type === "values") {
        state.values = message.payload || null;
        renderWidgets();
        return;
      }
      if (message.type === "status") {
        setStatus(message.payload);
        return;
      }
      if (message.type === "layoutSaved") {
        if (message.payload && message.payload.ok) {
          state.overrides = {};
          elements.save.disabled = true;
        }
      }
    });

    elements.refresh.addEventListener("click", () => {
      vscode.postMessage({ type: "refreshSchema" });
    });

    elements.editMode.addEventListener("change", () => {
      state.editMode = Boolean(elements.editMode.checked);
      if (!state.editMode) {
        state.overrides = {};
        elements.save.disabled = true;
      }
      render();
    });

    elements.save.addEventListener("click", () => {
      vscode.postMessage({
        type: "saveLayout",
        payload: { widgets: state.overrides },
      });
    });

    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
}

export function __testSetControlRequestHandler(handler?: ControlRequestHandler): void {
  controlRequest = handler ?? sendControlRequest;
}

export function __testGetHmiPanelState(): {
  hasPanel: boolean;
  schema: HmiSchemaResult | undefined;
  values: HmiValuesResult | undefined;
  status: string;
  overrides: LayoutOverrides;
} {
  return {
    hasPanel: Boolean(panel),
    schema: effectiveSchema ? { ...effectiveSchema, widgets: [...effectiveSchema.widgets] } : undefined,
    values: lastValues ? { ...lastValues, values: { ...lastValues.values } } : undefined,
    status: lastStatus,
    overrides: { ...overrides },
  };
}

export async function __testForceRefreshSchema(): Promise<void> {
  await refreshSchema();
}

export async function __testForcePollValues(): Promise<void> {
  await pollValues(true);
}

export function __testResetHmiPanelState(): void {
  stopPolling();
  if (panel) {
    panel.dispose();
    panel = undefined;
  }
  baseSchema = undefined;
  effectiveSchema = undefined;
  lastValues = undefined;
  lastStatus = "";
  overrides = {};
  controlRequest = sendControlRequest;
}

export function __testApplyLayoutOverrides(
  schema: HmiSchemaResult,
  localOverrides: LayoutOverrides
): HmiSchemaResult {
  return applyLayoutOverrides(schema, localOverrides);
}

export function __testValidateLayoutSavePayload(payload: unknown): LayoutOverrides {
  return validateLayoutSavePayload(payload);
}

export async function __testSaveLayoutPayload(
  workspaceUri: vscode.Uri,
  payload: unknown
): Promise<LayoutOverrides> {
  const parsed = validateLayoutSavePayload(payload);
  await saveLayoutOverrides(workspaceUri, parsed);
  return parsed;
}

export async function __testLoadLayoutOverrides(workspaceUri: vscode.Uri): Promise<LayoutOverrides> {
  return await loadLayoutOverrides(workspaceUri);
}

export async function __testResolveWidgetLocation(
  widget: HmiWidgetSchema
): Promise<vscode.Location | undefined> {
  return await resolveWidgetLocation(widget);
}
