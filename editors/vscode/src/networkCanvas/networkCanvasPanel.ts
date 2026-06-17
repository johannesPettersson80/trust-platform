import * as vscode from "vscode";
import * as path from "path";

import { debugChannel } from "../debug/configuration";
import type { CommCapabilitiesResponse } from "../communication/capability";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import {
  applyCommSetup,
  clientErrorResult,
  fetchCommSchema,
  normalizeProtocolId,
  testCommSetup,
} from "../communication/runtimeComm";
import {
  openRuntimePane,
  resolveRuntimeTarget,
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { localSimControl } from "../simControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
  type RuntimeStartFailure,
} from "../runtimeLifecycle";
import {
  fetchAndMergeFleetTopologies,
  type FleetTopologyResponse,
} from "./fleetTopology";
import {
  buildNetworkCanvasModel,
  isNetworkCanvasStage,
  NETWORK_CANVAS_IO_PROTOCOLS,
  nextNetworkCanvasStage,
  type NetworkCanvasFailure,
  type NetworkCanvasModel,
  type NetworkCanvasProtocolId,
  type NetworkCanvasStage,
} from "./model";
import { buildCanvasGraph } from "./graphData";

export const NETWORK_CANVAS_COMMAND = "trust-lsp.networkCanvas.open";

const NETWORK_CANVAS_VIEW_TYPE = "trust-network-canvas";
const REFRESH_INTERVAL_MS = 1500;

let panel: vscode.WebviewPanel | undefined;
let currentStage: NetworkCanvasStage = "welcome";
let deviceRequested = false;
let lastFailure: RuntimeStartFailure | undefined;
let refreshTimer: NodeJS.Timeout | undefined;
let activeProtocol: NetworkCanvasProtocolId = "simulated";
let activeSchema: CommSchemaResponse | undefined;
let lastTopology: FleetTopologyResponse | undefined;
let lastApplyResult: CommApplyResponse | undefined;
let searchQuery = "";
let pinnedNodeId: string | undefined;
let quickAddOpen = false;
let runtimeSetupMessage: string | undefined;
let autoStartedSim = false;

export function registerNetworkCanvasPanel(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(NETWORK_CANVAS_COMMAND, async () => {
      await showNetworkCanvasPanel(context);
    })
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => {
      void refreshNetworkCanvasPanel();
    })
  );
}

async function showNetworkCanvasPanel(
  context: vscode.ExtensionContext
): Promise<void> {
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
  } else {
    panel = vscode.window.createWebviewPanel(
      NETWORK_CANVAS_VIEW_TYPE,
      "Structured Text: Network Canvas",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [
          vscode.Uri.file(path.join(context.extensionPath, "media")),
        ],
      }
    );
    panel.webview.html = networkCanvasWebviewHtml(panel.webview, context);
    panel.onDidDispose(() => {
      panel = undefined;
      currentStage = "welcome";
      deviceRequested = false;
      lastFailure = undefined;
      activeProtocol = "simulated";
      activeSchema = undefined;
      lastTopology = undefined;
      lastApplyResult = undefined;
      searchQuery = "";
      pinnedNodeId = undefined;
      quickAddOpen = false;
      runtimeSetupMessage = undefined;
      autoStartedSim = false;
      stopPolling();
    });
    panel.webview.onDidReceiveMessage((message: unknown) => {
      void handleWebviewMessage(message);
    });
    context.subscriptions.push(panel);
  }
  startPolling();
  await refreshNetworkCanvasPanel();
}

async function refreshNetworkCanvasPanel(): Promise<void> {
  if (!panel) {
    return;
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  let runtime = await resolveRuntimeTarget();
  // New/unconfigured project: provision a local simulator automatically so the
  // canvas always has a runtime node — no welcome screen. Honest status: the
  // node stays pending until the lifecycle service proves it is running.
  if (
    !autoStartedSim &&
    runtime.status !== "online_reachable" &&
    !snapshot?.status?.running &&
    !snapshot?.starting
  ) {
    autoStartedSim = true;
    currentStage = "runtime_live";
    void runtimeLifecycleService.startLocalSimulator().then((result) => {
      lastFailure = result.ok ? undefined : result.failure;
      void refreshNetworkCanvasPanel();
    });
  }
  // The local simulator's debug adapter serves the full control API on its own Unix socket
  // (pinned + token-protected via the launch config, see simControl). If the configured target
  // isn't a reachable online runtime but a local sim session is live, adopt the sim's control
  // endpoint so the add-device flow (comm.schema/comm.apply) + fleet.topology work zero-config.
  // Probe-gated via resolveRuntimeTargetFromSettings — only adopted when the socket actually
  // answers with the token, so reachability stays honest (never green from "a session exists").
  if (
    runtime.status !== "online_reachable" &&
    runtimeLifecycleService.getStructuredTextSession()
  ) {
    const sim = localSimControl(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath);
    if (sim) {
      const simTarget = await resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint: sim.endpoint,
        authToken: sim.authToken,
        endpointEnabled: true,
        label: "Local simulator",
      });
      if (simTarget.status === "online_reachable") {
        runtime = simTarget;
      }
    }
  }
  let capabilities: CommCapabilitiesResponse | undefined;
  let topologyError: string | undefined;
  activeSchema = undefined;
  runtimeSetupMessage = undefined;
  lastTopology = undefined;
  if (runtime.status === "online_reachable" && runtime.endpoint) {
    try {
      capabilities = await sendRuntimeControlRequest<CommCapabilitiesResponse>(
        runtime.endpoint,
        runtime.authToken,
        "comm.capabilities",
        undefined,
        { timeoutMs: 2000 }
      );
    } catch {
      capabilities = undefined;
    }
    try {
      activeSchema = await fetchCommSchema(runtime);
    } catch (error) {
      runtimeSetupMessage =
        error instanceof Error ? error.message : String(error);
    }
    try {
      // §12.10: aggregate the primary runtime + any configured fleet peers into one view.
      lastTopology = await fetchAndMergeFleetTopologies(await resolveFleetTargets(runtime));
    } catch (error) {
      topologyError =
        error instanceof Error
          ? `Fleet topology unavailable: ${error.message}`
          : `Fleet topology unavailable: ${String(error)}`;
    }
  } else if (currentStage === "add_device") {
    runtimeSetupMessage =
      runtime.status === "simulate"
        ? "Persistent I/O setup uses the runtime control channel. Select an online runtime to load comm.schema; the local simulator values remain real."
        : "Select a reachable runtime control endpoint before applying I/O driver setup.";
  }
  const model = buildNetworkCanvasModel(
    modelInputForSnapshot(currentStage, snapshot, {
      schema: activeSchema,
      capabilities,
      activeProtocol,
      applyResult: lastApplyResult,
      searchQuery,
      pinnedNodeId,
      quickAddOpen,
      topology: lastTopology,
      topologyError,
      runtimeSetupMessage,
    })
  );
  void panel.webview.postMessage({
    type: "graph",
    graph: buildCanvasGraph(model, lastTopology),
  });
  void panel.webview.postMessage({
    type: "meta",
    schema: activeSchema,
    applyResult: lastApplyResult,
    reachable: runtime.status === "online_reachable",
    setupMessage: runtimeSetupMessage,
  });
}

// §12.10 hybrid source: the primary runtime plus any configured fleet peers, each probed.
// Peers that don't resolve to a reachable online runtime contribute nothing to the merge.
async function resolveFleetTargets(primary: RuntimeTarget): Promise<RuntimeTarget[]> {
  const extra = vscode.workspace
    .getConfiguration("trust-lsp")
    .get<string[]>("runtime.fleetEndpoints", []);
  const endpoints = [
    ...new Set(
      (extra ?? [])
        .map((endpoint) => endpoint.trim())
        .filter((endpoint) => endpoint.length > 0 && endpoint !== primary.endpoint)
    ),
  ];
  if (endpoints.length === 0) {
    return [primary];
  }
  const peers = await Promise.all(
    endpoints.map((endpoint) =>
      resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint,
        endpointEnabled: true,
      }).catch(() => undefined)
    )
  );
  return [primary, ...peers.filter((peer): peer is RuntimeTarget => peer !== undefined)];
}

async function handleWebviewMessage(message: unknown): Promise<void> {
  if (!isRecord(message)) {
    return;
  }
  switch (message.type) {
    case "ready":
      await refreshNetworkCanvasPanel();
      break;
    case "search":
      searchQuery = typeof message.query === "string" ? message.query : "";
      await refreshNetworkCanvasPanel();
      break;
    case "focus":
    case "selectNode":
      pinnedNodeId =
        typeof message.nodeId === "string" && message.nodeId.length > 0
          ? message.nodeId
          : pinnedNodeId;
      await refreshNetworkCanvasPanel();
      break;
    case "action":
      await handleCanvasAction(
        typeof message.action === "string" ? message.action : ""
      );
      break;
    case "startLocalSimulator":
      await handleCanvasAction("startLocalSimulator");
      break;
    case "addSimulatedDevice":
      currentStage = "connected";
      deviceRequested = true;
      {
        const result = await runtimeLifecycleService.requestIoState();
        lastFailure = result.ok ? undefined : result.failure;
      }
      await refreshNetworkCanvasPanel();
      break;
    case "nextStep":
      currentStage = nextNetworkCanvasStage(currentStage);
      await refreshNetworkCanvasPanel();
      break;
    case "showPreview":
      await vscode.window.showInformationMessage(
        "This Network Canvas path is Preview. Open Communication for the production setup flow."
      );
      break;
    case "setupProtocol":
    case "adoptDiscovered":
      {
        const protocol = normalizeCanvasProtocol(message.protocol);
        if (!protocol) {
          await vscode.window.showInformationMessage(
            "That Network Canvas path is Preview in this slice."
          );
          return;
        }
        activeProtocol = protocol;
        currentStage = "add_device";
        quickAddOpen = false;
        lastApplyResult = undefined;
        await refreshNetworkCanvasPanel();
      }
      break;
    case "commApply":
      await applyNetworkCanvasSetup(message);
      break;
    case "commTest":
      await testNetworkCanvasSetup(message);
      break;
    case "commApplyClientError":
      {
        const result = clientErrorResult(message.protocol, message.fieldErrors);
        if (result) {
          const protocol = normalizeCanvasProtocol(result.protocol);
          if (protocol) {
            activeProtocol = protocol;
          }
          lastApplyResult = result.applyResult;
          await refreshNetworkCanvasPanel();
        }
      }
      break;
    case "setSearch":
      searchQuery = typeof message.query === "string" ? message.query : "";
      await refreshNetworkCanvasPanel();
      break;
    case "setPinnedNode":
    case "focusFault":
      pinnedNodeId =
        typeof message.nodeId === "string" && message.nodeId.length > 0
          ? message.nodeId
          : pinnedNodeId;
      await refreshNetworkCanvasPanel();
      break;
    case "openQuickAdd":
      quickAddOpen = !quickAddOpen;
      await refreshNetworkCanvasPanel();
      break;
    case "setStage":
      if (isNetworkCanvasStage(message.stage)) {
        currentStage = message.stage;
        if (currentStage === "welcome") {
          deviceRequested = false;
          lastFailure = undefined;
        }
        await refreshNetworkCanvasPanel();
      }
      break;
    case "openRuntimePane":
      await openRuntimePane();
      break;
    case "openRuntimeSettings":
      await vscode.commands.executeCommand(
        "trust-lsp.debug.openIoPanelSettings"
      );
      break;
    case "openRuntimeLogs":
      debugChannel().show(true);
      break;
    case "openCommunication":
      await vscode.commands.executeCommand("trust-lsp.communication.openPanel");
      break;
  }
}

async function applyNetworkCanvasSetup(
  message: Record<string, unknown>
): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  const result = await applyCommSetup(runtime, message, activeSchema);
  if (!result) {
    return;
  }
  const protocol = normalizeCanvasProtocol(result.protocol);
  if (protocol) {
    activeProtocol = protocol;
  }
  lastApplyResult = result.applyResult;
  await refreshNetworkCanvasPanel();
}

async function testNetworkCanvasSetup(
  message: Record<string, unknown>
): Promise<void> {
  const runtime = await resolveRuntimeTarget();
  const result = await testCommSetup(runtime, message, activeSchema);
  if (!result) {
    return;
  }
  const protocol = normalizeCanvasProtocol(result.protocol);
  if (protocol) {
    activeProtocol = protocol;
  }
  lastApplyResult = result.applyResult;
  await refreshNetworkCanvasPanel();
}

function modelInputForSnapshot(
  stage: NetworkCanvasStage,
  snapshot: RuntimeLifecycleSnapshot | undefined,
  options: {
    schema?: CommSchemaResponse;
    capabilities?: CommCapabilitiesResponse;
    activeProtocol?: NetworkCanvasProtocolId;
    applyResult?: CommApplyResponse;
    searchQuery?: string;
    pinnedNodeId?: string;
    quickAddOpen?: boolean;
    topology?: FleetTopologyResponse;
    topologyError?: string;
    runtimeSetupMessage?: string;
  } = {}
) {
  return {
    stage,
    runtime: snapshot?.status,
    ioState: snapshot?.ioState,
    schema: options.schema,
    capabilities: options.capabilities,
    activeProtocol: options.activeProtocol,
    applyResult: options.applyResult,
    searchQuery: options.searchQuery,
    pinnedNodeId: options.pinnedNodeId,
    quickAddOpen: options.quickAddOpen,
    topology: options.topology,
    topologyError: options.topologyError,
    starting: snapshot?.starting,
    failure: asNetworkFailure(lastFailure ?? snapshot?.failure),
    deviceRequested,
    runtimeSetupMessage: options.runtimeSetupMessage,
  };
}

function normalizeCanvasProtocol(value: unknown): NetworkCanvasProtocolId | undefined {
  const normalized = normalizeProtocolId(value);
  return normalized &&
    NETWORK_CANVAS_IO_PROTOCOLS.includes(normalized as NetworkCanvasProtocolId)
    ? (normalized as NetworkCanvasProtocolId)
    : undefined;
}

function asNetworkFailure(
  failure: RuntimeStartFailure | undefined
): NetworkCanvasFailure | undefined {
  if (!failure) {
    return undefined;
  }
  return {
    kind: failure.kind,
    message: failure.message,
    detail: failure.detail,
  };
}

async function handleCanvasAction(action: string): Promise<void> {
  switch (action) {
    case "startLocalSimulator":
      currentStage = "runtime_live";
      lastFailure = undefined;
      await refreshNetworkCanvasPanel();
      {
        const result = await runtimeLifecycleService.startLocalSimulator();
        lastFailure = result.ok ? undefined : result.failure;
      }
      await refreshNetworkCanvasPanel();
      break;
    case "openRuntimePane":
      await openRuntimePane();
      break;
    case "openRuntimeLogs":
      debugChannel().show(true);
      break;
    case "openRuntimeSettings":
      await vscode.commands.executeCommand(
        "trust-lsp.debug.openIoPanelSettings"
      );
      break;
    case "addDevice":
    case "openQuickAdd":
    case "openCommunication":
      await vscode.commands.executeCommand("trust-lsp.communication.openPanel");
      break;
  }
}

function networkCanvasWebviewHtml(
  webview: vscode.Webview,
  context: vscode.ExtensionContext
): string {
  const scriptUri = webview.asWebviewUri(
    vscode.Uri.file(
      path.join(context.extensionPath, "media", "networkCanvasWebview.js")
    )
  );
  const styleUri = webview.asWebviewUri(
    vscode.Uri.file(
      path.join(context.extensionPath, "media", "networkCanvasWebview.css")
    )
  );
  const csp = `default-src 'none'; img-src ${webview.cspSource} data: https:; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-eval'; font-src ${webview.cspSource} data:;`;
  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="${csp}" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Network Canvas</title>
    <link rel="stylesheet" href="${styleUri}" />
    <style>
      * { box-sizing: border-box; margin: 0; padding: 0; }
      html, body, #root {
        width: 100%; height: 100%; overflow: hidden;
        font-family: var(--vscode-font-family, -apple-system, "Segoe UI", sans-serif);
        background: #0f1116; color: #eef1f5;
      }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script src="${scriptUri}"></script>
  </body>
</html>`;
}

function startPolling(): void {
  if (refreshTimer) {
    return;
  }
  refreshTimer = setInterval(() => {
    void refreshNetworkCanvasPanel();
  }, REFRESH_INTERVAL_MS);
}

function stopPolling(): void {
  if (refreshTimer) {
    clearInterval(refreshTimer);
    refreshTimer = undefined;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
