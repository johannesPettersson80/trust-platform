import * as vscode from "vscode";
import * as path from "path";

import { getTrustConfiguration } from "../configuration";
import { debugChannel } from "../debug/configuration";
import type { CommCapabilitiesResponse } from "../communication/capability";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import {
  clientErrorResult,
  fetchCommSchema,
  normalizeProtocolId,
} from "../communication/runtimeComm";
import {
  openRuntimePane,
  resolveRuntimeTarget,
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { getControlAuthToken } from "../runtimeAuth";
import { localSimControl } from "../simControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
  type RuntimeStartFailure,
} from "../runtimeLifecycle";
import { getSelectedRuntimeId, setSelectedRuntimeId } from "../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../trustHomeModel";
import {
  listManagedRuntimes,
  onDidChangeManagedRuntimes,
  showManagedRuntimeLogs,
  startManagedRuntime,
  stopManagedRuntime,
} from "../localRuntime";
import {
  attachManagedRuntimeAfterStart,
  disconnectManagedRuntimeAfterStop,
} from "../managedRuntimeSession";
import {
  fetchFleetTopology,
  mergeFleetTopologies,
  type FleetTopologyResponse,
} from "./fleetTopology";
import {
  fetchConnectorStatus,
  fetchAndMergeFleetTopologiesWithConnectorStatus,
  mergeConnectorStatusIntoTopology,
} from "./connectorsStatus";
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
import {
  offlineCommSchema,
  offlineCommTopology,
} from "./offlineComm";
import {
  DiscoveryRequestTracker,
  isActiveWebviewSession,
} from "./discoverySession";
import {
  parseDiscoveryEnvelope,
  runNetworkCanvasDiscovery,
  type DiscoveryMessageEnvelope,
} from "./discoveryController";
import { NetworkCanvasProtocolActions } from "./protocolActions";
import { networkCanvasWebviewHtml } from "./webviewHtml";
import { NetworkCanvasFleetActions } from "./fleetActions";
import { NetworkCanvasConfigurationActions } from "./configurationActions";
import { becameVisible } from "./panelVisibility";
import {
  LatestRefreshCoordinator,
  type LatestRefreshContext,
} from "./refreshCoordinator";

export const NETWORK_CANVAS_COMMAND = "trust-lsp.networkCanvas.open";

const NETWORK_CANVAS_VIEW_TYPE = "trust-network-canvas";
const REFRESH_INTERVAL_MS = 1500;

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let currentStage: NetworkCanvasStage = "welcome";
let deviceRequested = false;
let lastFailure: RuntimeStartFailure | undefined;
let refreshTimer: NodeJS.Timeout | undefined;
let activeProtocol: NetworkCanvasProtocolId = "simulated";
let activeSchema: CommSchemaResponse | undefined;
let lastTopology: FleetTopologyResponse | undefined;
let activeRuntimeTarget: RuntimeTarget | undefined;
let lastApplyResult: CommApplyResponse | undefined;
let searchQuery = "";
let pinnedNodeId: string | undefined;
let pendingFocusNodeId: string | undefined;
let quickAddOpen = false;
let runtimeSetupMessage: string | undefined;
const fleetEndpointLabels = new Map<string, string>();
const refreshCoordinator = new LatestRefreshCoordinator();
const discoveryRequests = new DiscoveryRequestTracker<vscode.WebviewPanel>();
let activeDiscoveryRequest:
  | { readonly sessionId: string; readonly requestId: number }
  | undefined;
let activeWebviewSessionId: string | undefined;

const protocolActions = new NetworkCanvasProtocolActions({
  panel: () => panel,
  extensionContext: () => extensionContext,
  topology: () => lastTopology,
  runtimeTarget: () => activeRuntimeTarget,
  refresh: refreshNetworkCanvasPanel,
  startRuntime: startConfiguredRuntime,
});
const fleetActions = new NetworkCanvasFleetActions({
  extensionContext: () => extensionContext,
  endpointLabels: fleetEndpointLabels,
  focusEndpoint: (nodeId) => {
    pendingFocusNodeId = nodeId;
    pinnedNodeId = nodeId;
  },
  refresh: refreshNetworkCanvasPanel,
});
const configurationActions = new NetworkCanvasConfigurationActions({
  extensionContext: () => extensionContext,
  schema: () => activeSchema,
  commit: (protocol, result) => {
    const normalized = normalizeCanvasProtocol(protocol);
    if (normalized) {
      activeProtocol = normalized;
    }
    lastApplyResult = result;
  },
  refresh: refreshNetworkCanvasPanel,
});

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
  // A managed runtime starting/stopping (here or from the Run bar) re-renders its node state.
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => {
      void refreshNetworkCanvasPanel();
    })
  );
}

async function showNetworkCanvasPanel(
  context: vscode.ExtensionContext
): Promise<void> {
  extensionContext = context;
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
  } else {
    panel = vscode.window.createWebviewPanel(
      NETWORK_CANVAS_VIEW_TYPE,
      "Devices & Connections",
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
    let wasVisible = panel.visible;
    panel.onDidDispose(() => {
      refreshCoordinator.invalidate();
      discoveryRequests.invalidate();
      activeDiscoveryRequest = undefined;
      activeWebviewSessionId = undefined;
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
      pendingFocusNodeId = undefined;
      quickAddOpen = false;
      runtimeSetupMessage = undefined;
      stopPolling();
    });
    panel.onDidChangeViewState(({ webviewPanel }) => {
      const panelBecameVisible = becameVisible(
        wasVisible,
        webviewPanel.visible
      );
      wasVisible = webviewPanel.visible;
      if (panelBecameVisible) {
        startPolling();
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
        if (activeWebviewSessionId) {
          void webviewPanel.webview.postMessage({
            type: "discoverReset",
            sessionId: activeWebviewSessionId,
          });
        }
        void webviewPanel.webview.postMessage({ type: "browseReset" });
        void refreshNetworkCanvasPanel();
      } else if (!webviewPanel.visible) {
        stopPolling();
        refreshCoordinator.invalidate();
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
      }
    });
    panel.webview.onDidReceiveMessage((message: unknown) => {
      void handleWebviewMessage(message);
    });
    context.subscriptions.push(panel);
  }
  startPolling();
  void refreshNetworkCanvasPanel();
}

async function refreshNetworkCanvasPanel(): Promise<void> {
  const panelRef = panel;
  if (!panelRef || !panelRef.visible) {
    return;
  }
  await refreshCoordinator.request((context) =>
    refreshNetworkCanvasPanelOnce(panelRef, context)
  );
}

async function refreshNetworkCanvasPanelOnce(
  panelRef: vscode.WebviewPanel,
  refreshContext: LatestRefreshContext
): Promise<void> {
  const refreshDelayMs = networkCanvasRefreshDelayMs();
  if (refreshDelayMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, refreshDelayMs));
    if (!refreshContext.isCurrent() || panel !== panelRef || !panelRef.visible) {
      return;
    }
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  let runtime = await resolveRuntimeTarget(workspaceConfigResource());
  // The canvas SHOWS the local simulator as a node in the host but NEVER auto-starts it —
  // the user starts it on demand (the node's Start action). The graph below renders it
  // stopped until the lifecycle service proves it is running.
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
        label: "Simulator",
      });
      if (simTarget.status === "online_reachable") {
        runtime = simTarget;
      }
    }
  }
  let capabilities: CommCapabilitiesResponse | undefined;
  let topologyError: string | undefined;
  let nextSchema: CommSchemaResponse | undefined;
  let nextRuntimeSetupMessage: string | undefined;
  let nextTopology: FleetTopologyResponse | undefined;
  let offlineTopology: FleetTopologyResponse | undefined;
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

  // Offline-first: the protocol schema and the configured topology come from the trust-runtime
  // CLI (no running runtime), so the canvas + settings work whether the project is stopped or
  // online. (Returns undefined on an older binary without the `comm` subcommands → falls back.)
  if (extensionContext) {
    nextSchema = await offlineCommSchema(extensionContext);
    if (projectDir) {
      offlineTopology = await offlineCommTopology(extensionContext, projectDir);
      nextTopology = offlineTopology;
    }
  }

  // Live overlay: when a runtime is reachable, prefer its live schema + topology (real status).
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
      const liveSchema = await fetchCommSchema(runtime);
      if (liveSchema) {
        nextSchema = liveSchema;
      }
    } catch (error) {
      if (!nextSchema) {
        nextRuntimeSetupMessage =
          error instanceof Error ? error.message : String(error);
      }
    }
    try {
      // The local primary's own live topology (real status). Peers are resolved separately below.
      const liveTopology = mergeConnectorStatusIntoTopology(
        await fetchFleetTopology(runtime),
        await fetchConnectorStatus(runtime).catch(() => undefined)
      );
      if (liveTopology) {
        // Keep the project-file topology as a configured overlay when the selected runtime comes
        // online. Live topology owns real state; offline topology contributes configured endpoints
        // that may require restart before the runtime reports them.
        const shouldPreserveProjectOverlay = offlineTopology !== undefined;
        nextTopology = shouldPreserveProjectOverlay
          ? mergeFleetTopologies([liveTopology, offlineTopology])
          : liveTopology;
      }
    } catch (error) {
      topologyError =
        error instanceof Error
          ? `Fleet topology unavailable: ${error.message}`
          : `Fleet topology unavailable: ${String(error)}`;
    }
  }

  // §12.10: configured fleet peers (added hosts) ALWAYS resolve — real topology if reachable, a
  // stopped node if not — whether or not the local primary is running, so an added host/runtime
  // never silently vanishes. Kept separate from lastTopology so the local view is preserved.
  let peerTopology: FleetTopologyResponse | undefined;
  try {
    const peers = (await resolveFleetTargets(runtime)).filter(
      (target) => target.endpoint && target.endpoint !== runtime.endpoint
    );
    if (peers.length > 0) {
      peerTopology = await fetchAndMergeFleetTopologiesWithConnectorStatus(peers);
    }
  } catch {
    // Peers are best-effort; the local view still renders without them.
  }

  const displayTopology = peerTopology
    ? mergeFleetTopologies([nextTopology, peerTopology])
    : nextTopology;

  const model = buildNetworkCanvasModel(
    modelInputForSnapshot(currentStage, snapshot, {
      schema: nextSchema,
      capabilities,
      activeProtocol,
      applyResult: lastApplyResult,
      searchQuery,
      pinnedNodeId,
      quickAddOpen,
      topology: displayTopology,
      topologyError,
      runtimeSetupMessage: nextRuntimeSetupMessage,
    })
  );
  // Honest "attached" signal for the runtime-node controls: the endpoint we actually hold a live
  // connection to (an attached remote), so exactly one remote shows "Disconnect" — never every
  // reachable peer.
  const attachedEndpoint =
    snapshot.status.runtimeMode === "online" &&
    snapshot.status.runtimeState === "connected"
      ? snapshot.status.endpoint
      : undefined;
  // Managed local runtimes (fleet.toml projects we own) injected as nodes so Start/Stop/Logs live on
  // the canvas node (§0.6 / Phase 9). Same service the Run bar uses — one lifecycle model.
  const managed = extensionContext
    ? await listManagedRuntimes(extensionContext)
    : [];
  if (!refreshContext.isCurrent() || panel !== panelRef || !panelRef.visible) {
    return;
  }
  activeRuntimeTarget = runtime;
  activeSchema = nextSchema;
  runtimeSetupMessage = nextRuntimeSetupMessage;
  lastTopology = nextTopology;
  void panelRef.webview.postMessage({
    type: "graph",
    graph: buildCanvasGraph(
      model,
      displayTopology,
      undefined,
      attachedEndpoint,
      managed,
      getSelectedRuntimeId()
    ),
  });
  void panelRef.webview.postMessage({
    type: "meta",
    schema: nextSchema,
    applyResult: lastApplyResult,
    reachable: runtime.status === "online_reachable",
    setupMessage: nextRuntimeSetupMessage,
  });
  if (pendingFocusNodeId) {
    void panelRef.webview.postMessage({
      type: "focusNode",
      nodeId: pendingFocusNodeId,
    });
    pendingFocusNodeId = undefined;
  }
}

// §12.10 hybrid source: the primary runtime plus any configured fleet peers, each probed.
// Peers that don't resolve to a reachable online runtime contribute nothing to the merge.
async function resolveFleetTargets(primary: RuntimeTarget): Promise<RuntimeTarget[]> {
  const extra = trustConfig().get<string[]>("runtime.fleetEndpoints", []);
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
    endpoints.map(async (endpoint) =>
      resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint,
        authToken: await getControlAuthToken(endpoint),
        endpointEnabled: true,
        label: fleetEndpointLabels.get(endpoint),
      }).catch(() => undefined)
    )
  );
  return [primary, ...peers.filter((peer): peer is RuntimeTarget => peer !== undefined)];
}

function workspaceConfigResource(): vscode.Uri | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri;
}

function trustConfig(): vscode.WorkspaceConfiguration {
  return getTrustConfiguration(workspaceConfigResource());
}

// §0.5 Discover: run `comm.discover` for each selected protocol IN SEQUENCE (clear per-row
// progress), then post the combined candidates. Degrades gracefully if the verb isn't there yet.
async function handleDiscover(message: Record<string, unknown>): Promise<void> {
  const panelRef = panel;
  const contextRef = extensionContext;
  const envelope = parseDiscoveryEnvelope(message);
  if (
    !panelRef ||
    !contextRef ||
    !envelope ||
    !isActiveWebviewSession(envelope.sessionId, activeWebviewSessionId)
  ) {
    return;
  }
  const token = discoveryRequests.start(panelRef);
  activeDiscoveryRequest = {
    sessionId: envelope.sessionId,
    requestId: envelope.requestId,
  };
  const discoveryRuntimeTarget = await resolveDiscoveryRuntimeTarget(envelope);
  if (!discoveryRequests.isCurrent(token, panelRef)) {
    return;
  }
  await runNetworkCanvasDiscovery(envelope, {
    panel: panelRef,
    extensionContext: contextRef,
    runtimeTarget: discoveryRuntimeTarget,
    tracker: discoveryRequests,
    token,
  });
}

async function resolveDiscoveryRuntimeTarget(
  envelope: DiscoveryMessageEnvelope
): Promise<RuntimeTarget | undefined> {
  if (envelope.request.origin === "this_host") {
    return undefined;
  }
  const endpoint = envelope.request.originEndpoint?.trim();
  if (endpoint) {
    return resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint,
      authToken: await getControlAuthToken(endpoint),
      endpointEnabled: true,
      label: envelope.request.origin,
    }).catch(() => undefined);
  }
  return envelope.request.origin === "runtime:local"
    ? activeRuntimeTarget
    : undefined;
}

function networkCanvasRefreshDelayMs(): number {
  const value = Number(
    process.env.TRUST_VSCODE_NETWORK_CANVAS_REFRESH_DELAY_MS ?? 0
  );
  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }
  return Math.min(Math.floor(value), 10_000);
}
async function handleWebviewMessage(message: unknown): Promise<void> {
  if (!isRecord(message)) {
    return;
  }
  switch (message.type) {
    case "ready":
      discoveryRequests.invalidate();
      activeDiscoveryRequest = undefined;
      activeWebviewSessionId =
        typeof message.sessionId === "string" ? message.sessionId : undefined;
      if (panel && activeWebviewSessionId) {
        void panel.webview.postMessage({
          type: "discoverReset",
          sessionId: activeWebviewSessionId,
        });
      }
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
    case "setupProtocol":
    case "adoptDiscovered":
      {
        const protocol = normalizeCanvasProtocol(message.protocol);
        if (!protocol) {
          await vscode.window.showInformationMessage(
            "That device protocol isn't available yet."
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
      await configurationActions.apply(message);
      break;
    case "commSave":
      // Editable inspector: write config to disk (works stopped/offline).
      await configurationActions.save(message, "upsert");
      break;
    case "commRemove":
      await configurationActions.save(message, "remove");
      break;
    case "commDisable":
      await configurationActions.save(message, "disable");
      break;
    case "commApplyLive":
      // Explicit "push to the running runtime now" — control channel, online only.
      await configurationActions.apply(message);
      break;
    case "commTest":
      await configurationActions.test(message);
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
    case "clearApplyResult":
      lastApplyResult = undefined;
      await refreshNetworkCanvasPanel();
      break;
    case "addHost":
      await fleetActions.addHost(message);
      break;
    case "addRuntime":
      await fleetActions.addRuntime(message);
      break;
    case "discover":
      await handleDiscover(message);
      break;
    case "cancelDiscover":
      if (
        activeDiscoveryRequest &&
        message.sessionId === activeDiscoveryRequest.sessionId &&
        message.requestId === activeDiscoveryRequest.requestId
      ) {
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
      }
      break;
    case "browseSymbols":
      await protocolActions.browseSymbols(message);
      break;
    case "createRoute":
      // The browse pane owns the visible recovery instructions. Keep this notification aligned with
      // that in-canvas flow; do not send users to retired ADS panels.
      await vscode.window.showInformationMessage(
        "Run the generated ADS route PowerShell as Administrator on the TwinCAT computer, then reopen Browse."
      );
      break;
    case "addTags":
      await protocolActions.addTags(message);
      break;
    case "addEthercatChannels":
      await protocolActions.addEthercatChannels(message);
      break;
    case "addExpose":
      await protocolActions.addExpose(message);
      break;
    case "addOpcuaConnection":
      await protocolActions.addOpcuaConnection(message);
      break;
    case "copyText":
      if (typeof message.text === "string" && message.text.length > 0) {
        await vscode.env.clipboard.writeText(message.text);
        await vscode.window.showInformationMessage("Copied to clipboard.");
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
    case "runtimeConnect":
      // Connect (attach) to a remote runtime by its control endpoint — never a remote "Start".
      {
        const endpoint =
          typeof message.endpoint === "string" ? message.endpoint : "";
        const label =
          typeof message.label === "string" ? message.label : undefined;
        const result = await runtimeLifecycleService.connectRemote(endpoint, label);
        lastFailure = result.ok ? undefined : result.failure;
        // Connecting also makes this runtime the active Run target (§0.5.11) — one source of truth.
        if (result.ok && endpoint) {
          await setSelectedRuntimeId(endpoint);
        }
      }
      await refreshNetworkCanvasPanel();
      break;
    case "runtimeDisconnect":
      // Disconnect drops OUR connection (ends the attach session) — it never kills a remote we
      // don't own.
      await runtimeLifecycleService.stopRuntime();
      lastFailure = undefined;
      await refreshNetworkCanvasPanel();
      break;
    case "setRuntimeAuthToken":
      {
        const endpoint =
          typeof message.endpoint === "string" ? message.endpoint : "";
        await vscode.commands.executeCommand(
          "trust-lsp.runtime.setAuthToken",
          { endpoint }
        );
      }
      await refreshNetworkCanvasPanel();
      break;
    case "setAsRunTarget":
      // Select this runtime for the Run bar WITHOUT connecting (§0.5.11). Managed node → its name;
      // local sim node → the Simulator; remote → its endpoint.
      {
        const managedName =
          typeof message.managedName === "string" ? message.managedName : "";
        const endpoint =
          typeof message.endpoint === "string" ? message.endpoint : "";
        const target = managedName
          ? managedName
          : message.isLocal || !endpoint
            ? SIMULATOR_RUNTIME_ID
            : endpoint;
        await setSelectedRuntimeId(target);
      }
      await refreshNetworkCanvasPanel();
      break;
    case "runtimeManagedStart":
    case "runtimeManagedStop":
      // Managed local runtime lifecycle — SAME service the Run bar uses (one lifecycle model).
      {
        const name = typeof message.name === "string" ? message.name : "";
        if (extensionContext && name) {
          const starting = message.type === "runtimeManagedStart";
          const result =
            starting
              ? await startManagedRuntime(extensionContext, name)
              : await stopManagedRuntime(extensionContext, name);
          if (!result.ok) {
            void vscode.window.showWarningMessage(
              result.message ||
                `Could not ${starting ? "start" : "stop"} ${name}.`
            );
          } else if (starting) {
            const attach = await attachManagedRuntimeAfterStart(name, result);
            lastFailure = undefined;
            if (!attach.ok) {
              void vscode.window.showWarningMessage(
                attach.message || `Runtime started, but Live Values could not connect.`
              );
            }
          } else {
            await disconnectManagedRuntimeAfterStop(name, result);
          }
        }
      }
      await refreshNetworkCanvasPanel();
      break;
    case "runtimeManagedLogs":
      {
        const name = typeof message.name === "string" ? message.name : "";
        if (extensionContext && name) {
          await showManagedRuntimeLogs(extensionContext, name);
        }
      }
      break;
  }
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
    case "stopLocalSimulator":
      await runtimeLifecycleService.stopRuntime();
      lastFailure = undefined;
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
  }
}

async function startConfiguredRuntime(): Promise<void> {
  currentStage = "runtime_live";
  lastFailure = undefined;
  await refreshNetworkCanvasPanel();
  const result = await runtimeLifecycleService.startRuntime();
  lastFailure = result.ok ? undefined : result.failure;
  await refreshNetworkCanvasPanel();
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
