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
    `script-src ${cspSource} 'nonce-${scriptNonce}' 'wasm-unsafe-eval'`,
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
  const assetRootUri = webview.asWebviewUri(assetRoot);
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
    #trust-twin-canvas {
      display: block;
      width: 100%;
      height: 100%;
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
  <main id="surface"><canvas id="trust-twin-canvas"></canvas></main>
  <script type="module" nonce="${scriptNonce}">
    import initWasm, {
      init as createRenderer,
      apply_scene,
      apply_values,
      render_frame,
      set_offline,
      dispose,
      renderer_origin
    } from "${rendererScriptUri}";
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
    const canvas = document.getElementById("trust-twin-canvas");
    let rendererHandle = null;
    let rendererReady = false;
    let renderLoopStarted = false;
    let renderFramePending = false;
    let sceneSyncSerial = 0;
    const trustTwinAssetRootUri = "${assetRootUri}";
    window.__trustTwinRendererOrigin = "";
    window.__trustTwinAssetProof = null;
    window.__trustTwinRenderFrameCount = 0;
    window.__trustTwinRenderedSceneApplyCount = 0;
    window.__trustTwinRenderError = "";
    window.__trustTwinSceneApplyCount = 0;
    function resizeCanvas() {
      const rect = surface.getBoundingClientRect();
      const ratio = Math.max(1, window.devicePixelRatio || 1);
      const width = Math.max(1, Math.floor(rect.width * ratio));
      const height = Math.max(1, Math.floor(rect.height * ratio));
      if (canvas.width !== width) canvas.width = width;
      if (canvas.height !== height) canvas.height = height;
    }
    async function initializeRenderer() {
      try {
        resizeCanvas();
        await initWasm("${rendererWasmUri}");
        rendererHandle = await createRenderer(canvas);
        window.__trustTwinRendererOrigin = renderer_origin(rendererHandle);
        rendererReady = true;
        window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
          detail: {
            ok: true,
            renderer: "trust-twin-renderer",
            origin: window.__trustTwinRendererOrigin,
            contract: 2
          }
        }));
        vscode.postMessage({ type: "ready" });
        startRenderLoop();
        render();
      } catch (error) {
        const message = String(error && error.message ? error.message : error);
        setStatus("trust-twin renderer failed: " + message);
        window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
          detail: { ok: false, error: message }
        }));
      }
    }
    function startRenderLoop() {
      if (renderLoopStarted) return;
      renderLoopStarted = true;
      const tick = () => {
        if (rendererReady && rendererHandle) {
          if (renderFramePending) {
            renderFramePending = false;
            try {
              render_frame(rendererHandle);
              window.__trustTwinRenderFrameCount += 1;
              window.__trustTwinRenderedSceneApplyCount = window.__trustTwinSceneApplyCount;
              window.__trustTwinRenderError = "";
            } catch (error) {
              const message = String(error && error.message ? error.message : error);
              window.__trustTwinRenderError = message;
              setStatus("trust-twin render failed: " + message);
            }
          }
        }
        window.requestAnimationFrame(tick);
      };
      window.requestAnimationFrame(tick);
    }
    function currentScenePayload() {
      const renderPage = state.scenePage || state.page;
      return renderPage && renderPage.scene_view ? rewriteSceneAssetUris(renderPage.scene_view) : null;
    }
    function rewriteSceneAssetUris(scenePayload) {
      if (!scenePayload || typeof scenePayload !== "object") return null;
      const cloned = JSON.parse(JSON.stringify(scenePayload));
      const assetRootBase = trustTwinAssetRootUri.endsWith("/") ? trustTwinAssetRootUri : trustTwinAssetRootUri + "/";
      const assets = Array.isArray(cloned.asset)
        ? cloned.asset
        : (Array.isArray(cloned.assets) ? cloned.assets : []);
      const resolved = [];
      for (const asset of assets) {
        if (!asset || typeof asset !== "object") continue;
        const sourceUri = typeof asset.uri === "string" && asset.uri.trim()
          ? asset.uri.trim()
          : (typeof asset.id === "string" ? asset.id.trim() : "");
        if (sourceUri.startsWith("trust-twin/")) {
          asset.uri = new URL(sourceUri.slice("trust-twin/".length), assetRootBase).toString();
        }
        resolved.push({
          id: typeof asset.id === "string" ? asset.id : "",
          uri: typeof asset.uri === "string" ? asset.uri : sourceUri,
          kind: typeof asset.kind === "string" ? asset.kind : "",
        });
      }
      window.__trustTwinAssetProof = {
        asset_state: cloned.metadata && cloned.metadata.asset_state,
        metadata: cloned.metadata && typeof cloned.metadata === "object"
          ? { ...cloned.metadata }
          : {},
        asset_count: resolved.length,
        assets: resolved,
      };
      return cloned;
    }
    async function syncRendererScene() {
      if (!rendererReady || !rendererHandle) return false;
      const scenePayload = currentScenePayload();
      const nodes = Array.isArray(scenePayload && scenePayload.node) ? scenePayload.node : [];
      if (!scenePayload || !nodes.length) return false;
      const sceneJson = JSON.stringify(scenePayload);
      const serial = ++sceneSyncSerial;
      try {
        await apply_scene(rendererHandle, sceneJson);
        if (serial !== sceneSyncSerial) return false;
        setStatus("");
        apply_values(rendererHandle, JSON.stringify(state.valuesBySource || {}));
        set_offline(rendererHandle, !state.connected);
        window.__trustTwinRendererOrigin = renderer_origin(rendererHandle);
        window.__trustTwinSceneApplyCount += 1;
        window.__trustTwinRenderError = "";
        renderFramePending = true;
        return true;
      } catch (error) {
        const message = String(error && error.message ? error.message : error);
        window.__trustTwinRenderError = message;
        setStatus("trust-twin scene failed: " + message);
        return false;
      }
    }
    function clearOverlays() {
      surface.querySelectorAll("#breadcrumbs,#alarmBar,#trendOverlay,#meta,.empty").forEach((element) => element.remove());
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
      clearOverlays();
      const scenePayload = currentScenePayload();
      const nodes = Array.isArray(scenePayload && scenePayload.node) ? scenePayload.node : [];
      if (!nodes.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "No scene3d payload is available.";
        surface.appendChild(empty);
        renderOverlays();
        return;
      }
      void syncRendererScene();
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
    window.addEventListener("resize", () => {
      resizeCanvas();
    });
    window.addEventListener("beforeunload", () => {
      if (rendererHandle) {
        dispose(rendererHandle);
      }
    });
    void initializeRenderer();
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

export function __testGetTrustTwinPanelHtmlForPlaywright(
  extensionRoot: string,
  cspSource = "file:",
): string {
  const extensionUri = vscode.Uri.file(extensionRoot);
  const webview = {
    cspSource,
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
    "media/trust-twin/components/ur10/visual/base.gltf",
    "media/trust-twin/components/ur10/visual/base.bin",
    "media/trust-twin/components/ur10/visual/shoulder.gltf",
    "media/trust-twin/components/ur10/visual/shoulder.bin",
    "media/trust-twin/components/ur10/visual/upperarm.gltf",
    "media/trust-twin/components/ur10/visual/upperarm.bin",
    "media/trust-twin/components/ur10/visual/forearm.gltf",
    "media/trust-twin/components/ur10/visual/forearm.bin",
    "media/trust-twin/components/ur10/visual/wrist1.gltf",
    "media/trust-twin/components/ur10/visual/wrist1.bin",
    "media/trust-twin/components/ur10/visual/wrist2.gltf",
    "media/trust-twin/components/ur10/visual/wrist2.bin",
    "media/trust-twin/components/ur10/visual/wrist3.gltf",
    "media/trust-twin/components/ur10/visual/wrist3.bin",
    "media/trust-twin/components/schunk-wsg50/meshes/wsg_body.gltf",
    "media/trust-twin/components/schunk-wsg50/meshes/wsg_body.bin",
    "media/trust-twin/components/schunk-wsg50/meshes/finger_with_tip.gltf",
    "media/trust-twin/components/schunk-wsg50/meshes/finger_with_tip.bin",
    "media/trust-twin/components/ycb/meshes/003_cracker_box_textured.gltf",
    "media/trust-twin/components/ycb/meshes/003_cracker_box_textured.bin",
  ];
  const assets = required.filter((relativePath) =>
    fs.existsSync(path.join(extensionRoot, relativePath)),
  );
  const missing = required.filter((relativePath) => !assets.includes(relativePath));
  return { ok: missing.length === 0, assets, missing };
}
