import * as vscode from "vscode";

import { openAdsPanelForAction, type AdsPanelAction } from "../adsPanel";
import type { AdsStatusReport } from "../adsStatusSummary";
import { openRuntimePane, resolveRuntimeTarget, type RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  buildCommunicationCards,
  statusLabel,
  type CommCapabilitiesResponse,
  type CommunicationCardModel,
} from "./capability";
import { renderExternalCard } from "./cards/externalCard";
import { renderIoDriverCard } from "./cards/ioDriverCard";
import { renderRuntimeMeshCard } from "./cards/runtimeMeshCard";
import { type CardRenderOptions, renderSetupCard } from "./cards/shared";
import { renderTelemetryCard } from "./cards/telemetryCard";
import {
  COMMUNICATION_GROUPS,
  COMMUNICATION_PROTOCOLS,
  type CommunicationGroup,
} from "./communicationProtocols";
import { escapeAttribute, escapeHtml } from "./html";
import {
  applyCommSetup,
  blockedApplyResult,
  clientErrorResult,
  fetchCommSchema,
  normalizeProtocolId,
  testCommSetup,
} from "./runtimeComm";
import {
  renderSchemaForm,
  schemaFormClientScript,
  type CommApplyResponse,
  type CommSchemaResponse,
} from "./schemaForm";

export const COMMUNICATION_COMMAND = "trust-lsp.communication.openPanel";
const COMMUNICATION_DOCS_REPOSITORY =
  "https://github.com/johannesPettersson80/trust-platform/blob/main/";
const COMMUNICATION_DOCS_PATHS = new Set(
  COMMUNICATION_PROTOCOLS.map((protocol) => protocol.docsPath)
);

const COMMUNICATION_VIEW_TYPE = "trust-communication";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let focusedAdsAction: AdsPanelAction | undefined;
let activeProtocol: string | undefined;
let activeSchema: CommSchemaResponse | undefined;
let lastApplyResult: CommApplyResponse | undefined;

export interface CommunicationPanelModel {
  runtime: RuntimeTarget;
  cards: CommunicationCardModel[];
  capabilitiesError?: string;
  focusedAdsAction?: AdsPanelAction;
  schema?: CommSchemaResponse;
  activeProtocol?: string;
  applyResult?: CommApplyResponse;
  adsStatus?: AdsStatusReport;
}

export function registerCommunicationPanel(context: vscode.ExtensionContext): void {
  extensionContext = context;
  context.subscriptions.push(
    vscode.commands.registerCommand(
      COMMUNICATION_COMMAND,
      async (options?: { adsAction?: AdsPanelAction }) => {
        await showCommunicationPanel(context, options);
      }
    )
  );
}

export async function openCommunicationPanelForAdsAction(
  adsAction: AdsPanelAction
): Promise<void> {
  if (!extensionContext) {
    throw new Error("Communication panel has not been registered.");
  }
  await showCommunicationPanel(extensionContext, { adsAction });
}

export function buildCommunicationPanelModel(
  runtime: RuntimeTarget,
  capabilities?: CommCapabilitiesResponse,
  capabilitiesError?: string,
  adsAction?: AdsPanelAction,
  schema?: CommSchemaResponse,
  selectedProtocol?: string,
  applyResult?: CommApplyResponse,
  adsStatus?: AdsStatusReport
): CommunicationPanelModel {
  return {
    runtime,
    cards: buildCommunicationCards(runtime, capabilities, capabilitiesError, adsStatus),
    capabilitiesError,
    focusedAdsAction: adsAction,
    schema,
    activeProtocol: selectedProtocol,
    applyResult,
    adsStatus,
  };
}

export function renderCommunicationPanelHtml(
  model: CommunicationPanelModel,
  nonce = "communication-panel"
): string {
  const groupedCards = COMMUNICATION_GROUPS.map((group) => ({
    ...group,
    cards: model.cards.filter((card) => card.protocol.group === group.id),
  })).filter((group) => group.cards.length > 0);
  const statusSummary = model.cards
    .filter((card) => card.protocol.id !== "enterprise")
    .map(
      (card) =>
        `<span class="status-chip ${escapeAttribute(card.status)}">${escapeHtml(card.protocol.title)}: ${escapeHtml(statusLabel(card.status))}</span>`
    )
    .join("");
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta
    http-equiv="Content-Security-Policy"
    content="default-src 'none'; style-src 'nonce-${escapeAttribute(nonce)}'; script-src 'nonce-${escapeAttribute(nonce)}';"
  />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Communication</title>
  <style nonce="${escapeAttribute(nonce)}">
    :root {
      color-scheme: light dark;
      --bg: var(--vscode-editor-background);
      --text: var(--vscode-editor-foreground);
      --muted: var(--vscode-descriptionForeground);
      --border: var(--vscode-panel-border);
      --panel: var(--vscode-sideBar-background);
      --button-bg: var(--vscode-button-background);
      --button-fg: var(--vscode-button-foreground);
      --button-hover: var(--vscode-button-hoverBackground);
      --error: var(--vscode-errorForeground);
      --warn: var(--vscode-editorWarning-foreground);
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 14px;
      color: var(--text);
      background: var(--bg);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    header {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: flex-start;
      border-bottom: 1px solid var(--border);
      padding-bottom: 12px;
      margin-bottom: 12px;
    }
    h1, h2, h3, p { margin: 0; }
    h1 { font-size: 18px; line-height: 1.25; }
    h2 { font-size: 14px; margin-bottom: 8px; }
    h3 { font-size: 13px; margin-bottom: 6px; }
    .muted { color: var(--muted); }
    .actions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
    button {
      border: none;
      background: var(--button-bg);
      color: var(--button-fg);
      padding: 5px 9px;
      border-radius: 3px;
      cursor: pointer;
    }
    button:hover { background: var(--button-hover); }
    button.secondary {
      background: transparent;
      color: var(--text);
      border: 1px solid var(--border);
    }
    .intent-grid, .card-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
      gap: 8px;
    }
    .intent, .card, .panel {
      border: 1px solid var(--border);
      background: var(--panel);
      padding: 10px;
      border-radius: 6px;
    }
    button.intent {
      display: block;
      width: 100%;
      text-align: left;
      color: var(--text);
      font: inherit;
      cursor: pointer;
    }
    button.intent:hover,
    button.intent:focus {
      border-color: var(--vscode-focusBorder);
      outline: 1px solid var(--vscode-focusBorder);
      outline-offset: 1px;
    }
    .intent-action {
      display: block;
      margin-top: 6px;
      color: var(--vscode-textLink-foreground);
    }
    .overview {
      display: flex;
      gap: 6px;
      flex-wrap: wrap;
      margin: 10px 0;
    }
    .status-chip, .pill {
      display: inline-flex;
      align-items: center;
      border: 1px solid var(--border);
      border-radius: 999px;
      padding: 2px 7px;
      color: var(--muted);
      white-space: nowrap;
    }
    .pill {
      font-weight: 600;
      background: color-mix(in srgb, var(--muted) 12%, transparent);
    }
    .connected { color: #2da44e; }
    .degraded, .simulate, .configured_policy { color: var(--warn); }
    .error, .runtime_unreachable { color: var(--error); }
    .not_in_build, .not_configured { color: var(--muted); }
    .pill.connected,
    .status-chip.connected {
      border-color: #2da44e;
      background: rgba(45, 164, 78, 0.14);
      color: #2da44e;
    }
    .pill.degraded,
    .pill.simulate,
    .pill.configured_policy,
    .status-chip.degraded,
    .status-chip.simulate,
    .status-chip.configured_policy {
      border-color: var(--warn);
      background: color-mix(in srgb, var(--warn) 16%, transparent);
      color: var(--warn);
    }
    .pill.error,
    .pill.runtime_unreachable,
    .status-chip.error,
    .status-chip.runtime_unreachable {
      border-color: var(--error);
      background: color-mix(in srgb, var(--error) 14%, transparent);
      color: var(--error);
    }
    .group { margin-top: 14px; }
    .group.focused-group {
      outline: 1px solid var(--vscode-focusBorder);
      outline-offset: 4px;
    }
    h4 { margin: 8px 0; font-size: 13px; }
    .state-detail { margin-top: 6px; }
    .next-step {
      margin-top: 6px;
      color: var(--vscode-textLink-foreground);
    }
    .requirements {
      margin: 6px 0 0;
      padding: 0;
      color: var(--muted);
      list-style: none;
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
    }
    .requirements li {
      border: 1px solid var(--border);
      border-radius: 999px;
      padding: 2px 6px;
      white-space: nowrap;
    }
    .card.focused {
      outline: 2px solid var(--vscode-focusBorder);
      outline-offset: 2px;
    }
    .card.active-protocol {
      outline: 1px solid var(--vscode-focusBorder);
      outline-offset: 2px;
    }
    .card[data-status="error"],
    .card[data-status="runtime_unreachable"] {
      border-left: 4px solid var(--error);
    }
    .card[data-status="degraded"],
    .card[data-status="simulate"],
    .card[data-status="configured_policy"] {
      border-left: 4px solid var(--warn);
    }
    .active-setup {
      margin-top: 8px;
    }
    .active-setup .schema-form {
      margin-top: 0;
      border-top: none;
      padding-top: 0;
    }
    .active-setup .form-grid {
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    }
    .schema-form {
      margin-top: 10px;
      border-top: 1px solid var(--border);
      padding-top: 10px;
    }
    .form-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 8px;
    }
    .field {
      display: flex;
      flex-direction: column;
      gap: 4px;
      min-width: 0;
    }
    .field input, .field select, .field textarea {
      width: 100%;
      min-width: 0;
      background: var(--vscode-input-background);
      color: var(--vscode-input-foreground);
      border: 1px solid var(--vscode-input-border, var(--border));
      padding: 4px 6px;
      border-radius: 3px;
      font: inherit;
    }
    .field textarea {
      min-height: 80px;
      resize: vertical;
      font-family: var(--vscode-editor-font-family);
    }
    .field small, .field-error, .apply-result {
      color: var(--muted);
      line-height: 1.35;
    }
    .field-error { color: var(--error); }
    .apply-result {
      margin: 0 0 8px;
      padding: 6px 8px;
      border-left: 3px solid currentColor;
      background: color-mix(in srgb, currentColor 8%, transparent);
    }
    .apply-result.pending {
      color: var(--muted);
      border-left-color: var(--border);
      background: transparent;
    }
    code { color: var(--vscode-textPreformat-foreground); }
  </style>
</head>
<body>
  <header>
    <div>
      <h1>Communication</h1>
      <p class="muted">Using runtime: ${escapeHtml(model.runtime.label)}</p>
    </div>
    <div class="actions">
      <button data-action="refresh">Refresh</button>
      <button class="secondary" data-action="openRuntimePane">Open Runtime pane</button>
    </div>
  </header>

  <section class="panel">
    <h2>Which communication do I need?</h2>
    <div class="intent-grid">
      <button type="button" class="intent" data-group-jump="runtime"><strong>Another truST runtime</strong><p class="muted">Use Discovery, Mesh / Zenoh, Realtime T0, or federation policy.</p><span class="intent-action">Show runtime options</span></button>
      <button type="button" class="intent" data-group-jump="external"><strong>External software or plant system</strong><p class="muted">Use ADS, OPC UA, Modbus TCP, or MQTT.</p><span class="intent-action">Show external options</span></button>
      <button type="button" class="intent" data-group-jump="fieldbus"><strong>Local hardware or fieldbus</strong><p class="muted">Use EtherCAT, GPIO, simulated, or loopback I/O.</p><span class="intent-action">Show hardware options</span></button>
    </div>
  </section>

  <section>
    <div class="overview">${statusSummary}</div>
  </section>

  ${model.capabilitiesError ? `<p class="panel error">${escapeHtml(model.capabilitiesError)}</p>` : ""}

  ${groupedCards.map((group) => renderGroup(group.id, group.title, group.purpose, group.cards, model.focusedAdsAction, model.schema, model.activeProtocol, model.applyResult)).join("")}

  <script nonce="${escapeAttribute(nonce)}">
    const vscode = acquireVsCodeApi();
    document.addEventListener("click", (event) => {
      const groupJump = event.target.closest("[data-group-jump]");
      if (groupJump) {
        const target = Array.from(document.querySelectorAll("[data-group]")).find((group) => group.dataset.group === groupJump.dataset.groupJump);
        if (target) {
          target.scrollIntoView({ behavior: "smooth", block: "start" });
          target.classList.add("focused-group");
          window.setTimeout(() => target.classList.remove("focused-group"), 1200);
        }
        return;
      }
      const button = event.target.closest("button[data-action]");
      if (!button || button.disabled) return;
      vscode.postMessage({
        type: button.dataset.action,
        protocol: button.dataset.protocol,
        adsAction: button.dataset.adsAction,
        docsPath: button.dataset.docsPath,
      });
    });
    ${schemaFormClientScript()}
  </script>
</body>
</html>`;
}

async function showCommunicationPanel(
  context: vscode.ExtensionContext,
  options?: { adsAction?: AdsPanelAction }
): Promise<void> {
  focusedAdsAction = options?.adsAction;
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
  } else {
    panel = vscode.window.createWebviewPanel(
      COMMUNICATION_VIEW_TYPE,
      "Structured Text: Communication",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      }
    );
    panel.onDidDispose(() => {
      panel = undefined;
    });
    panel.onDidChangeViewState((event) => {
      if (event.webviewPanel.visible) {
        void refreshCommunicationPanel();
      }
    });
    panel.webview.onDidReceiveMessage((message: unknown) => {
      void handleWebviewMessage(context, message);
    });
    context.subscriptions.push(panel);
  }
  await refreshCommunicationPanel();
}

async function refreshCommunicationPanel(): Promise<void> {
  if (!panel) return;
  const runtime = await resolveRuntimeTarget();
  let capabilities: CommCapabilitiesResponse | undefined;
  let capabilitiesError: string | undefined;
  let adsStatus: AdsStatusReport | undefined;
  activeSchema = undefined;
  if (runtime.status === "online_reachable" && runtime.endpoint) {
    try {
      capabilities = await sendRuntimeControlRequest<CommCapabilitiesResponse>(
        runtime.endpoint,
        runtime.authToken,
        "comm.capabilities",
        undefined,
        { timeoutMs: 2000 }
      );
    } catch (error) {
      capabilitiesError = error instanceof Error ? error.message : String(error);
    }
    try {
      adsStatus = await sendRuntimeControlRequest<AdsStatusReport>(
        runtime.endpoint,
        runtime.authToken,
        "ads.status",
        undefined,
        { timeoutMs: 2000 }
      );
    } catch {
      adsStatus = undefined;
    }
    if (activeProtocol && activeProtocol !== "ads") {
      try {
        activeSchema = await fetchCommSchema(runtime, activeProtocol);
      } catch (error) {
        lastApplyResult = {
          schema_version: 1,
          protocol: activeProtocol,
          driver: "",
          action: "schema",
          applied: false,
          lifecycle_effect: "blocked",
          message: error instanceof Error ? error.message : String(error),
          field_errors: [],
        };
      }
    }
  }
  panel.webview.html = renderCommunicationPanelHtml(
    buildCommunicationPanelModel(
      runtime,
      capabilities,
      capabilitiesError,
      focusedAdsAction,
      activeSchema,
      activeProtocol,
      lastApplyResult,
      adsStatus
    ),
    nonce()
  );
}

async function handleWebviewMessage(
  context: vscode.ExtensionContext,
  message: unknown
): Promise<void> {
  if (!isRecord(message)) return;
  switch (message.type) {
    case "refresh":
      await refreshCommunicationPanel();
      break;
    case "openRuntimePane":
      await openRuntimePane();
      break;
    case "adsWorkflow":
      await openAdsPanelForAction(normalizeAdsAction(message.adsAction));
      break;
    case "setupProtocol":
      activeProtocol = normalizeProtocolId(message.protocol);
      lastApplyResult = undefined;
      await refreshCommunicationPanel();
      break;
    case "commApply":
      await applyCommunicationSetup(message);
      break;
    case "commTest":
      await testCommunicationSetup(message);
      break;
    case "commApplyClientError":
      {
        const result = clientErrorResult(message.protocol, message.fieldErrors);
        if (result) {
          activeProtocol = result.protocol;
          lastApplyResult = result.applyResult;
          await refreshCommunicationPanel();
        }
      }
      break;
    case "openDocs":
      await openDocs(context, String(message.docsPath ?? ""));
      break;
  }
}

async function applyCommunicationSetup(message: Record<string, unknown>): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  const result = await applyCommSetup(runtime, message, activeSchema);
  if (!result) {
    return;
  }
  activeProtocol = result.protocol;
  activeSchema = result.schema ?? activeSchema;
  lastApplyResult = result.applyResult;
  await refreshCommunicationPanel();
}

async function testCommunicationSetup(message: Record<string, unknown>): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  const result = await testCommSetup(runtime, message, activeSchema);
  if (!result) {
    return;
  }
  activeProtocol = result.protocol;
  activeSchema = result.schema ?? activeSchema;
  lastApplyResult = result.applyResult;
  await refreshCommunicationPanel();
}

function renderGroup(
  id: CommunicationGroup,
  title: string,
  purpose: string,
  cards: CommunicationCardModel[],
  focusedAdsAction: AdsPanelAction | undefined,
  schema: CommSchemaResponse | undefined,
  activeProtocolId: string | undefined,
  applyResult: CommApplyResponse | undefined
): string {
  const options: CardRenderOptions = {
    schema,
    activeProtocolId,
    applyResult,
  };
  const activeProtocolSchema = activeProtocolId
    ? schema?.protocols.find((protocol) => protocol.id === activeProtocolId)
    : undefined;
  const activeSetup =
    activeProtocolSchema && cards.some((card) => card.protocol.id === activeProtocolId)
      ? `<div class="active-setup panel" data-active-setup="${escapeAttribute(activeProtocolId)}">${renderSchemaForm(activeProtocolSchema, applyResult)}</div>`
      : "";
  return `<section class="group" data-group="${escapeAttribute(id)}">
    <h2>${escapeHtml(title)}</h2>
    <p class="muted">${escapeHtml(purpose)}</p>
    <div class="card-grid">
      ${cards.map((card) => renderCardForGroup(id, card, focusedAdsAction, options)).join("")}
    </div>
    ${activeSetup}
  </section>`;
}

function renderCardForGroup(
  group: CommunicationGroup,
  card: CommunicationCardModel,
  focusedAdsAction: AdsPanelAction | undefined,
  options: CardRenderOptions
): string {
  switch (group) {
    case "external":
      return renderExternalCard(card, focusedAdsAction, options);
    case "runtime":
      return renderRuntimeMeshCard(card, options);
    case "fieldbus":
      return renderIoDriverCard(card, options);
    case "telemetry":
      return renderTelemetryCard(card, options);
    case "enterprise":
      return renderSetupCard(card, options);
  }
}

async function openDocs(
  context: vscode.ExtensionContext,
  docsPath: string
): Promise<void> {
  const uri = await resolveCommunicationDocsUri(
    docsPath,
    communicationDocsRoots(context)
  );
  if (!uri) return;
  if (uri.scheme === "http" || uri.scheme === "https") {
    await vscode.env.openExternal(uri);
    return;
  }
  await vscode.commands.executeCommand("vscode.open", uri);
}

export async function resolveCommunicationDocsUri(
  docsPath: string,
  roots: readonly vscode.Uri[]
): Promise<vscode.Uri | undefined> {
  const normalized = normalizeCommunicationDocsPath(docsPath);
  if (!normalized) {
    return undefined;
  }
  for (const root of roots) {
    const candidate = uriForRelativePath(root, normalized);
    if (await uriExists(candidate)) {
      return candidate;
    }
  }
  return vscode.Uri.parse(
    `${COMMUNICATION_DOCS_REPOSITORY}${encodeURI(normalized)}`
  );
}

function communicationDocsRoots(context: vscode.ExtensionContext): vscode.Uri[] {
  return [
    vscode.Uri.joinPath(context.extensionUri, "..", ".."),
    context.extensionUri,
  ];
}

function normalizeCommunicationDocsPath(docsPath: string): string | undefined {
  const normalized = docsPath.trim().replace(/\\/g, "/").replace(/^\/+/, "");
  if (!COMMUNICATION_DOCS_PATHS.has(normalized)) {
    return undefined;
  }
  return normalized;
}

function uriForRelativePath(root: vscode.Uri, relativePath: string): vscode.Uri {
  return vscode.Uri.joinPath(root, ...relativePath.split("/"));
}

async function uriExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

function normalizeAdsAction(value: unknown): AdsPanelAction {
  switch (value) {
    case "addDevice":
    case "diagnose":
    case "importSymbols":
    case "addRoute":
    case "serverStatus":
    case "status":
      return value;
    default:
      return "status";
  }
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
