import * as vscode from "vscode";
import * as path from "path";
import { randomUUID } from "crypto";

import { debugChannel } from "../debug/configuration";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import {
  applyCommSetup,
  clientErrorResult,
  normalizeProtocolId,
  testCommSetup,
} from "../communication/runtimeComm";
import {
  openRuntimePane,
  resolveRuntimeTarget,
  type RuntimeTarget,
} from "../runtimeTarget";
import { RuntimeControlError, sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  runtimeLifecycleService,
  type RuntimeStartFailure,
} from "../runtimeLifecycle";
import { setSelectedRuntimeId } from "../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../trustHomeModel";
import {
  onDidChangeManagedRuntimes,
  showManagedRuntimeLogs,
  startManagedRuntime,
  stopManagedRuntime,
} from "../localRuntime";
import {
  attachManagedRuntimeAfterStart,
  disconnectManagedRuntimeAfterStop,
} from "../managedRuntimeSession";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  isNetworkCanvasStage,
  NETWORK_CANVAS_IO_PROTOCOLS,
  nextNetworkCanvasStage,
  type NetworkCanvasProtocolId,
  type NetworkCanvasStage,
} from "./model";
import {
  ensureAdsRuntimeEnabled,
  offlineAdsImportSymbols,
  offlineCommApply,
  offlineCommDiscover,
  offlineCommSchema,
  offlineCommTopology,
  openGeneratedAdsDocuments,
  type AdsImportSymbolsReport,
  type DiscoverCandidate,
} from "./offlineComm";
import {
  loadCanvasData,
  removeProjectPlant,
  saveProjectPlant,
} from "./canvasDataService";
import {
  addFleetHost as persistFleetHost,
  addFleetRuntime as scaffoldFleetRuntime,
} from "./fleetConfiguration";
import { networkCanvasWebviewHtml } from "./webviewHtml";
import { countLabel, discoverLabel } from "./canvasLabels";
import {
  decodeCanvasMessage,
  decodeOpenControlStudioMessage,
} from "./canvasMessageRouter";
import { CONTROL_STUDIO_OPEN_COMMAND } from "../controlStudio/panel";
import { normalizeHmiBindingsCatalog } from "../lm-tools/shared/hmi/core";
import {
  buildPlantSignalCatalog,
  type PlantSignalCatalog,
} from "./plant/signalCatalog";
import { decodePlantContractError } from "./plant/contract";
import { plantSaveResultLabel } from "./plant/service";
import {
  addOfflineEthercatChannels,
  addOfflineExpose,
  addOfflineOpcuaConnection,
  browseOfflineSymbols,
  type OfflineCanvasActionContext,
} from "./offlineCanvasActions";

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
let getLanguageClient:
  | (() => { sendRequest(method: string, params: unknown): Promise<unknown> } | undefined)
  | undefined;
let compiledBindings: unknown;
let compiledBindingsRoot: string | undefined;
const fleetEndpointLabels = new Map<string, string>();

export function registerNetworkCanvasPanel(
  context: vscode.ExtensionContext,
  deps: {
    getClient: () =>
      | { sendRequest(method: string, params: unknown): Promise<unknown> }
      | undefined;
  }
): void {
  getLanguageClient = deps.getClient;
  context.subscriptions.push(
    vscode.commands.registerCommand(NETWORK_CANVAS_COMMAND, async (request?: unknown) => {
      if (isRecord(request) && typeof request.focusNodeId === "string") {
        pendingFocusNodeId = request.focusNodeId;
        pinnedNodeId = request.focusNodeId;
      }
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
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (document.languageId !== "structured-text") return;
      compiledBindings = undefined;
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
      pendingFocusNodeId = undefined;
      quickAddOpen = false;
      runtimeSetupMessage = undefined;
      stopPolling();
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
  if (!panelRef) {
    return;
  }
  const refreshDelayMs = networkCanvasRefreshDelayMs();
  if (refreshDelayMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, refreshDelayMs));
    if (!panel || panel !== panelRef) {
      return;
    }
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  const data = await loadCanvasData({
    context: extensionContext,
    stage: currentStage,
    snapshot,
    hasStructuredTextSession: Boolean(runtimeLifecycleService.getStructuredTextSession()),
    activeProtocol,
    applyResult: lastApplyResult,
    searchQuery,
    pinnedNodeId,
    quickAddOpen,
    lastFailure,
    deviceRequested,
    fleetEndpointLabels,
  });
  const plantSignals = await currentPlantSignals(snapshot.ioState, data.schema, data.topology);
  activeRuntimeTarget = data.runtime;
  activeSchema = data.schema;
  lastTopology = data.topology;
  runtimeSetupMessage = data.setupMessage;
  if (panel !== panelRef) {
    return;
  }
  void panelRef.webview.postMessage({
    type: "graph",
    graph: data.graph,
  });
  void panelRef.webview.postMessage({
    type: "meta",
    schema: activeSchema,
    applyResult: lastApplyResult,
    reachable: data.reachable,
    setupMessage: runtimeSetupMessage,
    plantSignals,
  });
  if (pendingFocusNodeId) {
    void panelRef.webview.postMessage({
      type: "focusNode",
      nodeId: pendingFocusNodeId,
    });
    pendingFocusNodeId = undefined;
  }
}

async function currentPlantSignals(
  live: import("../io-panel/types").IoState,
  communicationSchema?: CommSchemaResponse,
  communicationTopology?: import("./fleetTopology").FleetTopologyResponse
): Promise<PlantSignalCatalog> {
  const rootUri = vscode.workspace.workspaceFolders?.[0]?.uri.toString();
  const client = getLanguageClient?.();
  if (!rootUri || !client) {
    return buildPlantSignalCatalog(
      { ok: false, error: "Compiled PLC signals are not available yet." },
      live,
      communicationSchema,
      communicationTopology
    );
  }
  if (compiledBindings === undefined || compiledBindingsRoot !== rootUri) {
    compiledBindingsRoot = rootUri;
    try {
      compiledBindings = await client.sendRequest("workspace/executeCommand", {
        command: "trust-lsp.hmiBindings",
        arguments: [{ root_uri: rootUri }],
      });
    } catch (error) {
      compiledBindings = {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }
  const response = compiledBindings as { ok?: boolean } | undefined;
  return buildPlantSignalCatalog(
    response?.ok === true ? normalizeHmiBindingsCatalog(compiledBindings) : compiledBindings,
    live,
    communicationSchema,
    communicationTopology
  );
}

async function addFleetHost(message: Record<string, unknown>): Promise<void> {
  const result = await persistFleetHost(message, fleetEndpointLabels);
  if (result.focusNodeId) {
    pendingFocusNodeId = result.focusNodeId;
    pinnedNodeId = result.focusNodeId;
  }
  await refreshNetworkCanvasPanel();
}

async function addFleetRuntime(message: Record<string, unknown>): Promise<void> {
  await scaffoldFleetRuntime(extensionContext, message);
  await refreshNetworkCanvasPanel();
}

// §0.5 Discover: run `comm.discover` for each selected protocol IN SEQUENCE (clear per-row
// progress), then post the combined candidates. Degrades gracefully if the verb isn't there yet.
async function handleDiscover(message: Record<string, unknown>): Promise<void> {
  if (!panel || !extensionContext) {
    return;
  }
  const request = isRecord(message.request) ? message.request : {};
  const origin = typeof request.origin === "string" ? request.origin : "this_host";
  const items = Array.isArray(request.items) ? request.items : [];
  // origin "this_host" → CLI on this machine (this-host). Otherwise scan FROM the runtime — the
  // control verb on a reachable runtime (Codex: origin=runtime is a control verb). The candidate
  // carries no protocol, so we stamp it for the UI's "+ Add".
  const viaRuntime =
    origin !== "this_host" &&
    activeRuntimeTarget?.status === "online_reachable" &&
    Boolean(activeRuntimeTarget.endpoint);
  const all: DiscoverCandidate[] = [];
  for (const raw of items) {
    if (!isRecord(raw) || typeof raw.protocol !== "string") {
      continue;
    }
    const protocol = raw.protocol;
    const cidr = typeof raw.cidr === "string" ? raw.cidr : undefined;
    const host = typeof raw.host === "string" ? raw.host : undefined;
    const label = discoverLabel(protocol, host, cidr);
    void panel.webview.postMessage({ type: "discoverProgress", protocol, label, status: "scanning" });
    let candidates: DiscoverCandidate[] = [];
    if (viaRuntime && activeRuntimeTarget?.endpoint) {
      const res = await sendRuntimeControlRequest<{ candidates?: DiscoverCandidate[] }>(
        activeRuntimeTarget.endpoint,
        activeRuntimeTarget.authToken,
        "comm.discover",
        { protocol, origin: "runtime", scope: { cidr, host } },
        { timeoutMs: 8000 }
      ).catch(() => undefined);
      candidates = res?.candidates ?? [];
    } else {
      const res = await offlineCommDiscover(extensionContext, protocol, "this-host", { cidr, host });
      candidates = res?.candidates ?? [];
    }
    const stamped = candidates.map((c) => ({ ...c, protocol }));
    all.push(...stamped);
    void panel.webview.postMessage({ type: "discoverProgress", protocol, label, status: "done", count: stamped.length });
  }
  void panel.webview.postMessage({ type: "discoverResults", candidates: all });
}

// §0.5.2 browse a target's symbol tree (ADS first). Reports route_missing so the UI can offer the
// "Create route" fix. Graceful if the verb isn't there yet.
async function handleBrowseSymbols(message: Record<string, unknown>): Promise<void> {
  await browseOfflineSymbols(offlineActionContext(), message);
}

async function handleAddExpose(message: Record<string, unknown>): Promise<void> {
  await addOfflineExpose(offlineActionContext(), message);
}

function networkCanvasRefreshDelayMs(): number {
  const value = Number(process.env.TRUST_VSCODE_NETWORK_CANVAS_REFRESH_DELAY_MS ?? 0);
  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }
  return Math.min(Math.floor(value), 10_000);
}

// opcua_client: save a browsed connection (endpoint + chosen security/auth + selected node points,
// each with its real OPC-UA NodeId) to opcua_client.toml via comm.apply. Honest lifecycle — a file
// write never claims the connection is live; the runtime turns it green only on real reads.
async function handleAddOpcuaConnection(message: Record<string, unknown>): Promise<void> {
  await addOfflineOpcuaConnection(offlineActionContext(), message);
}

// EtherCAT "Browse channels" selects PDO channel paths from the configured modules and writes that
// selection back to the EtherCAT driver params. It is NOT an ADS import/tag pipeline.
async function handleAddEthercatChannels(message: Record<string, unknown>): Promise<void> {
  await addOfflineEthercatChannels(offlineActionContext(), message);
}

function offlineActionContext(): OfflineCanvasActionContext {
  return {
    extensionContext,
    projectDir: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    topology: lastTopology,
    postMessage: (message) => void panel?.webview.postMessage(message),
    refresh: refreshNetworkCanvasPanel,
  };
}

// §0.5 ADS "Add tags": write the selected symbols through the existing ADS import pipeline
// (ads.toml points + cached snapshot + generated ST). Prefer a reachable runtime's
// ads.import_symbols.apply control verb; otherwise use the same deterministic local CLI import
// from the extension host so a stopped project can still complete the authoring flow.
async function handleAddTags(message: Record<string, unknown>): Promise<void> {
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((p): p is string => typeof p === "string")
    : [];
  if (paths.length === 0) {
    return;
  }
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!projectDir) {
    await vscode.window.showWarningMessage(
      "Add tags needs an open project so truST can write ads.toml and generated ST."
    );
    return;
  }
  if (
    !activeRuntimeTarget ||
    activeRuntimeTarget.status !== "online_reachable" ||
    !activeRuntimeTarget.endpoint
  ) {
    const protocol = typeof message.protocol === "string" ? message.protocol : "";
    if (protocol === "ads" && extensionContext && projectDir) {
      const src = isRecord(message.target) ? message.target : {};
      const report = await offlineAdsImportSymbols(
        extensionContext,
        projectDir,
        src,
        paths,
        Boolean(message.writable)
      );
      if (report.applied) {
        await vscode.window.showInformationMessage(report.message);
        await refreshNetworkCanvasPanel();
      } else {
        await vscode.window.showWarningMessage(`Could not add ADS tags: ${report.message}`);
      }
      return;
    }
    await vscode.window.showWarningMessage(
      "Add tags needs a reachable runtime — it writes ads.toml + the generated ST through the runtime's ADS import pipeline."
    );
    return;
  }
  // TargetIdentity wants ip/ams_net_id; the ADS endpoint params use host/target_net_id — map them.
  const src = isRecord(message.target) ? message.target : {};
  const target: Record<string, unknown> = {
    name: typeof src.name === "string" ? src.name : undefined,
    ip: typeof src.host === "string" ? src.host : src.ip,
    ams_net_id: typeof src.target_net_id === "string" ? src.target_net_id : src.ams_net_id,
    ams_port: typeof src.ams_port === "number" ? src.ams_port : 851,
    tc_version: src.tc_version,
  };
  const connectionName =
    typeof src.name === "string" && src.name.trim().length > 0 ? src.name : "ads_import";
  try {
    const report = await sendRuntimeControlRequest<AdsImportSymbolsReport>(
      activeRuntimeTarget.endpoint,
      activeRuntimeTarget.authToken,
      "ads.import_symbols.apply",
      {
        connection_name: connectionName,
        symbols: paths,
        target,
        write_acknowledged: Boolean(message.writable),
      },
      { timeoutMs: 20_000 }
    );
    if (report?.applied) {
      const runtimeConfig = ensureAdsRuntimeEnabled(projectDir);
      if (!runtimeConfig.ok) {
        await vscode.window.showWarningMessage(
          `Added ADS tags, but ADS runtime was not enabled automatically: ${runtimeConfig.message}`
        );
        await refreshNetworkCanvasPanel();
        return;
      }
      const count = report.selected_count ?? paths.length;
      await openGeneratedAdsDocuments(report);
      await vscode.window.showInformationMessage(
        `Added ${countLabel(count, "ADS tag")}. Restart the runtime to apply the generated ST symbols.`
      );
      await refreshNetworkCanvasPanel();
    } else {
      await vscode.window.showWarningMessage(
        `Could not add tags: ${report?.message ?? "the runtime rejected the import."}`
      );
    }
  } catch (error) {
    await vscode.window.showWarningMessage(
      `Could not add tags: ${
        error instanceof Error ? error.message : String(error)
      } (live ADS import needs an ads-wire runtime build).`
    );
  }
}

async function handleWebviewMessage(value: unknown): Promise<void> {
  const message = decodeCanvasMessage(value);
  if (!message) {
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
      await applyNetworkCanvasSetup(message);
      break;
    case "commSave":
      // Editable inspector: write config to disk (works stopped/offline).
      await saveNetworkCanvasSetup(message, "upsert");
      break;
    case "commRemove":
      await saveNetworkCanvasSetup(message, "remove");
      break;
    case "commDisable":
      await saveNetworkCanvasSetup(message, "disable");
      break;
    case "commApplyLive":
      // Explicit "push to the running runtime now" — control channel, online only.
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
    case "clearApplyResult":
      lastApplyResult = undefined;
      await refreshNetworkCanvasPanel();
      break;
    case "addHost":
      await addFleetHost(message);
      break;
    case "addRuntime":
      await addFleetRuntime(message);
      break;
    case "discover":
      await handleDiscover(message);
      break;
    case "browseSymbols":
      await handleBrowseSymbols(message);
      break;
    case "createRoute":
      // The browse pane owns the visible recovery instructions. Keep this notification aligned with
      // that in-canvas flow; do not send users to retired ADS panels.
      await vscode.window.showInformationMessage(
        "Run the generated ADS route PowerShell as Administrator on the TwinCAT computer, then reopen Browse."
      );
      break;
    case "addTags":
      await handleAddTags(message);
      break;
    case "addEthercatChannels":
      await handleAddEthercatChannels(message);
      break;
    case "addExpose":
      await handleAddExpose(message);
      break;
    case "addOpcuaConnection":
      await handleAddOpcuaConnection(message);
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
    case "plantSave":
      await handlePlantSave(message);
      break;
    case "plantRemove":
      await handlePlantRemove(message);
      break;
    case "openControlStudio":
      {
        const request = decodeOpenControlStudioMessage(message);
        if (!request) return;
        await vscode.commands.executeCommand(CONTROL_STUDIO_OPEN_COMMAND, {
          schemaVersion: 1,
          handoffId: randomUUID(),
          projectId: request.projectId,
          runtimeId: request.runtimeId,
          plantId: request.plantId,
          savedConfigurationRevision: request.savedConfigurationRevision,
          ...(request.activeConfigurationDigest
            ? { activeConfigurationDigest: request.activeConfigurationDigest }
            : {}),
          ...(request.loopId ? { loopId: request.loopId } : {}),
          source: "Simulation",
          requestedContext: { mode: "Monitor", selection: "plant_summary" },
        });
      }
      break;
  }
}

async function handlePlantSave(message: Record<string, unknown>): Promise<void> {
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const candidate = isRecord(message.candidate) ? message.candidate : undefined;
  const runtimeId = typeof message.runtimeId === "string" ? message.runtimeId : "";
  const expectedRevision = typeof message.expectedRevision === "string"
    ? message.expectedRevision
    : undefined;
  const plantId = candidate && typeof candidate.id === "string" ? candidate.id : "";
  if (!panel || !extensionContext || !projectDir || !candidate || !runtimeId || !plantId) return;
  try {
    const saved = await saveProjectPlant(
      extensionContext,
      projectDir,
      candidate,
      `vscode-plant-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      expectedRevision
    );
    void vscode.window.showInformationMessage(plantSaveResultLabel(saved));
    const nodeId = `plant:${runtimeId}:${plantId}`;
    pendingFocusNodeId = nodeId;
    await refreshNetworkCanvasPanel();
    void panel?.webview.postMessage({
      type: "plantSaveResult",
      ok: true,
      nodeId,
    });
  } catch (error) {
    if (error instanceof RuntimeControlError) {
      const decoded = decodePlantContractError({ code: error.code, result: error.result });
      if (decoded.kind === "accepted_error" && decoded.code === "revision_conflict") {
        await refreshNetworkCanvasPanel();
        void panel?.webview.postMessage({
          type: "plantSaveResult",
          ok: false,
          code: decoded.code,
          message: "The saved plant changed since this editor was opened.",
          currentRevision: decoded.message.configuration_revision,
          currentPlant: decoded.message.current_plant,
        });
        return;
      }
    }
    void panel.webview.postMessage({
      type: "plantSaveResult",
      ok: false,
      message: error instanceof Error ? error.message : String(error),
    });
  }
}

async function handlePlantRemove(message: Record<string, unknown>): Promise<void> {
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const runtimeId = typeof message.runtimeId === "string" ? message.runtimeId : "";
  const plantId = typeof message.plantId === "string" ? message.plantId : "";
  const expectedRevision = typeof message.expectedRevision === "string"
    ? message.expectedRevision
    : "";
  if (!panel || !extensionContext || !projectDir || !runtimeId || !plantId || !expectedRevision) {
    return;
  }
  try {
    await removeProjectPlant(
      extensionContext,
      projectDir,
      plantId,
      `vscode-plant-remove-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      expectedRevision
    );
    pendingFocusNodeId = runtimeId;
    await refreshNetworkCanvasPanel();
    void panel?.webview.postMessage({
      type: "plantRemoveResult",
      ok: true,
      runtimeId,
    });
  } catch (error) {
    void panel.webview.postMessage({
      type: "plantRemoveResult",
      ok: false,
      message: error instanceof Error ? error.message : String(error),
    });
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

// Settings UX: Save/Remove write the project's config files via the offline CLI — no running
// runtime required. The change takes effect on next start; "Apply to running runtime" pushes
// it live separately.
async function saveNetworkCanvasSetup(
  message: Record<string, unknown>,
  action: "upsert" | "remove" | "disable"
): Promise<void> {
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const protocol = typeof message.protocol === "string" ? message.protocol : undefined;
  if (!extensionContext || !projectDir || !protocol) {
    return;
  }
  const params = isRecord(message.params) ? message.params : {};
  const result = await offlineCommApply(extensionContext, projectDir, protocol, params, action);
  if (result) {
    const topology = result.applied
      ? await offlineCommTopology(extensionContext, projectDir)
      : undefined;
    lastApplyResult = {
      ...result,
      instance_id:
        result.instance_id ??
        (topology && action !== "remove"
          ? findSavedEndpointId(topology, protocol, params)
          : undefined),
    };
  }
  await refreshNetworkCanvasPanel();
}

function findSavedEndpointId(
  topology: FleetTopologyResponse,
  protocol: string,
  params: Record<string, unknown>
): string | undefined {
  const protocolMatches: Array<{ id: string; params?: Record<string, unknown> }> = [];
  for (const host of topology.hosts ?? []) {
    const runtimes = [
      ...(host.runtimes ?? []),
      ...(host.containers ?? []).flatMap((container) => container.runtimes ?? []),
    ];
    for (const runtime of runtimes) {
      for (const endpoint of runtime.endpoints ?? []) {
        if (endpoint.protocol !== protocol) {
          continue;
        }
        protocolMatches.push({ id: endpoint.id, params: endpoint.params });
      }
    }
  }
  const exact = protocolMatches.filter((endpoint) =>
    endpoint.params ? paramsMatch(endpoint.params, params) : false
  );
  return (lastItem(exact) ?? lastItem(protocolMatches))?.id;
}

function lastItem<T>(items: readonly T[]): T | undefined {
  return items.length > 0 ? items[items.length - 1] : undefined;
}

function paramsMatch(
  endpointParams: Record<string, unknown>,
  submittedParams: Record<string, unknown>
): boolean {
  for (const [key, value] of Object.entries(submittedParams)) {
    if (!(key in endpointParams)) {
      continue; // Secret/redacted fields are intentionally absent from topology.
    }
    if (stableParamValue(endpointParams[key]) !== stableParamValue(value)) {
      return false;
    }
  }
  return true;
}

function stableParamValue(value: unknown): string {
  return JSON.stringify(normalizeParamValue(value)) ?? "undefined";
}

function normalizeParamValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(normalizeParamValue);
  }
  if (isRecord(value)) {
    return Object.keys(value)
      .sort()
      .reduce<Record<string, unknown>>((acc, key) => {
        acc[key] = normalizeParamValue(value[key]);
        return acc;
      }, {});
  }
  return value;
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

function normalizeCanvasProtocol(value: unknown): NetworkCanvasProtocolId | undefined {
  const normalized = normalizeProtocolId(value);
  return normalized &&
    NETWORK_CANVAS_IO_PROTOCOLS.includes(normalized as NetworkCanvasProtocolId)
    ? (normalized as NetworkCanvasProtocolId)
    : undefined;
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
