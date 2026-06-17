import * as vscode from "vscode";
import { execFile, spawn } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { getBinaryPath } from "./binary";
import {
  summarizeAdsStatus,
  type AdsStatusReport,
} from "./adsStatusSummary";
import {
  openRuntimePane,
  resolveRuntimeTarget,
  type RuntimeTarget,
} from "./runtimeTarget";
import { sendRuntimeControlRequest } from "./runtimeControlClient";

export const ADS_COMMANDS = {
  openPanel: "trust-lsp.ads.openPanel",
  openServerPanel: "trust-lsp.ads.server.openPanel",
  addDevice: "trust-lsp.ads.addDevice",
  diagnose: "trust-lsp.ads.diagnose",
  importSymbols: "trust-lsp.ads.importSymbols",
  addRoute: "trust-lsp.ads.addRoute",
} as const;

export type AdsPanelAction =
  | "status"
  | "addDevice"
  | "diagnose"
  | "importSymbols"
  | "addRoute"
  | "serverStatus";

export interface AdsPanelRegistrationOptions {
  openCommunicationPanel?: (activeAction: AdsPanelAction) => Promise<unknown> | unknown;
}

export interface AdsPanelModel {
  runtime: RuntimeTarget;
  activeAction: AdsPanelAction;
  status?: AdsStatusReport;
  statusError?: string;
  serverStatus?: AdsServerStatusSurface;
  serverStatusError?: string;
  productionActionsEnabled: boolean;
  productionBlockedReason: string;
  credentialForwardingAllowed: boolean;
  localCliRouteAddAvailable: boolean;
  credentialWarning: string;
  setupUrl?: string;
  connectionSummary: string;
  serverSummary: string;
  authoringOnlyAvailable: boolean;
  authoringOnlyBadge: string;
}

export interface AdsServerStatusSurface {
  schema_version?: number;
  role?: string;
  status?: AdsStatusReport;
  identity?: AdsServerIdentity;
  enabled?: boolean;
  listen?: string;
  ams_net_id?: string;
  ads_port?: number;
  exposed_count?: number;
  writable_count?: number;
  allowed_client_count?: number;
  connected_clients?: number | null;
  recently_refused_clients?: AdsServerPendingClient[];
  pending_clients?: AdsServerPendingClient[];
  discoverable?: boolean;
  external_client_verified?: boolean;
  configured_empty?: boolean;
  proof_status?: string;
}

interface AdsServerIdentity {
  host_name?: string;
  chosen_ip?: string;
  ams_net_id?: string;
  classification?: string;
}

interface AdsServerPendingClient {
  ams_net_id?: string;
  source_ip?: string;
  reason?: string;
  count?: number;
  last_seen_ms?: number;
  suggested_client?: {
    ams_net_id?: string;
    source_ip?: string;
  };
}

interface AdsGeneratedFilePreview {
  path: string;
  kind: string;
  content: string;
  bytes: number;
  exists: boolean;
  changed: boolean;
}

interface AdsImportSymbolsCliReport {
  connection_name: string;
  selected_count: number;
  dry_run: boolean;
  previews: AdsGeneratedFilePreview[];
}

interface AdsLocalIdentity {
  host_name?: string;
  chosen_ip: string;
  ams_net_id: string;
  nic?: string;
  classification: string;
}

interface AdsAddRouteCliReport {
  route_name: string;
  target_ip: string;
  target_net_id: string;
  local_ip: string;
  local_net_id: string;
  status: string;
}

export interface AdsAddRouteCliArgsInput {
  routeName: string;
  targetIp: string;
  targetNetId: string;
  amsPort: number;
  localIp: string;
  localNetId: string;
  username: string;
}

const ADS_PANEL_VIEW_TYPE = "trust-ads-devices";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;

export function registerAdsPanel(
  context: vscode.ExtensionContext,
  options: AdsPanelRegistrationOptions = {}
): void {
  extensionContext = context;
  const openAdsEntry = async (activeAction: AdsPanelAction): Promise<void> => {
    if (options.openCommunicationPanel) {
      await options.openCommunicationPanel(activeAction);
      return;
    }
    await showAdsPanel(context, activeAction);
  };
  context.subscriptions.push(
    vscode.commands.registerCommand(ADS_COMMANDS.openPanel, async () => {
      await openAdsEntry("status");
    }),
    vscode.commands.registerCommand(ADS_COMMANDS.openServerPanel, async () => {
      await openAdsEntry("serverStatus");
    }),
    vscode.commands.registerCommand(ADS_COMMANDS.addDevice, async () => {
      await openAdsEntry("addDevice");
    }),
    vscode.commands.registerCommand(ADS_COMMANDS.diagnose, async () => {
      await openAdsEntry("diagnose");
    }),
    vscode.commands.registerCommand(ADS_COMMANDS.importSymbols, async () => {
      await openAdsEntry("importSymbols");
    }),
    vscode.commands.registerCommand(ADS_COMMANDS.addRoute, async () => {
      await openAdsEntry("addRoute");
    })
  );
}

export function buildAdsPanelModel(
  runtime: RuntimeTarget,
  status: AdsStatusReport | undefined,
  activeAction: AdsPanelAction,
  statusError?: string,
  serverStatus?: AdsServerStatusSurface,
  serverStatusError?: string
): AdsPanelModel {
  const productionBlockedReason = blockedReason(runtime);
  const productionActionsEnabled = productionBlockedReason.length === 0;
  const credentialForwardingAllowed =
    runtime.credentialChannel === "trusted_same_host";
  const localCliRouteAddAvailable =
    runtime.credentialChannel === "untrusted_remote_plain_tcp" &&
    productionActionsEnabled;
  return {
    runtime,
    activeAction,
    status,
    statusError,
    serverStatus,
    serverStatusError,
    productionActionsEnabled,
    productionBlockedReason,
    credentialForwardingAllowed,
    localCliRouteAddAvailable,
    credentialWarning: credentialWarning(runtime),
    setupUrl: runtime.setupUrl,
    connectionSummary: summarizeAdsStatus(status).text,
    serverSummary: summarizeAdsServerStatus(serverStatus, serverStatusError),
    authoringOnlyAvailable: true,
    authoringOnlyBadge:
      "Authoring only: imports symbols from this computer and never grants production-ready.",
  };
}

export function renderAdsPanelHtml(model: AdsPanelModel, nonce = "ads-panel"): string {
  const actionTitle = activeActionTitle(model.activeAction);
  const setupDisabled = model.setupUrl ? "" : " disabled";
  const productionDisabled = model.productionActionsEnabled ? "" : " disabled";
  const addRouteAction = model.localCliRouteAddAvailable
    ? "addRouteLocalCli"
    : "addRoute";
  const addRouteLabel = model.localCliRouteAddAvailable
    ? "Add route from this computer"
    : "Add route";
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta
    http-equiv="Content-Security-Policy"
    content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${escapeAttribute(nonce)}';"
  />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>ADS Devices</title>
  <style>
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
      padding: 12px;
      color: var(--text);
      background: var(--bg);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 12px;
      border-bottom: 1px solid var(--border);
      padding-bottom: 10px;
    }
    h1, h2, p { margin: 0; }
    h1 { font-size: 18px; line-height: 1.2; }
    h2 { font-size: 13px; margin-bottom: 8px; }
    .muted { color: var(--muted); }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 10px;
    }
    .panel {
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 10px;
      background: var(--panel);
      min-width: 0;
    }
    .facts {
      display: grid;
      grid-template-columns: 112px minmax(0, 1fr);
      gap: 6px 10px;
    }
    .facts dt {
      color: var(--muted);
      font-weight: 700;
    }
    .facts dd {
      margin: 0;
      overflow-wrap: anywhere;
    }
    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 10px;
    }
    button {
      border: 0;
      border-radius: 4px;
      padding: 6px 10px;
      color: var(--button-fg);
      background: var(--button-bg);
      font: inherit;
      font-weight: 700;
      cursor: pointer;
    }
    button:hover { background: var(--button-hover); }
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    .status {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      border: 1px solid var(--border);
      border-radius: 999px;
      padding: 3px 8px;
      white-space: nowrap;
    }
    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--muted);
    }
    .dot.healthy { background: #2ea043; }
    .dot.degraded, .dot.unknown, .dot.disabled { background: var(--warn); }
    .dot.faulted { background: var(--error); }
    .notice {
      margin-top: 8px;
      color: var(--muted);
      line-height: 1.4;
    }
    .blocked { color: var(--warn); }
    .badge {
      display: inline-block;
      margin-top: 8px;
      border: 1px solid var(--warn);
      border-radius: 999px;
      padding: 3px 8px;
      color: var(--warn);
      font-weight: 700;
    }
    .connections {
      display: grid;
      gap: 8px;
    }
    .connection {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 8px;
      border-top: 1px solid var(--border);
      padding-top: 8px;
    }
    .connection:first-child { border-top: 0; padding-top: 0; }
    .connection strong,
    .connection span {
      overflow-wrap: anywhere;
    }
    .connection pre {
      margin: 6px 0 0;
      max-width: 100%;
      overflow-x: auto;
      border: 1px solid var(--border);
      border-radius: 4px;
      padding: 6px;
      background: var(--bg);
    }
    .stacked-actions {
      display: grid;
      gap: 6px;
      justify-items: end;
      align-content: start;
    }
    @media (max-width: 520px) {
      body { padding: 8px; }
      header { flex-direction: column; }
      .facts { grid-template-columns: 1fr; }
      .connection { grid-template-columns: 1fr; }
      .stacked-actions { justify-items: start; }
    }
  </style>
</head>
<body>
  <header>
    <div>
      <p class="muted">Selected runtime</p>
      <h1>Beckhoff ADS Devices</h1>
      <p class="muted">${escapeHtml(actionTitle)}</p>
    </div>
    <div class="status" title="${escapeAttribute(model.connectionSummary)}">
      <span class="dot ${escapeAttribute(statusClass(model.status?.overall))}"></span>
      <span>${escapeHtml(model.connectionSummary)}</span>
    </div>
  </header>

  <section class="grid" aria-label="ADS runtime context">
    <section class="panel">
      <h2>Runtime</h2>
      <dl class="facts">
        <dt>Name</dt><dd>${escapeHtml(model.runtime.label)}</dd>
        <dt>Mode</dt><dd>${escapeHtml(model.runtime.mode)}</dd>
        <dt>State</dt><dd>${escapeHtml(model.runtime.status.replace(/_/g, " "))}</dd>
        <dt>Endpoint</dt><dd>${escapeHtml(model.runtime.endpoint ?? "not configured")}</dd>
        <dt>Credentials</dt><dd>${escapeHtml(model.runtime.credentialChannel.replace(/_/g, " "))}</dd>
      </dl>
      ${
        model.productionActionsEnabled
          ? ""
          : `<p class="notice blocked">${escapeHtml(model.productionBlockedReason)}</p>`
      }
      <div class="actions">
        <button data-action="openRuntimePane">Open Runtime pane</button>
        <button data-action="refresh">Refresh</button>
      </div>
    </section>

    <section class="panel">
      <h2>Onboarding</h2>
      <p class="notice">${escapeHtml(model.credentialWarning)}</p>
      <span class="badge">${escapeHtml(model.authoringOnlyBadge)}</span>
      <div class="actions">
        <button data-action="openSetup"${setupDisabled}>Open setup on runtime host</button>
        <button data-action="diagnose"${productionDisabled}>Diagnose</button>
        <button data-action="importSymbols"${productionDisabled}>Import symbols</button>
        <button data-action="${escapeAttribute(addRouteAction)}"${productionDisabled}>${escapeHtml(addRouteLabel)}</button>
        <button data-action="authoringImport">Preview import (authoring only)</button>
      </div>
      <p class="notice">Authoring-only import runs the local CLI in dry-run mode, opens file diffs, and writes only after confirmation.</p>
    </section>
  </section>

  <section class="panel" style="margin-top: 10px">
    <h2>ADS Status</h2>
    ${renderStatusBody(model)}
  </section>

  <section class="panel" style="margin-top: 10px">
    <h2>ADS Server</h2>
    ${renderServerStatusBody(model)}
  </section>

  <section class="panel" style="margin-top: 10px">
    <h2>Deploy</h2>
    <p class="notice">Production-ready evidence is granted only after generated files are deployed/reloaded on the selected runtime and <code>ads.status</code> reports the deployed ADS worker healthy.</p>
    <div class="actions">
      <button data-action="openRuntimePane">Open Runtime pane</button>
      <button data-action="openSetup"${setupDisabled}>Open runtime setup</button>
    </div>
  </section>

  <script nonce="${escapeAttribute(nonce)}">
    const vscode = acquireVsCodeApi();
    document.addEventListener("click", (event) => {
      const button = event.target.closest("button[data-action]");
      if (!button || button.disabled) {
        return;
      }
      vscode.postMessage({
        type: button.dataset.action,
        clientToml: button.dataset.clientToml,
      });
    });
  </script>
</body>
</html>`;
}

async function showAdsPanel(
  context: vscode.ExtensionContext,
  activeAction: AdsPanelAction
): Promise<void> {
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
  } else {
    panel = vscode.window.createWebviewPanel(
      ADS_PANEL_VIEW_TYPE,
      "Structured Text: ADS Devices",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      }
    );
    panel.onDidDispose(() => {
      panel = undefined;
    });
    panel.webview.onDidReceiveMessage((message: unknown) => {
      void handleWebviewMessage(message, activeAction);
    });
    context.subscriptions.push(panel);
  }
  await refreshAdsPanel(activeAction);
}

async function refreshAdsPanel(activeAction: AdsPanelAction): Promise<void> {
  if (!panel) {
    return;
  }
  const runtime = await resolveRuntimeTarget();
  let status: AdsStatusReport | undefined;
  let statusError: string | undefined;
  let serverStatus: AdsServerStatusSurface | undefined;
  let serverStatusError: string | undefined;
  if (runtime.status === "online_reachable" && runtime.endpoint) {
    try {
      status = await sendRuntimeControlRequest<AdsStatusReport>(
        runtime.endpoint,
        runtime.authToken,
        "ads.status",
        undefined,
        { timeoutMs: 2000 }
      );
    } catch (error) {
      statusError = error instanceof Error ? error.message : String(error);
    }
    try {
      serverStatus = await sendRuntimeControlRequest<AdsServerStatusSurface>(
        runtime.endpoint,
        runtime.authToken,
        "ads.server.status",
        undefined,
        { timeoutMs: 2000 }
      );
    } catch (error) {
      serverStatusError = error instanceof Error ? error.message : String(error);
    }
  }
  panel.webview.html = renderAdsPanelHtml(
    buildAdsPanelModel(
      runtime,
      status,
      activeAction,
      statusError,
      serverStatus,
      serverStatusError
    ),
    nonce()
  );
}

async function handleWebviewMessage(
  message: unknown,
  activeAction: AdsPanelAction
): Promise<void> {
  if (!isRecord(message) || typeof message.type !== "string") {
    return;
  }
  switch (message.type) {
    case "openRuntimePane":
      await openRuntimePane();
      break;
    case "refresh":
      await refreshAdsPanel(activeAction);
      break;
    case "openSetup":
      await openSetupUrl();
      break;
    case "authoringImport":
      await runAuthoringOnlyImport();
      break;
    case "addRouteLocalCli":
      await runRemotePlainTcpRouteAdd();
      break;
    case "serverDoctor":
      await startAdsServerDoctor();
      break;
    case "copyServerClient":
      await copyServerClientSnippet(message.clientToml);
      break;
    case "diagnose":
    case "importSymbols":
    case "addRoute":
      await openSetupUrl();
      break;
    default:
      break;
  }
}

async function startAdsServerDoctor(): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  if (!runtime.endpoint || runtime.status !== "online_reachable") {
    void vscode.window.showWarningMessage(
      "Select a reachable online runtime before running the ADS server Doctor."
    );
    return;
  }
  try {
    const result = await sendRuntimeControlRequest<{ job_id?: string }>(
      runtime.endpoint,
      runtime.authToken,
      "ads.server.doctor.start",
      {},
      { timeoutMs: 3000 }
    );
    const suffix = result.job_id ? ` Job: ${result.job_id}.` : "";
    void vscode.window.showInformationMessage(
      `ADS server Doctor started on the selected runtime.${suffix}`
    );
    await refreshAdsPanel("serverStatus");
  } catch (error) {
    void vscode.window.showErrorMessage(
      `ADS server Doctor failed to start: ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

async function copyServerClientSnippet(snippet: unknown): Promise<void> {
  if (typeof snippet !== "string" || snippet.trim().length === 0) {
    void vscode.window.showWarningMessage("No ADS server client allowlist entry is available.");
    return;
  }
  await vscode.env.clipboard.writeText(snippet);
  void vscode.window.showInformationMessage(
    "Copied ADS server allowlist entry. Add it to runtime.toml and redeploy before rerunning the server Doctor."
  );
}

async function runRemotePlainTcpRouteAdd(): Promise<void> {
  if (!extensionContext) {
    void vscode.window.showErrorMessage("ADS panel is not registered.");
    return;
  }
  const runtime = await resolveRuntimeTarget();
  if (runtime.credentialChannel !== "untrusted_remote_plain_tcp") {
    await openSetupUrl();
    return;
  }
  if (!runtime.endpoint || runtime.status !== "online_reachable") {
    void vscode.window.showWarningMessage(
      "Select a reachable online runtime before adding an ADS route."
    );
    return;
  }

  const targetIp = await promptRequiredInput(
    "Add ADS route from this computer",
    "TwinCAT target IP or hostname",
    "192.168.10.5"
  );
  if (!targetIp) {
    return;
  }
  const targetNetId = await promptRequiredInput(
    "Add ADS route from this computer",
    "TwinCAT target AMS Net ID",
    "5.23.91.12.1.1"
  );
  if (!targetNetId) {
    return;
  }
  const amsPort = await promptAmsPort();
  if (!amsPort) {
    return;
  }
  const routeName = await promptRequiredInput(
    "Add ADS route from this computer",
    "Route name to create on the TwinCAT target",
    defaultRouteName(runtime)
  );
  if (!routeName) {
    return;
  }

  let identity: AdsLocalIdentity;
  try {
    identity = await sendRuntimeControlRequest<AdsLocalIdentity>(
      runtime.endpoint,
      runtime.authToken,
      "ads.identity",
      { target_ip: targetIp },
      { timeoutMs: 3000 }
    );
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not resolve runtime-host ADS identity: ${error instanceof Error ? error.message : String(error)}`
    );
    return;
  }

  const username = await vscode.window.showInputBox({
    title: "Add ADS route from this computer",
    prompt: "TwinCAT user name",
    value: "Administrator",
    ignoreFocusOut: true,
    validateInput: (value) =>
      value.trim().length === 0 ? "TwinCAT user name is required." : undefined,
  });
  if (!username) {
    return;
  }

  const confirmation = await vscode.window.showWarningMessage(
    `Add ADS route '${routeName.trim()}' on ${targetIp.trim()} for runtime ${identity.chosen_ip}/${identity.ams_net_id}? TwinCAT credentials are sent directly from this computer to the PLC over ADS UDP 48899 and are not sent to the runtime host.`,
    { modal: true },
    "Add route"
  );
  if (confirmation !== "Add route") {
    return;
  }

  const password = await vscode.window.showInputBox({
    title: "Add ADS route from this computer",
    prompt: "TwinCAT password. It is sent directly from this computer to the PLC and is not stored.",
    password: true,
    ignoreFocusOut: true,
    validateInput: (value) =>
      value.length === 0 ? "TwinCAT password is required." : undefined,
  });
  if (password === undefined) {
    return;
  }

  const binary = getBinaryPath(
    extensionContext,
    "trust-runtime",
    "runtime.cli.path"
  );
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath
    ?? extensionContext.extensionPath;
  const args = buildAdsAddRouteCliArgs({
    routeName: routeName.trim(),
    targetIp: targetIp.trim(),
    targetNetId: targetNetId.trim(),
    amsPort,
    localIp: identity.chosen_ip,
    localNetId: identity.ams_net_id,
    username: username.trim(),
  });

  try {
    const report = await runRuntimeJsonCommandWithInput<AdsAddRouteCliReport>(
      binary,
      args,
      cwd,
      `${password}\n`
    );
    const choice = await vscode.window.showInformationMessage(
      `ADS route '${report.route_name}' added for runtime ${report.local_ip}/${report.local_net_id}. Run the runtime-host Doctor before marking the device production-ready.`,
      "Open setup on runtime host",
      "Refresh"
    );
    if (choice === "Open setup on runtime host") {
      await openSetupUrl();
    } else {
      await refreshAdsPanel("addRoute");
    }
  } catch (error) {
    void vscode.window.showErrorMessage(
      `ADS route add failed: ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

async function runAuthoringOnlyImport(): Promise<void> {
  if (!extensionContext) {
    void vscode.window.showErrorMessage("ADS panel is not registered.");
    return;
  }
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    void vscode.window.showWarningMessage(
      "Open a truST project folder before importing ADS symbols."
    );
    return;
  }
  const target = await vscode.window.showInputBox({
    title: "ADS authoring-only import",
    prompt: "TwinCAT target IP or hostname",
    placeHolder: "192.168.10.5",
    ignoreFocusOut: true,
    validateInput: (value) =>
      value.trim().length === 0 ? "Target IP or hostname is required." : undefined,
  });
  if (!target) {
    return;
  }
  const targetNetId = await vscode.window.showInputBox({
    title: "ADS authoring-only import",
    prompt: "TwinCAT target AMS Net ID. Leave empty to use UDP identify.",
    placeHolder: "5.23.91.12.1.1",
    ignoreFocusOut: true,
  });
  const connection = await vscode.window.showInputBox({
    title: "ADS authoring-only import",
    prompt: "Connection name for ads.toml",
    value: "line1",
    ignoreFocusOut: true,
    validateInput: (value) =>
      value.trim().length === 0 ? "Connection name is required." : undefined,
  });
  if (!connection) {
    return;
  }

  const binary = getBinaryPath(
    extensionContext,
    "trust-runtime",
    "runtime.cli.path"
  );
  const args = [
    "ads",
    "import-symbols",
    "--target",
    target.trim(),
  ];
  if (targetNetId?.trim()) {
    args.push("--target-net-id", targetNetId.trim());
  }
  args.push(
    "--connection",
    connection.trim(),
    "--out",
    "ads.toml",
    "--gen",
    path.join("src", "generated", "ads_generated.st"),
    "--dry-run",
    "--json"
  );
  try {
    const report = await runRuntimeJsonCommand<AdsImportSymbolsCliReport>(
      binary,
      args,
      workspaceFolder.uri.fsPath
    );
    const applied = await previewAndApplyImport(
      report,
      workspaceFolder.uri.fsPath
    );
    if (applied) {
      void vscode.window.showInformationMessage(
        "Applied ADS authoring files. Run the runtime-host Doctor before deploying."
      );
    }
  } catch (error) {
    void vscode.window.showErrorMessage(
      `ADS authoring-only import failed: ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

async function openSetupUrl(): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  if (!runtime.setupUrl) {
    void vscode.window.showWarningMessage(
      "No ADS setup URL is configured for the selected runtime."
    );
    return;
  }
  await vscode.env.openExternal(vscode.Uri.parse(runtime.setupUrl));
}

async function runRuntimeJsonCommand<T>(
  binary: string,
  args: string[],
  cwd: string
): Promise<T> {
  return await new Promise<T>((resolve, reject) => {
    execFile(
      binary,
      args,
      { cwd, maxBuffer: 64 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          const detail = [error.message, stderr.trim()]
            .filter((value) => value.length > 0)
            .join("\n");
          reject(new Error(detail));
          return;
        }
        try {
          resolve(JSON.parse(stdout) as T);
        } catch (parseError) {
          reject(
            parseError instanceof Error
              ? parseError
              : new Error(String(parseError))
          );
        }
      }
    );
  });
}

async function runRuntimeJsonCommandWithInput<T>(
  binary: string,
  args: string[],
  cwd: string,
  input: string
): Promise<T> {
  return await new Promise<T>((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error("trust-runtime command timed out"));
    }, 60_000);

    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timeout);
      if (code !== 0) {
        const detail = stderr.trim() || `trust-runtime exited with ${code}`;
        reject(new Error(detail));
        return;
      }
      try {
        resolve(JSON.parse(stdout) as T);
      } catch (parseError) {
        reject(
          parseError instanceof Error
            ? parseError
            : new Error(String(parseError))
        );
      }
    });
    child.stdin.end(input, "utf8");
  });
}

export function buildAdsAddRouteCliArgs(
  input: AdsAddRouteCliArgsInput
): string[] {
  return [
    "ads",
    "add-route",
    "--route-name",
    input.routeName,
    "--target",
    input.targetIp,
    "--target-net-id",
    input.targetNetId,
    "--ams-port",
    String(input.amsPort),
    "--local-ip",
    input.localIp,
    "--local-net-id",
    input.localNetId,
    "--username",
    input.username,
    "--password-stdin",
    "--json",
  ];
}

async function previewAndApplyImport(
  report: AdsImportSymbolsCliReport,
  workspaceRoot: string
): Promise<boolean> {
  const previews = Array.isArray(report.previews) ? report.previews : [];
  if (!report.dry_run || previews.length === 0) {
    throw new Error("ADS import dry-run did not return file previews.");
  }
  const previewRoot = await fs.promises.mkdtemp(
    path.join(os.tmpdir(), "trust-ads-preview-")
  );
  for (const [index, preview] of previews.entries()) {
    await showGeneratedFilePreview(preview, workspaceRoot, previewRoot, index);
  }

  const changed = previews.filter((preview) => preview.changed).length;
  const selection = await vscode.window.showInformationMessage(
    `Apply ADS import files for '${report.connection_name}'?`,
    {
      modal: true,
      detail: `${changed} changed file(s), ${report.selected_count} selected ADS symbol(s). Review the opened diffs before applying.`,
    },
    "Apply",
    "Cancel"
  );
  if (selection !== "Apply") {
    return false;
  }
  for (const preview of previews) {
    const targetPath = resolveWorkspacePath(workspaceRoot, preview.path);
    await fs.promises.mkdir(path.dirname(targetPath), { recursive: true });
    await fs.promises.writeFile(targetPath, preview.content, "utf8");
  }
  return true;
}

async function showGeneratedFilePreview(
  preview: AdsGeneratedFilePreview,
  workspaceRoot: string,
  previewRoot: string,
  index: number
): Promise<void> {
  const targetPath = resolveWorkspacePath(workspaceRoot, preview.path);
  const safeName = path.basename(targetPath) || `ads-preview-${index}`;
  const previewPath = path.join(previewRoot, `${index}-${safeName}`);
  await fs.promises.writeFile(previewPath, preview.content, "utf8");
  const previewUri = vscode.Uri.file(previewPath);
  const relative = relativeWorkspacePath(workspaceRoot, targetPath);
  if (fs.existsSync(targetPath)) {
    await vscode.commands.executeCommand(
      "vscode.diff",
      vscode.Uri.file(targetPath),
      previewUri,
      `ADS import preview: ${relative}`
    );
    return;
  }
  const document = await vscode.workspace.openTextDocument(previewUri);
  await vscode.window.showTextDocument(document, { preview: false });
}

function resolveWorkspacePath(workspaceRoot: string, candidate: string): string {
  return path.isAbsolute(candidate)
    ? candidate
    : path.join(workspaceRoot, candidate);
}

function relativeWorkspacePath(workspaceRoot: string, candidate: string): string {
  const relative = path.relative(workspaceRoot, candidate);
  return relative && !relative.startsWith("..") ? relative : candidate;
}

function blockedReason(runtime: RuntimeTarget): string {
  switch (runtime.status) {
    case "online_reachable":
      return "";
    case "simulate":
      return "Production ADS onboarding is blocked in simulate mode. Switch to an online runtime in the Runtime pane.";
    case "missing_endpoint":
      return "Production ADS onboarding needs a selected online runtime control endpoint.";
    case "auth_failed":
      return "The selected runtime rejected the configured control credentials. Fix the runtime connection in the Runtime pane.";
    case "online_unreachable":
      return "The selected runtime is not reachable. Retry or change runtime in the Runtime pane.";
    default:
      return "Production ADS onboarding is blocked until the runtime is reachable.";
  }
}

function credentialWarning(runtime: RuntimeTarget): string {
  if (runtime.credentialChannel === "trusted_same_host") {
    return "This runtime control channel is same-host trusted. Route credentials may be entered only for one route action and are not stored.";
  }
  if (runtime.credentialChannel === "untrusted_remote_plain_tcp") {
    return "This runtime uses remote plain TCP control. VS Code must not forward TwinCAT credentials over this channel; Add route from this computer uses the local CLI and sends credentials directly to the PLC.";
  }
  return "No trusted credential channel is available for automatic route-add.";
}

async function promptRequiredInput(
  title: string,
  prompt: string,
  placeHolderOrValue: string
): Promise<string | undefined> {
  const value = await vscode.window.showInputBox({
    title,
    prompt,
    placeHolder: placeHolderOrValue,
    value: placeHolderOrValue.startsWith("trust-") ? placeHolderOrValue : undefined,
    ignoreFocusOut: true,
    validateInput: (candidate) =>
      candidate.trim().length === 0 ? `${prompt} is required.` : undefined,
  });
  return value?.trim();
}

async function promptAmsPort(): Promise<number | undefined> {
  const value = await vscode.window.showInputBox({
    title: "Add ADS route from this computer",
    prompt: "TwinCAT PLC AMS port",
    value: "851",
    ignoreFocusOut: true,
    validateInput: (candidate) => {
      const parsed = Number(candidate);
      return Number.isInteger(parsed) && parsed > 0 && parsed <= 65535
        ? undefined
        : "AMS port must be an integer from 1 to 65535.";
    },
  });
  if (!value) {
    return undefined;
  }
  return Number(value);
}

function defaultRouteName(runtime: RuntimeTarget): string {
  const normalized = runtime.label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return `trust-${normalized || "runtime"}`;
}

function renderStatusBody(model: AdsPanelModel): string {
  if (model.statusError) {
    return `<p class="notice blocked">${escapeHtml(model.statusError)}</p>`;
  }
  if (!model.status) {
    return `<p class="notice">No ADS status has been reported by the selected runtime.</p>`;
  }
  if (model.status.connections.length === 0) {
    return `<p class="notice">${escapeHtml(model.status.summary)}</p>`;
  }
  return `<div class="connections">${model.status.connections
    .map(
      (connection) => `<div class="connection">
        <div>
          <strong>${escapeHtml(connection.name)}</strong>
          <div class="muted">${escapeHtml(connection.summary)}</div>
        </div>
        <span>${escapeHtml(connection.state.replace(/_/g, " "))} · ${connection.degraded_points}/${connection.point_count}</span>
      </div>`
    )
    .join("")}</div>`;
}

function renderServerStatusBody(model: AdsPanelModel): string {
  if (model.serverStatusError) {
    return `<p class="notice blocked">${escapeHtml(model.serverStatusError)}</p>`;
  }
  const status = model.serverStatus;
  if (!status) {
    return `<p class="notice">No ADS server status has been reported by the selected runtime.</p>`;
  }
  const identity = status.identity;
  const pending = Array.isArray(status.pending_clients)
    ? status.pending_clients
    : [];
  const proof = serverProofLabel(status.proof_status);
  return `<dl class="facts">
      <dt>State</dt><dd>${escapeHtml(status.status?.overall ?? "unknown")}</dd>
      <dt>Proof</dt><dd>${escapeHtml(proof)}</dd>
      <dt>Bind IP</dt><dd>${escapeHtml(status.listen ?? identity?.chosen_ip ?? "not configured")}</dd>
      <dt>AMS Net ID</dt><dd>${escapeHtml(status.ams_net_id ?? identity?.ams_net_id ?? "not configured")}</dd>
      <dt>ADS port</dt><dd>${escapeHtml(status.ads_port ?? 851)}</dd>
      <dt>Exposed</dt><dd>${escapeHtml(status.exposed_count ?? 0)}</dd>
      <dt>Writable</dt><dd>${escapeHtml(status.writable_count ?? 0)}</dd>
      <dt>Allowed clients</dt><dd>${escapeHtml(status.allowed_client_count ?? 0)}</dd>
      <dt>Connected</dt><dd>${escapeHtml(status.connected_clients ?? "unknown")}</dd>
    </dl>
    <p class="notice">${escapeHtml(model.serverSummary)}</p>
    ${renderServerProofBadges(status)}
    ${renderPendingClients(pending)}
    <div class="actions">
      <button data-action="openSetup"${model.setupUrl ? "" : " disabled"}>Open server setup</button>
      <button data-action="serverDoctor"${model.productionActionsEnabled ? "" : " disabled"}>Run server Doctor</button>
    </div>`;
}

function renderServerProofBadges(status: AdsServerStatusSurface): string {
  const badges = [serverProofLabel(status.proof_status)];
  if (status.discoverable) {
    badges.push("Discoverable");
  }
  if (status.external_client_verified) {
    badges.push("External client verified");
  }
  return `<div>${badges
    .map((badge) => `<span class="badge">${escapeHtml(badge)}</span>`)
    .join(" ")}</div>`;
}

function serverProofLabel(value: string | undefined): string {
  switch (value) {
    case "self_test_available":
      return "Self-test ready";
    case "production_ready":
      return "Production ready";
    case "external_client_verified":
      return "External client verified";
    case "not_ready":
    case undefined:
      return "Not ready";
    default:
      return value.replace(/_/g, " ");
  }
}

function renderPendingClients(clients: AdsServerPendingClient[]): string {
  if (clients.length === 0) {
    return `<p class="notice">No refused ADS client attempts are waiting for review.</p>`;
  }
  return `<div class="connections">${clients
    .map(
      (client) => {
        const snippet = serverClientTomlSnippet(client);
        return `<div class="connection">
        <div>
          <strong>${escapeHtml(client.ams_net_id ?? "unknown AMS Net ID")}</strong>
          <div class="muted">${escapeHtml(client.source_ip ?? "unknown source IP")} · ${escapeHtml(client.reason ?? "refused")}</div>
          ${snippet ? `<pre><code>${escapeHtml(snippet)}</code></pre>` : ""}
        </div>
        <div class="stacked-actions">
          <span>${escapeHtml(client.count ?? 1)} attempt${client.count === 1 ? "" : "s"}</span>
          ${snippet ? `<button data-action="copyServerClient" data-client-toml="${escapeAttribute(snippet)}">Copy allowlist entry</button>` : ""}
        </div>
      </div>`;
      }
    )
    .join("")}</div>`;
}

function serverClientTomlSnippet(client: AdsServerPendingClient): string {
  const suggestion = client.suggested_client;
  const amsNetId = suggestion?.ams_net_id || client.ams_net_id;
  if (!amsNetId) {
    return "";
  }
  const sourceIp = suggestion?.source_ip || client.source_ip;
  const lines = [
    "[[runtime.ads_server.clients]]",
    `ams_net_id = "${tomlString(amsNetId)}"`,
  ];
  if (sourceIp) {
    lines.push(`source_ip = "${tomlString(sourceIp)}"`);
  }
  return lines.join("\n");
}

function tomlString(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
}

function activeActionTitle(action: AdsPanelAction): string {
  switch (action) {
    case "addDevice":
      return "Add Beckhoff ADS Device";
    case "diagnose":
      return "Diagnose ADS Connection";
    case "importSymbols":
      return "Import TwinCAT Symbols";
    case "addRoute":
      return "Add ADS Route";
    case "serverStatus":
      return "ADS Server";
    default:
      return "Runtime-host ADS status";
  }
}

function summarizeAdsServerStatus(
  status: AdsServerStatusSurface | undefined,
  error: string | undefined
): string {
  if (error) {
    return "ADS server status unavailable";
  }
  if (!status) {
    return "ADS server status unavailable";
  }
  const exposed = status.exposed_count ?? 0;
  const clients = status.allowed_client_count ?? 0;
  const pending = Array.isArray(status.pending_clients)
    ? status.pending_clients.length
    : 0;
  return `ADS Server: ${exposed} exposed · ${clients} clients · ${pending} pending`;
}

function statusClass(overall: string | undefined): string {
  const normalized = (overall ?? "unknown").toLowerCase();
  if (["healthy", "degraded", "faulted", "disabled"].includes(normalized)) {
    return normalized;
  }
  return "unknown";
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

function escapeHtml(value: unknown): string {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttribute(value: unknown): string {
  return escapeHtml(value).replace(/'/g, "&#39;");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export async function reopenAdsPanelForTests(
  activeAction: AdsPanelAction = "status"
): Promise<void> {
  if (!extensionContext) {
    throw new Error("ADS panel has not been registered.");
  }
  await showAdsPanel(extensionContext, activeAction);
}

export async function openAdsPanelForAction(
  activeAction: AdsPanelAction = "status"
): Promise<void> {
  if (!extensionContext) {
    throw new Error("ADS panel has not been registered.");
  }
  await showAdsPanel(extensionContext, activeAction);
}
