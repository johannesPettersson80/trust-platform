import * as path from "path";
import * as vscode from "vscode";

import { isHmiSchemaResult, isHmiValuesResult, isRecord } from "./hmi-panel/contracts";
import {
  applyLayoutOverrides,
  loadLayoutOverrides,
  saveLayoutOverrides,
  validateLayoutSavePayload,
} from "./hmi-panel/layout";
import { resolveWidgetLocation } from "./hmi-panel/navigation";
import { createControlRequestSender, runtimeEndpointSettings } from "./hmi-panel/transport";
import { localSimControl } from "./simControl";
import type {
  ControlRequestHandler,
  HmiSchemaResult,
  HmiValuesResult,
  HmiWidgetSchema,
  LayoutOverrides,
} from "./hmi-panel/types";
import { getHtml } from "./hmi-panel/view";

export type { HmiWidgetSchema } from "./hmi-panel/types";

const HMI_PANEL_VIEW_TYPE = "trust-hmi-preview";
const DESCRIPTOR_REFRESH_DEBOUNCE_MS = 150;
const SEARCH_GLOB = "**/*.{st,ST,pou,POU}";
const SEARCH_EXCLUDE = "**/{.git,node_modules,target,.vscode-test}/**";

let panel: vscode.WebviewPanel | undefined;
let pollTimer: NodeJS.Timeout | undefined;
let baseSchema: HmiSchemaResult | undefined;
let effectiveSchema: HmiSchemaResult | undefined;
let lastValues: HmiValuesResult | undefined;
let lastStatus = "";
let overrides: LayoutOverrides = {};
let controlRequest: ControlRequestHandler = createControlRequestSender();
let descriptorRefreshTimer: NodeJS.Timeout | undefined;

export function registerHmiPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.hmi.openPreview", async () => {
      await showPanel(context);
    })
  );
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.hmi.refreshFromDescriptor",
      async () => {
        if (!panel) {
          return false;
        }
        await refreshSchema();
        return true;
      }
    )
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (!panel || !isRelevantForSchemaRefresh(document.uri)) {
        return;
      }
      scheduleSchemaRefresh();
    })
  );
  const descriptorWatchers = [
    vscode.workspace.createFileSystemWatcher("**/hmi/*.{toml,svg}"),
    vscode.workspace.createFileSystemWatcher("**/hmi/views/*.view.toml"),
  ];
  for (const descriptorWatcher of descriptorWatchers) {
    context.subscriptions.push(
      descriptorWatcher,
      descriptorWatcher.onDidChange((uri) => {
        if (!panel || !isRelevantForSchemaRefresh(uri)) {
          return;
        }
        scheduleSchemaRefresh();
      }),
      descriptorWatcher.onDidCreate((uri) => {
        if (!panel || !isRelevantForSchemaRefresh(uri)) {
          return;
        }
        scheduleSchemaRefresh();
      }),
      descriptorWatcher.onDidDelete((uri) => {
        if (!panel || !isRelevantForSchemaRefresh(uri)) {
          return;
        }
        scheduleSchemaRefresh();
      })
    );
  }

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
    "HMI",
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
    clearScheduledSchemaRefresh();
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
      effectiveSchema = await resolveSchemaForPanel(baseSchema, folder.uri);
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

// Prefer the running local simulator's per-workspace endpoint (the same one the debug session and
// Network Canvas use, e.g. /tmp/trust-debug-<hash>.sock) so the HMI preview connects to the LIVE
// sim — not the default control endpoint (/tmp/trust-debug.sock), which nothing serves. Falls back
// to the configured endpoint for a real remote/managed runtime.
function effectiveEndpoint(): ReturnType<typeof runtimeEndpointSettings> {
  const base = runtimeEndpointSettings();
  const sim = localSimControl(pickWorkspaceFolder()?.uri.fsPath);
  return sim ? { ...base, endpoint: sim.endpoint, authToken: sim.authToken } : base;
}

async function refreshSchema(): Promise<void> {
  const endpointSettings = effectiveEndpoint();
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
    const workspaceFolder = pickWorkspaceFolder();
    effectiveSchema = await resolveSchemaForPanel(
      raw,
      workspaceFolder ? workspaceFolder.uri : undefined
    );
    postMessage("schema", effectiveSchema);
    setStatus(
      `Schema loaded (${effectiveSchema.widgets.length} widgets, ${effectiveSchema.pages.length} pages).`
    );
    await pollValues();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    // Never surface a raw socket error to the user (F-05). A connection failure on the live-sim
    // endpoint just means the runtime isn't running yet.
    const cannotConnect = /ECONNREFUSED|ENOENT|connect|refused|not running/i.test(detail);
    setStatus(
      cannotConnect
        ? "Start the runtime to see live HMI data."
        : `Could not load the HMI: ${detail}`
    );
  }
}

async function resolveSchemaForPanel(
  schema: HmiSchemaResult,
  workspaceUri: vscode.Uri | undefined
): Promise<HmiSchemaResult> {
  const withLayout = applyLayoutOverrides(schema, overrides);
  if (!workspaceUri) {
    return withLayout;
  }
  return await hydrateProcessPageAssets(withLayout, workspaceUri);
}

async function hydrateProcessPageAssets(
  schema: HmiSchemaResult,
  workspaceUri: vscode.Uri
): Promise<HmiSchemaResult> {
  const pages = await Promise.all(
    schema.pages.map(async (page) => {
      if (normalizePageKind(page.kind) !== "process") {
        return { ...page };
      }
      const svgContent = await loadProcessSvgContent(workspaceUri, page.svg);
      return {
        ...page,
        svg_content: svgContent ?? null,
      };
    })
  );
  return { ...schema, pages };
}

async function loadProcessSvgContent(
  workspaceUri: vscode.Uri,
  svgPath: string | null | undefined
): Promise<string | undefined> {
  const normalized = normalizeProcessSvgPath(svgPath);
  if (!normalized) {
    return undefined;
  }
  const svgUri = vscode.Uri.joinPath(workspaceUri, "hmi", ...normalized.split("/"));
  const rootPath = path.resolve(workspaceUri.fsPath);
  const svgFsPath = path.resolve(svgUri.fsPath);
  const safeRootPrefix = `${rootPath}${path.sep}`;
  if (svgFsPath !== rootPath && !svgFsPath.startsWith(safeRootPrefix)) {
    return undefined;
  }
  try {
    const bytes = await vscode.workspace.fs.readFile(svgUri);
    return Buffer.from(bytes).toString("utf8");
  } catch {
    return undefined;
  }
}

function normalizeProcessSvgPath(value: string | null | undefined): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const normalized = trimmed.replace(/\\/g, "/").replace(/^\/+/, "");
  const parts = normalized.split("/").filter((part) => part.length > 0);
  if (parts.length === 0) {
    return undefined;
  }
  if (
    parts.some(
      (part) => part === "." || part === ".." || !/^[A-Za-z0-9._-]+$/.test(part)
    )
  ) {
    return undefined;
  }
  const last = parts[parts.length - 1];
  if (!last.toLowerCase().endsWith(".svg")) {
    return undefined;
  }
  return parts.join("/");
}

function normalizePageKind(value: string | null | undefined): string {
  const kind = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (kind === "process" || kind === "trend" || kind === "alarm") {
    return kind;
  }
  return "dashboard";
}

async function pollValues(force = false): Promise<void> {
  if (!panel || !effectiveSchema || (!force && !panel.visible)) {
    return;
  }
  const endpointSettings = effectiveEndpoint();
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

function scheduleSchemaRefresh(): void {
  if (!panel) {
    return;
  }
  clearScheduledSchemaRefresh();
  descriptorRefreshTimer = setTimeout(() => {
    descriptorRefreshTimer = undefined;
    void refreshSchema();
  }, DESCRIPTOR_REFRESH_DEBOUNCE_MS);
}

function clearScheduledSchemaRefresh(): void {
  if (!descriptorRefreshTimer) {
    return;
  }
  clearTimeout(descriptorRefreshTimer);
  descriptorRefreshTimer = undefined;
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
  const active = vscode.window.activeTextEditor;
  if (active) {
    const fromEditor = vscode.workspace.getWorkspaceFolder(active.document.uri);
    if (fromEditor) {
      return fromEditor;
    }
  }
  return vscode.workspace.workspaceFolders?.[0];
}

function isRelevantForSchemaRefresh(uri: vscode.Uri): boolean {
  const lower = uri.fsPath.toLowerCase();
  return (
    lower.endsWith(".st") ||
    lower.endsWith(".pou") ||
    lower.endsWith(".toml") ||
    lower.endsWith(".svg")
  );
}

export function __testSetControlRequestHandler(handler?: ControlRequestHandler): void {
  controlRequest = handler ?? createControlRequestSender();
}

export function __testGetHmiPanelState(): {
  hasPanel: boolean;
  schema?: HmiSchemaResult;
  values?: HmiValuesResult;
  status: string;
  overrides: LayoutOverrides;
} {
  return {
    hasPanel: !!panel,
    schema: effectiveSchema,
    values: lastValues,
    status: lastStatus,
    overrides,
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
  clearScheduledSchemaRefresh();
  panel = undefined;
  baseSchema = undefined;
  effectiveSchema = undefined;
  lastValues = undefined;
  lastStatus = "";
  overrides = {};
  controlRequest = createControlRequestSender();
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
): Promise<void> {
  const parsed = validateLayoutSavePayload(payload);
  await saveLayoutOverrides(workspaceUri, parsed);
}

export async function __testLoadLayoutOverrides(
  workspaceUri: vscode.Uri
): Promise<LayoutOverrides> {
  return await loadLayoutOverrides(workspaceUri);
}

export async function __testResolveWidgetLocation(
  widget: HmiWidgetSchema
): Promise<vscode.Location | undefined> {
  return await resolveWidgetLocation(widget);
}
