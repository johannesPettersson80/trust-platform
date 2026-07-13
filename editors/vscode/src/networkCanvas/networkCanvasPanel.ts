import * as vscode from "vscode";
import * as path from "path";

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
import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
  type RuntimeStartFailure,
} from "../runtimeLifecycle";
import { getSelectedRuntimeId } from "../selectedRuntime";
import {
  lifecycleActionSucceeded,
  type LifecycleAction,
} from "../lifecycleEntryFailure";
import {
  listManagedRuntimes,
  onDidChangeManagedRuntimes,
} from "../localRuntime";
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
  type NetworkCanvasProtocolId,
  type NetworkCanvasStage,
} from "./model";
import { buildCanvasGraph } from "./graphData";
import { offlineCommSchema, offlineCommTopology } from "./offlineComm";
import {
  DiscoveryRequestTracker,
  isActiveWebviewSession,
} from "./discoverySession";
import {
  parseDiscoveryEnvelope,
  runNetworkCanvasDiscovery,
} from "./discoveryController";
import { NetworkCanvasProtocolActions } from "./protocolActions";
import {
  AdsServiceProbeController,
  isCurrentAdsServiceProbeRequest,
  localRuntimeTargetForAdsProbe,
} from "./adsServiceProbeController";
import { DiscoveryOriginContext } from "./discoveryOriginContext";
import { networkCanvasWebviewHtml } from "./webviewHtml";
import { NetworkCanvasFleetActions } from "./fleetActions";
import { NetworkCanvasConfigurationActions } from "./configurationActions";
import { becameVisible } from "./panelVisibility";
import {
  LatestRefreshCoordinator,
  networkCanvasRefreshDelayMs,
  type LatestRefreshContext,
} from "./refreshCoordinator";
import { NetworkCanvasPolling } from "./panelPolling";
import { shouldRefreshNetworkCanvasForLifecycleChange } from "./lifecycleRefreshPolicy";
import { initialNetworkCanvasGraph } from "./initialGraph";
import {
  buildImmediateSimulatorLifecycleGraph,
  modelInputForSnapshot,
} from "./lifecycleModel";
import { resolveNetworkCanvasFleetTargets } from "./fleetTargetResolver";
import { NetworkCanvasLifecycleActions } from "./lifecycleActions";
import { projectCanvasLifecycleAuthority } from "./lifecycleAuthorityProjection";
import { NetworkCanvasRuntimeAuthority } from "./runtimeAuthorityState";
import { resolveNetworkCanvasRuntimeTarget } from "./runtimeTargetResolution";

export const NETWORK_CANVAS_COMMAND = "trust-lsp.networkCanvas.open";

const NETWORK_CANVAS_VIEW_TYPE = "trust-network-canvas";

let panel: vscode.WebviewPanel | undefined;
let extensionContext: vscode.ExtensionContext | undefined;
let currentStage: NetworkCanvasStage = "welcome";
let deviceRequested = false;
let lastFailure: RuntimeStartFailure | undefined;
let lastFailureAction: LifecycleAction | undefined;
let activeProtocol: NetworkCanvasProtocolId = "simulated";
let activeSchema: CommSchemaResponse | undefined;
let lastTopology: FleetTopologyResponse | undefined;
let lastDisplayTopology: FleetTopologyResponse | undefined;
const runtimeAuthority = new NetworkCanvasRuntimeAuthority();
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
const discoveryOriginContext = new DiscoveryOriginContext();
const polling = new NetworkCanvasPolling(refreshNetworkCanvasPanel, 1500);
let activeDiscoveryRequest:
  | {
      readonly sessionId: string;
      readonly requestId: number;
      readonly origin: string;
    }
  | undefined;
let activeWebviewSessionId: string | undefined;
function clearDiscoveryOriginContext(): void {
  adsServiceProbeController.cancel();
  discoveryOriginContext.clearCredentials();
}

const lifecycleActions = new NetworkCanvasLifecycleActions({
  extensionContext: () => extensionContext,
  refresh: refreshNetworkCanvasPanel,
  clearFailure: () => {
    lastFailure = undefined;
    lastFailureAction = undefined;
  },
  recordResult: (result, action) => {
    lastFailure = result.ok ? undefined : result.failure;
    lastFailureAction = result.ok ? undefined : action;
  },
  stopRuntime: () => runtimeLifecycleService.stopRuntime(),
  connectRemote: (endpoint, label) =>
    runtimeLifecycleService.connectRemote(endpoint, label),
  runExclusiveOperation: (kind, target, operation) =>
    runtimeLifecycleService.runExclusiveOperation(kind, target, operation),
  lifecyclePhase: () => runtimeLifecycleService.phase(),
  // Host mutation consumes the exact inventory-validated authority used by
  // the latest graph. Raw debug-session labels never authorize Stop.
  activeTarget: () => runtimeAuthority.activeTarget(),
  managedTarget: (name, endpoint) =>
    runtimeAuthority.managedTarget(name, endpoint),
  operationInProgress: () =>
    runtimeLifecycleService.operationState() !== undefined,
});

const protocolActions = new NetworkCanvasProtocolActions({
  panel: () => panel,
  extensionContext: () => extensionContext,
  topology: () => lastTopology,
  runtimeTarget: () => activeRuntimeTarget,
  runtimeTargetForOrigin: (originId, leaseId, browseSessionId) =>
    discoveryOriginContext.browseTarget(
      originId,
      leaseId,
      activeWebviewSessionId,
      browseSessionId,
    ),
  refresh: refreshNetworkCanvasPanel,
});
const adsServiceProbeController = new AdsServiceProbeController({
  panel: () => panel,
  extensionContext: () => extensionContext,
  runtimeTargetForOrigin: (originId) =>
    discoveryOriginContext.probeTarget(originId),
  runtimeTargetOnDiscoveryComputer: () =>
    localRuntimeTargetForAdsProbe(activeRuntimeTarget),
  requestIsCurrent: (request) =>
    isCurrentAdsServiceProbeRequest(
      {
        sessionId: request.sessionId,
        requestId: request.requestId,
        origin: request.origin,
        candidate: request.candidate,
      },
      activeDiscoveryRequest,
      activeWebviewSessionId,
    ),
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

export function registerNetworkCanvasPanel(
  context: vscode.ExtensionContext,
): void {
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      discoveryRequests.invalidate();
      activeDiscoveryRequest = undefined;
      clearDiscoveryOriginContext();
      discoveryOriginContext.clearEndpoints();
    }),
    vscode.commands.registerCommand(NETWORK_CANVAS_COMMAND, async () => {
      await showNetworkCanvasPanel(context);
    }),
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange((change) => {
      const phase = runtimeLifecycleService.phase();
      runtimeAuthority.reconcile(rawLifecycleAuthorityTarget(phase));
      // Reconcile before any async/visibility-gated refresh. Ordinary I/O
      // events must not erase a failed Stop while the runtime is still running.
      if (lifecycleActionSucceeded(lastFailureAction, phase)) {
        lastFailure = undefined;
        lastFailureAction = undefined;
      }
      if (!shouldRefreshNetworkCanvasForLifecycleChange(change)) {
        return;
      }
      postImmediateSimulatorLifecycleGraph(phase);
      void refreshNetworkCanvasPanel();
    }),
  );
  // A managed runtime starting/stopping (here or from the Run bar) re-renders its node state.
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => {
      // Inventory identity changed. Fail closed until the refreshed inventory
      // has normalized the accepted session for both rendering and mutation.
      runtimeAuthority.invalidateInventory(
        rawLifecycleAuthorityTarget(runtimeLifecycleService.phase()),
      );
      void refreshNetworkCanvasPanel();
    }),
  );
}

async function showNetworkCanvasPanel(
  context: vscode.ExtensionContext,
): Promise<void> {
  extensionContext = context;
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
    postImmediateSimulatorLifecycleGraph(runtimeLifecycleService.phase());
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
      },
    );
    const initialPhase = runtimeLifecycleService.phase();
    const initialAuthorityTarget = runtimeAuthority.beginFirstPaint(
      rawLifecycleAuthorityTarget(initialPhase),
    );
    panel.webview.html = networkCanvasWebviewHtml(
      panel.webview,
      context,
      initialNetworkCanvasGraph(
        initialPhase,
        currentStage,
        getSelectedRuntimeId(),
        initialAuthorityTarget,
      ),
    );
    let wasVisible = panel.visible;
    panel.onDidDispose(() => {
      refreshCoordinator.invalidate();
      discoveryRequests.invalidate();
      activeDiscoveryRequest = undefined;
      activeWebviewSessionId = undefined;
      clearDiscoveryOriginContext();
      discoveryOriginContext.clearEndpoints();
      panel = undefined;
      currentStage = "welcome";
      deviceRequested = false;
      lastFailure = undefined;
      lastFailureAction = undefined;
      activeProtocol = "simulated";
      activeSchema = undefined;
      lastTopology = undefined;
      lastDisplayTopology = undefined;
      runtimeAuthority.reset();
      lastApplyResult = undefined;
      searchQuery = "";
      pinnedNodeId = undefined;
      pendingFocusNodeId = undefined;
      quickAddOpen = false;
      runtimeSetupMessage = undefined;
      polling.stop();
    });
    panel.onDidChangeViewState(({ webviewPanel }) => {
      const panelBecameVisible = becameVisible(
        wasVisible,
        webviewPanel.visible,
      );
      wasVisible = webviewPanel.visible;
      if (panelBecameVisible) {
        polling.start();
        postImmediateSimulatorLifecycleGraph(runtimeLifecycleService.phase());
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
        clearDiscoveryOriginContext();
        if (activeWebviewSessionId) {
          void webviewPanel.webview.postMessage({
            type: "discoverReset",
            sessionId: activeWebviewSessionId,
          });
        }
        void webviewPanel.webview.postMessage({ type: "browseReset" });
        void refreshNetworkCanvasPanel();
      } else if (!webviewPanel.visible) {
        polling.stop();
        refreshCoordinator.invalidate();
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
        clearDiscoveryOriginContext();
      }
    });
    panel.webview.onDidReceiveMessage((message: unknown) => {
      void handleWebviewMessage(message);
    });
    context.subscriptions.push(panel);
  }
  polling.start();
  void refreshNetworkCanvasPanel();
}

async function refreshNetworkCanvasPanel(): Promise<void> {
  const panelRef = panel;
  if (!panelRef || !panelRef.visible) {
    return;
  }
  await refreshCoordinator.request((context) =>
    refreshNetworkCanvasPanelOnce(panelRef, context),
  );
}

async function refreshNetworkCanvasPanelOnce(
  panelRef: vscode.WebviewPanel,
  refreshContext: LatestRefreshContext,
): Promise<void> {
  const refreshDelayMs = networkCanvasRefreshDelayMs();
  if (refreshDelayMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, refreshDelayMs));
    if (
      !refreshContext.isCurrent() ||
      panel !== panelRef ||
      !panelRef.visible
    ) {
      return;
    }
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  const rawAuthorityTarget = snapshot.starting
    ? snapshot.transitionTarget
    : snapshot.activeTarget;
  const workspaceResource = runtimeLifecycleService.runtimeConfigTarget();
  const runtime = await resolveNetworkCanvasRuntimeTarget(
    workspaceResource,
    runtimeLifecycleService.acceptedDebugSession()?.configuration,
  );

  // First paint must not wait for CLI schema/topology discovery. On a fresh
  // panel, publish the lifecycle-derived base graph immediately so users see
  // Simulator Stopped/Starting/Running instead of a 15-30 second Loading page.
  // The latest-only coordinator will replace this with the configured/live
  // topology below when those asynchronous sources finish.
  if (!activeSchema && !lastTopology) {
    if (
      !refreshContext.isCurrent() ||
      panel !== panelRef ||
      !panelRef.visible
    ) {
      return;
    }
    const baseModel = buildNetworkCanvasModel(
      modelInputForSnapshot(
        currentStage,
        snapshot,
        {
          lastFailure,
          lastFailureAction,
          deviceRequested,
        },
        {
          schema: undefined,
          activeProtocol,
          applyResult: lastApplyResult,
          searchQuery,
          pinnedNodeId,
          quickAddOpen,
          authorityTarget: rawAuthorityTarget,
        },
      ),
    );
    const baseAttachedEndpoint =
      snapshot.status.runtimeMode === "online" &&
      snapshot.status.runtimeState === "connected"
        ? snapshot.status.endpoint
        : undefined;
    const baseAuthorityTarget = runtimeAuthority.reconcile(
      rawAuthorityTarget,
      snapshot.status.targetLabel,
    );
    const authorityPending =
      rawAuthorityTarget !== undefined &&
      rawAuthorityTarget.kind !== "simulator" &&
      baseAuthorityTarget === undefined;
    const baseGraph = projectCanvasLifecycleAuthority(
      buildCanvasGraph(
        baseModel,
        undefined,
        undefined,
        baseAttachedEndpoint,
        [],
        getSelectedRuntimeId(),
      ),
      {
        phase: runtimeLifecycleService.phase(),
        target: baseAuthorityTarget,
      },
    );
    if (!authorityPending) {
      activeRuntimeTarget = runtime;
      discoveryOriginContext.updateEndpointRegistry(baseGraph, runtime);
      void panelRef.webview.postMessage({ type: "graph", graph: baseGraph });
      void panelRef.webview.postMessage({
        type: "meta",
        schema: undefined,
        applyResult: lastApplyResult,
        reachable: runtime.status === "online_reachable",
        lifecyclePhase: runtimeLifecycleService.phase(),
        operationInProgress:
          runtimeLifecycleService.operationState() !== undefined,
      });
    }
  }
  let capabilities: CommCapabilitiesResponse | undefined;
  let topologyError: string | undefined;
  let nextSchema: CommSchemaResponse | undefined;
  let nextRuntimeSetupMessage: string | undefined;
  let nextTopology: FleetTopologyResponse | undefined;
  let offlineTopology: FleetTopologyResponse | undefined;
  const projectDir = workspaceResource?.fsPath;

  // Offline-first: the protocol schema and the configured topology come from the trust-runtime
  // CLI (no running runtime), so the canvas + settings work whether the project is stopped or
  // online. (Returns undefined on an older binary without the `comm` subcommands → falls back.)
  if (extensionContext) {
    const [schema, topology] = await Promise.all([
      offlineCommSchema(extensionContext),
      projectDir
        ? offlineCommTopology(extensionContext, projectDir)
        : Promise.resolve(undefined),
    ]);
    nextSchema = schema;
    offlineTopology = topology;
    nextTopology = topology;
  }

  // Live overlay: when a runtime is reachable, prefer its live schema + topology (real status).
  if (runtime.status === "online_reachable" && runtime.endpoint) {
    try {
      capabilities = await sendRuntimeControlRequest<CommCapabilitiesResponse>(
        runtime.endpoint,
        runtime.authToken,
        "comm.capabilities",
        undefined,
        { timeoutMs: 2000 },
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
        await fetchConnectorStatus(runtime).catch(() => undefined),
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
    const peers = (
      await resolveNetworkCanvasFleetTargets(
        runtime,
        workspaceResource,
        fleetEndpointLabels,
      )
    ).filter(
      (target) => target.endpoint && target.endpoint !== runtime.endpoint,
    );
    if (peers.length > 0) {
      peerTopology =
        await fetchAndMergeFleetTopologiesWithConnectorStatus(peers);
    }
  } catch {
    // Peers are best-effort; the local view still renders without them.
  }

  const displayTopology = peerTopology
    ? mergeFleetTopologies([nextTopology, peerTopology])
    : nextTopology;

  const model = buildNetworkCanvasModel(
    modelInputForSnapshot(
      currentStage,
      snapshot,
      {
        lastFailure,
        lastFailureAction,
        deviceRequested,
      },
      {
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
        authorityTarget: rawAuthorityTarget,
      },
    ),
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
  lastDisplayTopology = displayTopology;
  const validatedAuthorityTarget = runtimeAuthority.acceptInventory(
    rawAuthorityTarget,
    managed,
    snapshot.status.targetLabel,
  );
  const canvasGraph = projectCanvasLifecycleAuthority(
    buildCanvasGraph(
      model,
      displayTopology,
      undefined,
      attachedEndpoint,
      managed,
      getSelectedRuntimeId(),
    ),
    {
      phase: runtimeLifecycleService.phase(),
      target: validatedAuthorityTarget,
    },
  );
  discoveryOriginContext.updateEndpointRegistry(canvasGraph, runtime);
  void panelRef.webview.postMessage({
    type: "graph",
    graph: canvasGraph,
  });
  void panelRef.webview.postMessage({
    type: "meta",
    schema: nextSchema,
    applyResult: lastApplyResult,
    reachable: runtime.status === "online_reachable",
    setupMessage: nextRuntimeSetupMessage,
    lifecyclePhase: runtimeLifecycleService.phase(),
    operationInProgress: runtimeLifecycleService.operationState() !== undefined,
  });
  if (pendingFocusNodeId) {
    void panelRef.webview.postMessage({
      type: "focusNode",
      nodeId: pendingFocusNodeId,
    });
    pendingFocusNodeId = undefined;
  }
}

function rawLifecycleAuthorityTarget(
  phase: "stopped" | "starting" | "running" | "connected",
) {
  return phase === "starting"
    ? runtimeLifecycleService.transitionTarget()
    : runtimeLifecycleService.activeTarget();
}

function postImmediateSimulatorLifecycleGraph(
  phase: "stopped" | "starting" | "running" | "connected",
): void {
  const panelRef = panel;
  if (!panelRef?.visible) {
    return;
  }
  const graph = buildImmediateSimulatorLifecycleGraph({
    phase,
    stage: currentStage,
    lastFailure,
    lastFailureAction,
    localFailure: runtimeLifecycleService.localFailure(),
    schema: activeSchema,
    activeProtocol,
    applyResult: lastApplyResult,
    searchQuery,
    pinnedNodeId,
    quickAddOpen,
    topology: lastDisplayTopology,
    managedRuntimes: runtimeAuthority.managedRuntimes(),
    selectedRuntimeId: getSelectedRuntimeId(),
    deviceRequested,
    authorityTarget: runtimeAuthority.lifecycleProjectionTarget(),
  });
  if (graph) {
    void panelRef.webview.postMessage({ type: "graph", graph });
  }
  void panelRef.webview.postMessage({
    type: "lifecyclePolicy",
    lifecyclePhase: phase,
    operationInProgress: runtimeLifecycleService.operationState() !== undefined,
  });
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
    origin: envelope.request.origin,
  };
  clearDiscoveryOriginContext();
  const discoveryRuntimeTarget =
    await discoveryOriginContext.resolveDiscoveryTarget(
      envelope.request.origin,
      envelope.request.originEndpoint,
      activeRuntimeTarget,
    );
  if (!discoveryRequests.isCurrent(token, panelRef)) {
    return;
  }
  discoveryOriginContext.pin(envelope.request.origin, discoveryRuntimeTarget);
  await runNetworkCanvasDiscovery(envelope, {
    panel: panelRef,
    extensionContext: contextRef,
    runtimeTarget: discoveryRuntimeTarget,
    tracker: discoveryRequests,
    token,
  });
}

async function handleWebviewMessage(message: unknown): Promise<void> {
  if (!isRecord(message)) {
    return;
  }
  if (await lifecycleActions.handleMessage(message)) {
    return;
  }
  switch (message.type) {
    case "ready":
      discoveryRequests.invalidate();
      activeDiscoveryRequest = undefined;
      clearDiscoveryOriginContext();
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
    case "addSimulatedDevice":
      currentStage = "connected";
      deviceRequested = true;
      {
        const result = await runtimeLifecycleService.requestIoState();
        lastFailure = result.ok ? undefined : result.failure;
        lastFailureAction = result.ok ? undefined : "other";
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
            "That device protocol isn't available yet.",
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
    case "probeAdsServices":
      if (
        !isCurrentAdsServiceProbeRequest(
          message,
          activeDiscoveryRequest,
          activeWebviewSessionId,
        )
      ) {
        return;
      }
      await adsServiceProbeController.probe(message);
      break;
    case "handoffDiscoveryToBrowse":
      if (
        discoveryOriginContext.handoffToBrowse(
          activeDiscoveryRequest,
          message,
          activeWebviewSessionId,
        )
      ) {
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
      }
      break;
    case "cancelDiscover":
      if (
        activeDiscoveryRequest &&
        message.sessionId === activeDiscoveryRequest.sessionId &&
        message.requestId === activeDiscoveryRequest.requestId
      ) {
        discoveryRequests.invalidate();
        activeDiscoveryRequest = undefined;
        clearDiscoveryOriginContext();
      }
      break;
    case "releaseDiscoveryOrigin":
      discoveryOriginContext.releaseBrowse(
        message.originRuntimeId,
        message.leaseId,
        message.browseSessionId,
      );
      break;
    case "browseSymbols":
      await protocolActions.browseSymbols(message);
      break;
    case "createRoute":
      // The browse pane owns the visible recovery instructions. Keep this notification aligned with
      // that in-canvas flow; do not send users to retired ADS panels.
      await vscode.window.showInformationMessage(
        "Run the generated ADS route PowerShell as Administrator on the remote ADS device, then select Retry browse.",
      );
      break;
    case "addTags":
      try {
        await protocolActions.addTags(message);
      } finally {
        const target = isRecord(message.target) ? message.target : {};
        discoveryOriginContext.releaseBrowse(
          target.discovery_origin_runtime_id,
          target.discovery_origin_lease_id,
          message.browseSessionId,
        );
      }
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
          lastFailureAction = undefined;
        }
        await refreshNetworkCanvasPanel();
      }
      break;
  }
}

function normalizeCanvasProtocol(
  value: unknown,
): NetworkCanvasProtocolId | undefined {
  const normalized = normalizeProtocolId(value);
  return normalized &&
    NETWORK_CANVAS_IO_PROTOCOLS.includes(normalized as NetworkCanvasProtocolId)
    ? (normalized as NetworkCanvasProtocolId)
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
