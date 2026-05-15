import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  isHmiAlarmResult,
  isHmiSchemaResult,
  isHmiTrendResult,
  isHmiValuesResult,
  isRecord,
} from "./hmi-panel/contracts";
import { createControlRequestSender, runtimeEndpointSettings } from "./hmi-panel/transport";
import type {
  ControlRequestHandler,
  HmiAlarmResult,
  HmiPageSchema,
  HmiSchemaResult,
  HmiSceneBindingSchema,
  HmiSceneInteractionSchema,
  HmiTrendResult,
  HmiValuesResult,
} from "./hmi-panel/types";

const TRUST_TWIN_PANEL_VIEW_TYPE = "trust-twin-3d-panel";
const TRUST_TWIN_ASSET_ROOT = "trust-twin";
const DESCRIPTOR_REFRESH_DEBOUNCE_MS = 150;

type WorkspaceViewStatus = {
  path: string;
  loaded: boolean;
  bytes: number;
  error?: string;
};

type TrustTwinPanelState = {
  hasPanel: boolean;
  status: string;
  connected: boolean;
  schema?: HmiSchemaResult;
  activePage?: HmiPageSchema;
  pages: HmiPageSchema[];
  breadcrumbs: string[];
  values?: HmiValuesResult;
  trends?: HmiTrendResult;
  alarms?: HmiAlarmResult;
  valuesBySource: Record<string, unknown>;
  workspaceView?: WorkspaceViewStatus;
};

type TrustTwinPackageProof = {
  ok: boolean;
  assets: string[];
  missing: string[];
};

let panel: vscode.WebviewPanel | undefined;
let pollTimer: NodeJS.Timeout | undefined;
let descriptorRefreshTimer: NodeJS.Timeout | undefined;
let lastSchema: HmiSchemaResult | undefined;
let activePage: HmiPageSchema | undefined;
let activePageId: string | undefined;
let lastValues: HmiValuesResult | undefined;
let lastTrends: HmiTrendResult | undefined;
let lastAlarms: HmiAlarmResult | undefined;
let lastStatus = "";
let connected = false;
let valuesBySource: Record<string, unknown> = {};
let workspaceView: WorkspaceViewStatus | undefined;
let controlRequest: ControlRequestHandler = createControlRequestSender();

export function registerTrustTwinPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.trustTwin.openPanel", async (options?: unknown) => {
      const pageId = parsePageIdOption(options);
      if (pageId) {
        activePageId = pageId;
      }
      await showPanel(context);
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.trustTwin.refreshPanel", async (options?: unknown) => {
      if (!panel) {
        return false;
      }
      const pageId = parsePageIdOption(options);
      if (pageId) {
        activePageId = pageId;
      }
      await refreshScene();
      return true;
    }),
  );

  const descriptorWatchers = [
    vscode.workspace.createFileSystemWatcher("**/hmi/*.toml"),
    vscode.workspace.createFileSystemWatcher("**/hmi/views/*.view.toml"),
  ];
  for (const descriptorWatcher of descriptorWatchers) {
    context.subscriptions.push(
      descriptorWatcher,
      descriptorWatcher.onDidChange(scheduleSceneRefresh),
      descriptorWatcher.onDidCreate(scheduleSceneRefresh),
      descriptorWatcher.onDidDelete(scheduleSceneRefresh),
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
        void refreshScene();
      }
      if (event.affectsConfiguration("trust-lsp.hmi.pollIntervalMs")) {
        startPolling();
      }
    }),
  );
}

async function showPanel(context: vscode.ExtensionContext): Promise<void> {
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
    await initializePanel();
    return;
  }

  panel = vscode.window.createWebviewPanel(
    TRUST_TWIN_PANEL_VIEW_TYPE,
    "trust-twin 3D Panel",
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: trustTwinLocalResourceRoots(context),
    },
  );
  panel.webview.html = getTrustTwinPanelHtml(panel.webview, context);

  panel.onDidDispose(() => {
    panel = undefined;
    stopPolling();
    clearScheduledSceneRefresh();
    lastSchema = undefined;
    activePage = undefined;
    activePageId = undefined;
    lastValues = undefined;
    lastTrends = undefined;
    lastAlarms = undefined;
    valuesBySource = {};
    workspaceView = undefined;
    connected = false;
  });
  panel.webview.onDidReceiveMessage((message: unknown) => {
    void handleWebviewMessage(message);
  });

  context.subscriptions.push(panel);
  await initializePanel();
}

async function initializePanel(): Promise<void> {
  await refreshScene();
  startPolling();
}

async function handleWebviewMessage(message: unknown): Promise<void> {
  if (!isRecord(message) || typeof message.type !== "string") {
    return;
  }
  switch (message.type) {
    case "ready":
      postScene();
      break;
    case "refresh":
      await refreshScene();
      break;
    case "selectPage":
      if (typeof message.pageId === "string") {
        await selectPage(message.pageId);
      }
      break;
    case "trustTwinInteraction":
      await handleTrustTwinInteractionMessage(message.payload);
      break;
    default:
      break;
  }
}

async function refreshScene(): Promise<void> {
  const endpointSettings = runtimeEndpointSettings();
  try {
    const raw = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.schema.get",
    );
    if (!isHmiSchemaResult(raw)) {
      throw new Error("runtime returned an invalid hmi.schema.get payload");
    }
    lastSchema = raw;
    activePage = selectActivePage(raw);
    const scenePage = scenePageForRender();
    workspaceView = scenePage ? await loadWorkspaceView(scenePage) : undefined;
    if (!activePage) {
      setStatus("No HMI page is available.");
      postScene();
      return;
    }
    const viewSuffix = workspaceView?.loaded
      ? `; loaded ${workspaceView.path}`
      : workspaceView?.path
        ? `; ${workspaceView.path} not loaded`
        : "";
    setStatus(
      `trust-twin page loaded (${pageTitle(activePage)}; ${sceneNodeCount(scenePage)} nodes${viewSuffix}).`,
    );
    await refreshOperatorOverlays();
    postScene();
  } catch (error) {
    connected = false;
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`trust-twin schema request failed: ${detail}`);
  }
}

async function selectPage(pageId: string): Promise<void> {
  const normalized = normalizePageId(pageId);
  if (!normalized || !lastSchema) {
    return;
  }
  const page = pageById(lastSchema, normalized);
  if (!page) {
    setStatus(`trust-twin page '${normalized}' is not available.`);
    return;
  }
  activePageId = page.id;
  activePage = page;
  await refreshOperatorOverlays();
  setStatus(`trust-twin page selected (${pageTitle(page)}).`);
  postScene();
}

function selectActivePage(schema: HmiSchemaResult): HmiPageSchema | undefined {
  const requested = activePageId ? pageById(schema, activePageId) : undefined;
  const previous = activePage?.id ? pageById(schema, activePage.id) : undefined;
  const fallback =
    schema.pages.find((page) => normalizePageKind(page.kind) === "scene3d") ?? schema.pages[0];
  const selected = requested ?? previous ?? fallback;
  activePageId = selected?.id;
  return selected;
}

function pageById(schema: HmiSchemaResult, pageId: string): HmiPageSchema | undefined {
  const normalized = normalizePageId(pageId);
  return schema.pages.find((page) => normalizePageId(page.id) === normalized);
}

function scenePageForRender(): HmiPageSchema | undefined {
  if (activePage && normalizePageKind(activePage.kind) === "scene3d") {
    return activePage;
  }
  return lastSchema?.pages.find((page) => normalizePageKind(page.kind) === "scene3d");
}

function normalizePageKind(value: string | null | undefined): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function normalizePageId(value: string | null | undefined): string {
  return typeof value === "string" ? value.trim() : "";
}

function pageTitle(page: HmiPageSchema | undefined): string {
  if (!page) {
    return "none";
  }
  return typeof page.title === "string" && page.title.trim() ? page.title.trim() : page.id;
}

function sceneNodeCount(page: HmiPageSchema | undefined): number {
  return Array.isArray(page?.scene_view?.node) ? page.scene_view.node.length : 0;
}

function parsePageIdOption(options: unknown): string | undefined {
  if (!isRecord(options) || typeof options.pageId !== "string") {
    return undefined;
  }
  return normalizePageId(options.pageId) || undefined;
}

async function loadWorkspaceView(page: HmiPageSchema): Promise<WorkspaceViewStatus | undefined> {
  const normalized = normalizeViewPath(page.view);
  if (!normalized) {
    return undefined;
  }
  const folder = pickWorkspaceFolder();
  if (!folder) {
    return { path: normalized, loaded: false, bytes: 0, error: "no workspace folder" };
  }
  const uri = vscode.Uri.joinPath(folder.uri, "hmi", ...normalized.split("/"));
  try {
    const bytes = await vscode.workspace.fs.readFile(uri);
    return { path: normalized, loaded: true, bytes: bytes.byteLength };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { path: normalized, loaded: false, bytes: 0, error: detail };
  }
}

function normalizeViewPath(value: string | null | undefined): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim().replace(/\\/g, "/").replace(/^\/+/, "");
  if (!normalized || !normalized.endsWith(".view.toml")) {
    return undefined;
  }
  const parts = normalized.split("/").filter(Boolean);
  if (
    parts.length === 0 ||
    parts.some((part) => part === "." || part === ".." || !/^[A-Za-z0-9._-]+$/.test(part))
  ) {
    return undefined;
  }
  return parts.join("/");
}

async function pollValues(force = false): Promise<void> {
  if (!panel || !lastSchema || !activePage || (!force && !panel.visible)) {
    return;
  }
  const ids = valueRequestIds(lastSchema, activePage, scenePageForRender());
  if (ids.length === 0) {
    return;
  }
  const endpointSettings = runtimeEndpointSettings();
  try {
    const raw = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.values.get",
      { ids },
    );
    if (!isHmiValuesResult(raw)) {
      throw new Error("runtime returned an invalid hmi.values.get payload");
    }
    lastValues = raw;
    connected = raw.connected;
    valuesBySource = mapValuesBySource(lastSchema, raw);
    await refreshOperatorOverlays();
    setStatus(`trust-twin values refreshed (${raw.connected ? "connected" : "disconnected"}).`);
    postScene();
  } catch (error) {
    connected = false;
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`trust-twin values disconnected: ${detail}`);
    postScene();
  }
}

async function refreshOperatorOverlays(): Promise<void> {
  if (!lastSchema || !activePage) {
    return;
  }
  const endpointSettings = runtimeEndpointSettings();
  const ids = trendRequestIds(lastSchema, activePage, scenePageForRender());
  try {
    const trends = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.trends.get",
      { ids, duration_ms: 10 * 60 * 1_000, buckets: 120 },
    );
    if (isHmiTrendResult(trends)) {
      lastTrends = trends;
    }
  } catch (error) {
    lastTrends = undefined;
  }
  try {
    const alarms = await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.alarms.get",
      { limit: 100 },
    );
    if (isHmiAlarmResult(alarms)) {
      lastAlarms = alarms;
    }
  } catch (error) {
    lastAlarms = undefined;
  }
}

function valueRequestIds(
  schema: HmiSchemaResult,
  page: HmiPageSchema,
  scenePage?: HmiPageSchema,
): string[] {
  const ids = new Set<string>();
  const widgetByPath = new Map(schema.widgets.map((widget) => [widget.path, widget.id]));
  for (const binding of sceneBindings(scenePage ?? page)) {
    const source = binding.source.trim();
    const widgetId = widgetByPath.get(source) ?? source;
    if (widgetId) {
      ids.add(widgetId);
    }
  }
  for (const signal of page.signals ?? []) {
    const source = signal.trim();
    const widgetId = widgetByPath.get(source) ?? source;
    if (widgetId) {
      ids.add(widgetId);
    }
  }
  return [...ids];
}

function trendRequestIds(
  schema: HmiSchemaResult,
  page: HmiPageSchema,
  scenePage?: HmiPageSchema,
): string[] {
  return valueRequestIds(schema, page, scenePage).filter((id) =>
    schema.widgets.some((widget) => widget.id === id || widget.path === id),
  );
}

function sceneBindings(page: HmiPageSchema): HmiSceneBindingSchema[] {
  if (Array.isArray(page.scene_view?.bind3d)) {
    return page.scene_view.bind3d;
  }
  return Array.isArray(page.bind3d) ? page.bind3d : [];
}

function mapValuesBySource(
  schema: HmiSchemaResult,
  values: HmiValuesResult,
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const widget of schema.widgets) {
    const record = values.values[widget.id] ?? values.values[widget.path];
    if (record) {
      result[widget.path] = record.v;
      result[widget.id] = record.v;
    }
  }
  return result;
}

async function handleTrustTwinInteractionMessage(payload: unknown): Promise<void> {
  const parsed = parseTrustTwinInteractionPayload(payload);
  if (!parsed) {
    setStatus("trust-twin interaction rejected: invalid hmi.write descriptor.");
    return;
  }
  if (parsed.interaction.required_role.trim().toLowerCase() !== "engineer") {
    setStatus("trust-twin interaction rejected: hmi.write requires Engineer role.");
    return;
  }
  const endpointSettings = runtimeEndpointSettings();
  try {
    await controlRequest(
      endpointSettings.endpoint,
      endpointSettings.authToken,
      "hmi.write",
      { id: parsed.interaction.id, value: parsed.interaction.value },
    );
    setStatus(`trust-twin write queued from ${parsed.node}.`);
    await pollValues(true);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(`trust-twin write rejected from ${parsed.node}: ${detail}`);
  }
}

function parseTrustTwinInteractionPayload(
  payload: unknown,
): { node: string; interaction: HmiSceneInteractionSchema } | undefined {
  if (!isRecord(payload) || !isRecord(payload.interaction)) {
    return undefined;
  }
  const node = typeof payload.node === "string" ? payload.node.trim() : "";
  const interaction = payload.interaction;
  const id = typeof interaction.id === "string" ? interaction.id.trim() : "";
  const action = typeof interaction.action === "string" ? interaction.action.trim() : "";
  if (!node || !id || action.toLowerCase() !== "hmi.write") {
    return undefined;
  }
  return {
    node,
    interaction: {
      event: typeof interaction.event === "string" ? interaction.event : "click",
      action: "hmi.write",
      id,
      value: interaction.value,
      required_role:
        typeof interaction.required_role === "string"
          ? interaction.required_role
          : "Engineer",
      confirmation: isRecord(interaction.confirmation)
        ? {
            title:
              typeof interaction.confirmation.title === "string"
                ? interaction.confirmation.title
                : "",
            message:
              typeof interaction.confirmation.message === "string"
                ? interaction.confirmation.message
                : "",
          }
        : null,
    },
  };
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

function scheduleSceneRefresh(): void {
  if (!panel) {
    return;
  }
  clearScheduledSceneRefresh();
  descriptorRefreshTimer = setTimeout(() => {
    descriptorRefreshTimer = undefined;
    void refreshScene();
  }, DESCRIPTOR_REFRESH_DEBOUNCE_MS);
}

function clearScheduledSceneRefresh(): void {
  if (!descriptorRefreshTimer) {
    return;
  }
  clearTimeout(descriptorRefreshTimer);
  descriptorRefreshTimer = undefined;
}

function postScene(): void {
  if (!panel) {
    return;
  }
  void panel.webview.postMessage({
    type: "scene",
    payload: {
      page: activePage,
      scenePage: scenePageForRender(),
      pages: sortedPages(lastSchema),
      breadcrumbs: breadcrumbsFor(activePage),
      values: lastValues,
      trends: lastTrends,
      alarms: lastAlarms,
      valuesBySource,
      connected,
      workspaceView,
    },
  });
  void panel.webview.postMessage({ type: "status", payload: lastStatus });
}

function setStatus(message: string): void {
  lastStatus = message;
  if (!panel) {
    return;
  }
  void panel.webview.postMessage({ type: "status", payload: message });
}

function sortedPages(schema: HmiSchemaResult | undefined): HmiPageSchema[] {
  return [...(schema?.pages ?? [])].sort((left, right) =>
    left.order - right.order || left.id.localeCompare(right.id),
  );
}

function breadcrumbsFor(page: HmiPageSchema | undefined): string[] {
  if (!page) {
    return [];
  }
  const scenePage = lastSchema?.pages.find((entry) => normalizePageKind(entry.kind) === "scene3d");
  if (scenePage && scenePage.id !== page.id) {
    return [pageTitle(scenePage), pageTitle(page)];
  }
  return [pageTitle(page)];
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

function trustTwinLocalResourceRoots(context: vscode.ExtensionContext): vscode.Uri[] {
  const roots = [vscode.Uri.joinPath(context.extensionUri, "media", TRUST_TWIN_ASSET_ROOT)];
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    roots.push(vscode.Uri.joinPath(folder.uri, "hmi"));
    roots.push(vscode.Uri.joinPath(folder.uri, "hmi", "views"));
  }
  return roots;
}

function nonce(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
  for (let index = 0; index < 32; index += 1) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

function buildTrustTwinCsp(cspSource: string, scriptNonce: string): string {
  return [
    "default-src 'none'",
    `img-src ${cspSource} data:`,
    `style-src ${cspSource} 'unsafe-inline'`,
    `script-src 'nonce-${scriptNonce}' 'wasm-unsafe-eval'`,
    `connect-src ${cspSource}`,
  ].join("; ");
}

function getTrustTwinPanelHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext,
): string {
  const scriptNonce = nonce();
  const assetRoot = vscode.Uri.joinPath(context.extensionUri, "media", TRUST_TWIN_ASSET_ROOT);
  const rendererScriptUri = webview.asWebviewUri(
    vscode.Uri.joinPath(assetRoot, "trust-twin-renderer.js"),
  );
  const rendererWasmUri = webview.asWebviewUri(
    vscode.Uri.joinPath(assetRoot, "trust-twin-renderer.wasm"),
  );
  const csp = buildTrustTwinCsp(webview.cspSource, scriptNonce);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>trust-twin 3D Panel</title>
  <style>
    :root { color-scheme: light dark; }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      color: var(--vscode-editor-foreground);
      background: var(--vscode-editor-background);
    }
    header {
      display: flex;
      gap: 8px;
      align-items: center;
      padding: 10px;
      border-bottom: 1px solid var(--vscode-panel-border);
      background: var(--vscode-editor-background);
    }
    #pages {
      display: flex;
      gap: 6px;
      align-items: center;
      min-width: 0;
      overflow-x: auto;
    }
    .page-button {
      white-space: nowrap;
      color: var(--vscode-button-secondaryForeground);
      background: var(--vscode-button-secondaryBackground);
    }
    .page-button.active {
      color: var(--vscode-button-foreground);
      background: var(--vscode-button-background);
    }
    button {
      border: 1px solid var(--vscode-button-border, transparent);
      color: var(--vscode-button-foreground);
      background: var(--vscode-button-background);
      padding: 4px 10px;
      cursor: pointer;
    }
    #status {
      margin-left: auto;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 12px;
      opacity: 0.88;
    }
    #surface {
      position: relative;
      height: calc(100vh - 44px);
      min-height: 420px;
      overflow: hidden;
      background:
        linear-gradient(0deg, color-mix(in srgb, var(--vscode-editor-background) 90%, transparent), color-mix(in srgb, var(--vscode-editor-background) 90%, transparent)),
        repeating-linear-gradient(90deg, transparent 0 47px, color-mix(in srgb, var(--vscode-panel-border) 34%, transparent) 48px),
        repeating-linear-gradient(0deg, transparent 0 47px, color-mix(in srgb, var(--vscode-panel-border) 34%, transparent) 48px);
    }
    .node {
      position: absolute;
      min-width: 76px;
      max-width: 150px;
      min-height: 38px;
      padding: 7px 10px;
      border: 1px solid var(--vscode-panel-border);
      border-radius: 6px;
      transform: translate(-50%, -50%);
      box-sizing: border-box;
      text-align: center;
      overflow-wrap: anywhere;
      background: color-mix(in srgb, var(--vscode-button-secondaryBackground) 64%, var(--vscode-editor-background));
      transition: left 180ms linear, top 180ms linear, transform 180ms linear, background-color 180ms linear, box-shadow 180ms linear;
    }
    .node.robot-base {
      border-radius: 4px;
      font-size: 10px;
      font-weight: 600;
    }
    .node.robot-link,
    .node.robot-wrist,
    .node.robot-gripper {
      min-width: 34px;
      min-height: 14px;
      padding: 2px 6px;
      border-radius: 4px;
      font-size: 10px;
      font-weight: 600;
      transform-origin: 50% 50%;
    }
    .node.robot-jaw {
      min-width: 18px;
      min-height: 30px;
      padding: 2px;
      border-radius: 3px;
      font-size: 0;
    }
    .node.robot-box {
      min-width: 42px;
      min-height: 42px;
      padding: 4px;
      border-radius: 3px;
      font-size: 10px;
      font-weight: 700;
    }
    .node.robot-zone {
      min-width: 92px;
      min-height: 30px;
      padding: 4px;
      border-style: dashed;
      font-size: 10px;
      font-weight: 600;
    }
    .node.robot-light {
      min-width: 28px;
      min-height: 28px;
      width: 28px;
      height: 28px;
      padding: 0;
      border-radius: 999px;
      font-size: 0;
      border-color: color-mix(in srgb, var(--vscode-panel-border) 50%, #ffffff);
    }
    button.node:hover {
      border-color: var(--vscode-focusBorder);
      background: color-mix(in srgb, var(--vscode-focusBorder) 18%, var(--vscode-button-secondaryBackground));
    }
    .offline .node {
      opacity: 0.6;
      filter: grayscale(0.72);
    }
    .empty {
      padding: 14px;
      opacity: 0.78;
    }
    #meta {
      position: absolute;
      left: 10px;
      bottom: 10px;
      font-size: 11px;
      opacity: 0.76;
      background: color-mix(in srgb, var(--vscode-editor-background) 86%, transparent);
      border: 1px solid var(--vscode-panel-border);
      padding: 5px 7px;
      border-radius: 4px;
    }
    #breadcrumbs {
      position: absolute;
      left: 10px;
      top: 10px;
      max-width: 54vw;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 12px;
      opacity: 0.86;
    }
    #alarmBar {
      position: absolute;
      right: 10px;
      top: 10px;
      max-width: min(420px, 42vw);
      padding: 6px 8px;
      border: 1px solid var(--vscode-inputValidation-errorBorder);
      background: color-mix(in srgb, var(--vscode-inputValidation-errorBackground) 72%, var(--vscode-editor-background));
      color: var(--vscode-inputValidation-errorForeground, var(--vscode-editor-foreground));
      font-size: 12px;
      border-radius: 4px;
    }
    #trendOverlay {
      position: absolute;
      right: 10px;
      bottom: 10px;
      width: min(360px, 42vw);
      min-height: 96px;
      border: 1px solid var(--vscode-panel-border);
      background: color-mix(in srgb, var(--vscode-editor-background) 88%, transparent);
      border-radius: 4px;
      padding: 8px;
      box-sizing: border-box;
      font-size: 12px;
    }
    #trendOverlay svg {
      display: block;
      width: 100%;
      height: 56px;
      margin-top: 6px;
    }
  </style>
</head>
<body>
  <header>
    <button id="refresh">Refresh</button>
    <nav id="pages" aria-label="HMI pages"></nav>
    <span id="status">Loading trust-twin panel...</span>
  </header>
  <main id="surface" class="offline"></main>
  <script nonce="${scriptNonce}" src="${rendererScriptUri}" data-wasm-uri="${rendererWasmUri}"></script>
  <script nonce="${scriptNonce}">
    const vscode = acquireVsCodeApi();
    const state = {
      page: null,
      scenePage: null,
      pages: [],
      breadcrumbs: [],
      connected: false,
      valuesBySource: {},
      trends: null,
      alarms: null,
      workspaceView: null
    };
    const status = document.getElementById("status");
    const surface = document.getElementById("surface");
    const pages = document.getElementById("pages");
    document.getElementById("refresh").addEventListener("click", () => {
      vscode.postMessage({ type: "refresh" });
    });
    function setStatus(text) {
      status.textContent = String(text || "");
    }
    function nodePosition(node, axis, fallback) {
      const position = node && node.transform && Array.isArray(node.transform.position)
        ? node.transform.position
        : null;
      if (!position || position.length < 3) {
        return fallback;
      }
      const value = Number(position[axis]);
      return Number.isFinite(value) ? value : fallback;
    }
    function projectedNodeX(node, fallback) {
      return nodePosition(node, 0, fallback) + nodePosition(node, 2, 0);
    }
    function interactions(node) {
      if (Array.isArray(node && node.interaction)) {
        return node.interaction;
      }
      if (Array.isArray(node && node.interactions)) {
        return node.interactions;
      }
      return [];
    }
    function cloneNode(node) {
      return JSON.parse(JSON.stringify(node || {}));
    }
    function ensureTransform(node) {
      if (!node.transform || typeof node.transform !== "object") {
        node.transform = {};
      }
      if (!Array.isArray(node.transform.position)) {
        node.transform.position = [0, 0, 0];
      }
      if (!Array.isArray(node.transform.rotation)) {
        node.transform.rotation = [0, 0, 0];
      }
      if (!Array.isArray(node.transform.scale)) {
        node.transform.scale = [1, 1, 1];
      }
      return node.transform;
    }
    function ensureMaterial(node) {
      if (!node.material || typeof node.material !== "object") {
        node.material = {};
      }
      return node.material;
    }
    function mapKey(value) {
      if (typeof value === "boolean") {
        return value ? "true" : "false";
      }
      return String(value);
    }
    function mappedBindingValue(binding, raw) {
      if (!binding || !binding.map || typeof binding.map !== "object") {
        return raw;
      }
      const key = mapKey(raw);
      if (Object.prototype.hasOwnProperty.call(binding.map, key)) {
        return binding.map[key];
      }
      const lower = key.toLowerCase();
      if (Object.prototype.hasOwnProperty.call(binding.map, lower)) {
        return binding.map[lower];
      }
      return raw;
    }
    function numericBindingValue(binding, raw) {
      const mapped = mappedBindingValue(binding, raw);
      const numeric = Number(mapped);
      if (!Number.isFinite(numeric)) {
        return undefined;
      }
      const scale = binding && binding.scale;
      if (!scale || !Number.isFinite(Number(scale.min)) || !Number.isFinite(Number(scale.max))) {
        return numeric;
      }
      const min = Number(scale.min);
      const max = Number(scale.max);
      if (max === min) {
        return numeric;
      }
      const ratio = Math.max(0, Math.min(1, (numeric - min) / (max - min)));
      return Number(scale.output_min || 0) + ratio * (Number(scale.output_max || 0) - Number(scale.output_min || 0));
    }
    function boolBindingValue(binding, raw) {
      const mapped = mappedBindingValue(binding, raw);
      if (typeof mapped === "boolean") {
        return mapped;
      }
      const text = String(mapped).trim().toLowerCase();
      return text === "true" || text === "1" || text === "on" || text === "yes";
    }
    function textBindingValue(binding, raw) {
      const mapped = mappedBindingValue(binding, raw);
      return mapped == null ? "" : String(mapped);
    }
    function applySceneBindings(nodes, bindings, valuesBySource) {
      const nextNodes = nodes.map(cloneNode);
      const originalNodes = nodes.map(cloneNode);
      const byId = new Map(nextNodes.map((node) => [String(node.id || ""), node]));
      const originalById = new Map(originalNodes.map((node) => [String(node.id || ""), node]));
      const boundPositionAxes = new Map();
      function markBoundPositionAxis(nodeId, axis) {
        const axes = boundPositionAxes.get(nodeId) || new Set();
        axes.add(axis);
        boundPositionAxes.set(nodeId, axes);
      }
      (Array.isArray(bindings) ? bindings : []).forEach((binding) => {
        if (!binding || typeof binding.node !== "string" || typeof binding.property !== "string") {
          return;
        }
        const node = byId.get(binding.node);
        if (!node || !valuesBySource || !Object.prototype.hasOwnProperty.call(valuesBySource, binding.source)) {
          return;
        }
        const raw = valuesBySource[binding.source];
        const property = binding.property.trim().toLowerCase();
        const transform = ensureTransform(node);
        const material = ensureMaterial(node);
        if (property === "visible") {
          node.visible = boolBindingValue(binding, raw);
          return;
        }
        if (property === "transform.position.x") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) {
            transform.position[0] = value;
            markBoundPositionAxis(binding.node, 0);
          }
          return;
        }
        if (property === "transform.position.y") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) {
            transform.position[1] = value;
            markBoundPositionAxis(binding.node, 1);
          }
          return;
        }
        if (property === "transform.position.z") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) {
            transform.position[2] = value;
            markBoundPositionAxis(binding.node, 2);
          }
          return;
        }
        if (property === "transform.rotation.x") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) transform.rotation[0] = value;
          return;
        }
        if (property === "transform.rotation.y") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) transform.rotation[1] = value;
          return;
        }
        if (property === "transform.rotation.z") {
          const value = numericBindingValue(binding, raw);
          if (value !== undefined) transform.rotation[2] = value;
          return;
        }
        if (property === "material.base_color") {
          material.base_color = textBindingValue(binding, raw);
          return;
        }
        if (property === "material.emissive") {
          material.emissive = textBindingValue(binding, raw);
        }
      });
      nextNodes.forEach((node) => {
        const nodeId = String(node.id || "");
        const separator = nodeId.lastIndexOf(".");
        if (separator <= 0) {
          return;
        }
        const parentId = nodeId.slice(0, separator);
        const parent = byId.get(parentId);
        const originalParent = originalById.get(parentId);
        const originalNode = originalById.get(nodeId);
        if (!parent || !originalParent || !originalNode) {
          return;
        }
        const transform = ensureTransform(node);
        const parentPosition = ensureTransform(parent).position;
        const originalParentPosition = ensureTransform(originalParent).position;
        const originalPosition = ensureTransform(originalNode).position;
        const boundAxes = boundPositionAxes.get(nodeId) || new Set();
        for (const axis of [0, 1, 2]) {
          const local = boundAxes.has(axis)
            ? Number(transform.position[axis])
            : Number(originalPosition[axis]) - Number(originalParentPosition[axis]);
          if (Number.isFinite(local) && Number.isFinite(Number(parentPosition[axis]))) {
            transform.position[axis] = Number(parentPosition[axis]) + local;
          }
        }
      });
      return nextNodes;
    }
    function isTrustTwinPrimitiveNode(nodeId) {
      return (
        nodeId.startsWith("robot.") ||
        nodeId.startsWith("box.") ||
        nodeId.startsWith("zone.") ||
        nodeId === "ROBOT-1" ||
        nodeId.startsWith("ROBOT-1.") ||
        nodeId === "GRIPPER-1" ||
        nodeId.startsWith("GRIPPER-1.") ||
        nodeId === "BOX-1" ||
        nodeId === "PICKUP-1" ||
        nodeId === "DROP-1"
      );
    }
    function nodeClass(nodeId) {
      if (nodeId === "ROBOT-1") return "node robot-base";
      if (nodeId === "ROBOT-1.shoulder" || nodeId === "ROBOT-1.elbow") return "node robot-link";
      if (nodeId === "ROBOT-1.wrist") return "node robot-wrist";
      if (nodeId === "GRIPPER-1") return "node robot-gripper";
      if (nodeId === "GRIPPER-1.left_jaw" || nodeId === "GRIPPER-1.right_jaw") return "node robot-jaw";
      if (nodeId === "BOX-1") return "node robot-box";
      if (nodeId === "PICKUP-1" || nodeId === "DROP-1") return "node robot-zone";
      if (nodeId === "LIGHT-1") return "node robot-light";
      if (nodeId === "robot.base") return "node robot-base";
      if (nodeId === "robot.shoulder" || nodeId === "robot.elbow") return "node robot-link";
      if (nodeId === "robot.wrist") return "node robot-wrist";
      if (nodeId === "robot.gripper") return "node robot-gripper";
      if (nodeId === "robot.gripper.left" || nodeId === "robot.gripper.right") return "node robot-jaw";
      if (nodeId.startsWith("box.")) return "node robot-box";
      if (nodeId.startsWith("zone.")) return "node robot-zone";
      if (nodeId === "robot.status_light") return "node robot-light";
      return "node";
    }
    function nodeScale(node) {
      const scale = node && node.transform && Array.isArray(node.transform.scale) ? node.transform.scale : [1, 1, 1];
      return scale.map((value) => Number.isFinite(Number(value)) ? Number(value) : 1);
    }
    function nodeRotationZ(node) {
      const rotation = node && node.transform && Array.isArray(node.transform.rotation) ? node.transform.rotation : [0, 0, 0];
      const value = Number(rotation[2]);
      return Number.isFinite(value) ? value : 0;
    }
    function nodeColor(node) {
      const material = node && node.material && typeof node.material === "object" ? node.material : {};
      return material.emissive || material.base_color || "";
    }
    function surfacePixelsPerMeter() {
      const height = surface && Number.isFinite(surface.clientHeight) ? surface.clientHeight : 600;
      return Math.max(42, Math.min(72, height * 0.085));
    }
    function surfaceFloorTop(pixelsPerMeter) {
      const height = surface && Number.isFinite(surface.clientHeight) ? surface.clientHeight : 600;
      return height - Math.max(42, pixelsPerMeter * 0.9);
    }
    function renderPages() {
      pages.innerHTML = "";
      (Array.isArray(state.pages) ? state.pages : []).forEach((page) => {
        if (!page || typeof page.id !== "string") {
          return;
        }
        const button = document.createElement("button");
        button.className = "page-button" + (state.page && state.page.id === page.id ? " active" : "");
        button.textContent = typeof page.title === "string" && page.title.trim() ? page.title.trim() : page.id;
        button.addEventListener("click", () => {
          vscode.postMessage({ type: "selectPage", pageId: page.id });
        });
        pages.appendChild(button);
      });
    }
    function renderAlarmBar() {
      const active = state.alarms && Array.isArray(state.alarms.active) ? state.alarms.active : [];
      if (!active.length) {
        return null;
      }
      const bar = document.createElement("div");
      bar.id = "alarmBar";
      const first = active[0];
      const label = typeof first.label === "string" && first.label.trim() ? first.label.trim() : first.id;
      bar.textContent = active.length + " active alarm" + (active.length === 1 ? "" : "s") + ": " + label;
      return bar;
    }
    function renderTrendOverlay() {
      const series = state.trends && Array.isArray(state.trends.series) ? state.trends.series[0] : null;
      if (!series || !Array.isArray(series.points) || !series.points.length) {
        return null;
      }
      const overlay = document.createElement("div");
      overlay.id = "trendOverlay";
      const title = document.createElement("div");
      title.textContent = (series.label || series.id) + (series.unit ? " (" + series.unit + ")" : "");
      overlay.appendChild(title);
      const values = series.points.map((point) => Number(point.value)).filter(Number.isFinite);
      if (!values.length) {
        return null;
      }
      const min = Math.min(...values);
      const max = Math.max(...values);
      const span = Math.max(1, max - min);
      const polyline = series.points.map((point, index) => {
        const x = series.points.length === 1 ? 50 : (index / (series.points.length - 1)) * 100;
        const y = 52 - ((Number(point.value) - min) / span) * 48;
        return x.toFixed(1) + "," + y.toFixed(1);
      }).join(" ");
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("viewBox", "0 0 100 56");
      const line = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
      line.setAttribute("fill", "none");
      line.setAttribute("stroke", "currentColor");
      line.setAttribute("stroke-width", "2");
      line.setAttribute("points", polyline);
      svg.appendChild(line);
      overlay.appendChild(svg);
      return overlay;
    }
    function renderOverlays() {
      const breadcrumbs = document.createElement("div");
      breadcrumbs.id = "breadcrumbs";
      breadcrumbs.textContent = (Array.isArray(state.breadcrumbs) ? state.breadcrumbs : []).join(" / ");
      surface.appendChild(breadcrumbs);
      const alarmBar = renderAlarmBar();
      if (alarmBar) {
        surface.appendChild(alarmBar);
      }
      const trendOverlay = renderTrendOverlay();
      if (trendOverlay) {
        surface.appendChild(trendOverlay);
      }
    }
    function render() {
      renderPages();
      surface.classList.toggle("offline", !state.connected);
      surface.innerHTML = "";
      const renderPage = state.scenePage || state.page;
      const rawNodes = Array.isArray(renderPage && renderPage.scene_view && renderPage.scene_view.node)
        ? renderPage.scene_view.node
        : [];
      const nodes = applySceneBindings(
        rawNodes,
        renderPage && renderPage.scene_view ? renderPage.scene_view.bind3d : [],
        state.valuesBySource
      ).filter((node) => node.visible !== false);
      if (!nodes.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "No scene3d payload is available.";
        surface.appendChild(empty);
        renderOverlays();
        return;
      }
      const pixelsPerMeter = surfacePixelsPerMeter();
      const floorTop = surfaceFloorTop(pixelsPerMeter);
      let minX = Infinity, maxX = -Infinity;
      nodes.forEach((node, index) => {
        const x = projectedNodeX(node, index * 1.5);
        minX = Math.min(minX, x);
        maxX = Math.max(maxX, x);
      });
      const spanX = Math.max(1, maxX - minX);
      nodes.forEach((node, index) => {
        const firstWrite = interactions(node).find((entry) =>
          entry && typeof entry.action === "string" && entry.action.toLowerCase() === "hmi.write"
        );
        const element = firstWrite ? document.createElement("button") : document.createElement("div");
        const nodeId = typeof node.id === "string" && node.id.trim() ? node.id.trim() : "node-" + index;
        element.className = nodeClass(nodeId);
        element.textContent = typeof node.label === "string" && node.label.trim() ? node.label.trim() : nodeId;
        element.dataset.trustTwinNode = nodeId;
        const x = projectedNodeX(node, index * 1.5);
        const y = nodePosition(node, 1, 0);
        element.style.left = 8 + ((x - minX) / spanX) * 84 + "%";
        element.style.top = Math.round(floorTop - y * pixelsPerMeter) + "px";
        const scale = nodeScale(node);
        if (isTrustTwinPrimitiveNode(nodeId)) {
          element.style.minWidth = "0";
          element.style.minHeight = "0";
          element.style.width = Math.max(12, pixelsPerMeter * Math.abs(scale[0])) + "px";
          element.style.height = Math.max(8, pixelsPerMeter * Math.abs(scale[1])) + "px";
        }
        element.style.transform = "translate(-50%, -50%) rotate(" + nodeRotationZ(node).toFixed(4) + "rad)";
        const color = nodeColor(node);
        if (color) {
          element.style.backgroundColor = color;
        }
        const material = node.material && typeof node.material === "object" ? node.material : {};
        if (material.emissive && material.emissive !== "#000000") {
          element.style.boxShadow = "0 0 16px " + material.emissive;
        }
        if (firstWrite) {
          element.addEventListener("click", () => {
            const confirmation = firstWrite.confirmation;
            if (
              confirmation &&
              typeof confirmation.message === "string" &&
              confirmation.message.trim() &&
              !window.confirm(confirmation.message.trim())
            ) {
              return;
            }
            vscode.postMessage({
              type: "trustTwinInteraction",
              payload: { page: state.page && state.page.id, node: nodeId, interaction: firstWrite },
            });
          });
        }
        surface.appendChild(element);
      });
      const meta = document.createElement("div");
      meta.id = "meta";
      const view = state.workspaceView && state.workspaceView.path ? state.workspaceView.path : "runtime schema";
      meta.textContent = "View: " + view + " | nodes: " + nodes.length + " | " + (state.connected ? "connected" : "disconnected");
      surface.appendChild(meta);
      renderOverlays();
    }
    window.addEventListener("message", (event) => {
      const message = event.data;
      if (!message || typeof message.type !== "string") {
        return;
      }
      if (message.type === "status") {
        setStatus(message.payload);
        return;
      }
      if (message.type === "scene") {
        const payload = message.payload || {};
        state.page = payload.page || null;
        state.scenePage = payload.scenePage || null;
        state.pages = Array.isArray(payload.pages) ? payload.pages : [];
        state.breadcrumbs = Array.isArray(payload.breadcrumbs) ? payload.breadcrumbs : [];
        state.connected = !!payload.connected;
        state.valuesBySource = payload.valuesBySource || {};
        state.trends = payload.trends || null;
        state.alarms = payload.alarms || null;
        state.workspaceView = payload.workspaceView || null;
        render();
      }
    });
    window.addEventListener("trustTwinRendererWasmReady", (event) => {
      if (event.detail && event.detail.ok) {
        vscode.postMessage({ type: "ready" });
      }
    });
    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
}

export function __testSetTrustTwinControlRequestHandler(handler?: ControlRequestHandler): void {
  controlRequest = handler ?? createControlRequestSender();
}

export async function __testForceTrustTwinRefresh(): Promise<void> {
  await refreshScene();
}

export async function __testForceTrustTwinPollValues(): Promise<void> {
  await pollValues(true);
}

export async function __testForceTrustTwinSelectPage(pageId: string): Promise<void> {
  await selectPage(pageId);
}

export function __testGetTrustTwinPanelState(): TrustTwinPanelState {
  return {
    hasPanel: !!panel,
    status: lastStatus,
    connected,
    schema: lastSchema,
    activePage,
    pages: sortedPages(lastSchema),
    breadcrumbs: breadcrumbsFor(activePage),
    values: lastValues,
    trends: lastTrends,
    alarms: lastAlarms,
    valuesBySource,
    workspaceView,
  };
}

export function __testResetTrustTwinPanelState(): void {
  stopPolling();
  clearScheduledSceneRefresh();
  panel = undefined;
  lastSchema = undefined;
  activePage = undefined;
  activePageId = undefined;
  lastValues = undefined;
  lastTrends = undefined;
  lastAlarms = undefined;
  lastStatus = "";
  connected = false;
  valuesBySource = {};
  workspaceView = undefined;
  controlRequest = createControlRequestSender();
}

export function __testGetTrustTwinPanelWebviewContract(
  workspaceUri: vscode.Uri,
  extensionUri: vscode.Uri,
): { csp: string; localResourceRoots: string[] } {
  return {
    csp: buildTrustTwinCsp("${webview.cspSource}", "${nonce}"),
    localResourceRoots: [
      vscode.Uri.joinPath(extensionUri, "media", TRUST_TWIN_ASSET_ROOT).fsPath,
      vscode.Uri.joinPath(workspaceUri, "hmi").fsPath,
      vscode.Uri.joinPath(workspaceUri, "hmi", "views").fsPath,
    ],
  };
}

export function __testGetTrustTwinPanelHtmlForPlaywright(extensionRoot: string): string {
  const extensionUri = vscode.Uri.file(extensionRoot);
  const webview = {
    cspSource: "file:",
    asWebviewUri: (uri: vscode.Uri) => uri,
  } as vscode.Webview;
  const context = { extensionUri } as vscode.ExtensionContext;
  return getTrustTwinPanelHtml(webview, context);
}

export function __testGetTrustTwinPanelPackageProof(): TrustTwinPackageProof {
  const extensionRoot = path.resolve(__dirname, "..");
  const required = [
    "media/trust-twin/trust-twin-renderer.wasm",
    "media/trust-twin/trust-twin-renderer.js",
    "media/trust-twin/components/motor.gltf",
    "media/trust-twin/components/pump.gltf",
    "media/trust-twin/components/valve.gltf",
  ];
  const assets = required.filter((relativePath) =>
    fs.existsSync(path.join(extensionRoot, relativePath)),
  );
  const missing = required.filter((relativePath) => !assets.includes(relativePath));
  return { ok: missing.length === 0, assets, missing };
}
