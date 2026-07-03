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
import { getControlAuthToken, setControlAuthToken } from "../runtimeAuth";
import { localSimControl } from "../simControl";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  runtimeLifecycleService,
  type RuntimeLifecycleSnapshot,
  type RuntimeStartFailure,
} from "../runtimeLifecycle";
import { setSelectedRuntimeId } from "../selectedRuntime";
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
  fetchAndMergeFleetTopologies,
  fetchFleetTopology,
  mergeFleetTopologies,
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
import {
  offlineBrowseSymbols,
  offlineCommApply,
  offlineCommDiscover,
  offlineCommSchema,
  offlineCommTopology,
  offlineFleetRuntimeAdd,
  type DiscoverCandidate,
} from "./offlineComm";
import { buildExposeApplyParams } from "./exposeConfig";

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

function protocolDisplayName(protocol: string): string {
  switch (protocol) {
    case "ads":
      return "ADS client";
    case "ads_server":
      return "ADS server";
    case "opcua":
      return "OPC UA server";
    case "opcua_client":
      return "OPC UA client";
    case "modbus_tcp":
      return "Modbus TCP";
    default:
      return protocol.replace(/_/g, " ");
  }
}

function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

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
        label: "Local simulator",
      });
      if (simTarget.status === "online_reachable") {
        runtime = simTarget;
      }
    }
  }
  activeRuntimeTarget = runtime;
  let capabilities: CommCapabilitiesResponse | undefined;
  let topologyError: string | undefined;
  activeSchema = undefined;
  runtimeSetupMessage = undefined;
  lastTopology = undefined;
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

  // Offline-first: the protocol schema and the configured topology come from the trust-runtime
  // CLI (no running runtime), so the canvas + settings work whether the project is stopped or
  // online. (Returns undefined on an older binary without the `comm` subcommands → falls back.)
  if (extensionContext) {
    activeSchema = await offlineCommSchema(extensionContext);
    if (projectDir) {
      lastTopology = await offlineCommTopology(extensionContext, projectDir);
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
        activeSchema = liveSchema;
      }
    } catch (error) {
      if (!activeSchema) {
        runtimeSetupMessage =
          error instanceof Error ? error.message : String(error);
      }
    }
    try {
      // The local primary's own live topology (real status). Peers are resolved separately below.
      const liveTopology = await fetchFleetTopology(runtime);
      if (liveTopology) {
        lastTopology = liveTopology;
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
      peerTopology = await fetchAndMergeFleetTopologies(peers);
    }
  } catch {
    // Peers are best-effort; the local view still renders without them.
  }

  const displayTopology = peerTopology
    ? mergeFleetTopologies([lastTopology, peerTopology])
    : lastTopology;

  const model = buildNetworkCanvasModel(
    modelInputForSnapshot(currentStage, snapshot, {
      schema: activeSchema,
      capabilities,
      activeProtocol,
      applyResult: lastApplyResult,
      searchQuery,
      pinnedNodeId,
      quickAddOpen,
      topology: displayTopology,
      topologyError,
      runtimeSetupMessage,
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
  if (panel !== panelRef) {
    return;
  }
  void panelRef.webview.postMessage({
    type: "graph",
    graph: buildCanvasGraph(model, displayTopology, undefined, attachedEndpoint, managed),
  });
  void panelRef.webview.postMessage({
    type: "meta",
    schema: activeSchema,
    applyResult: lastApplyResult,
    reachable: runtime.status === "online_reachable",
    setupMessage: runtimeSetupMessage,
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

// Add-host (§0.4): client-side fleet membership — append the runtime's control endpoint to the
// `trust-lsp.runtime.fleetEndpoints` setting so resolveFleetTargets() fetches + merges it.
async function addFleetHost(message: Record<string, unknown>): Promise<void> {
  const endpoint = normalizeFleetControlEndpoint(
    typeof message.endpoint === "string" ? message.endpoint.trim() : ""
  );
  if (!endpoint) {
    return;
  }
  const authToken =
    typeof message.authToken === "string" ? message.authToken.trim() : "";
  const label =
    typeof message.label === "string" ? message.label.trim() : "";
  if (authToken) {
    await setControlAuthToken(endpoint, authToken);
  }
  if (label) {
    fleetEndpointLabels.set(endpoint, label);
  }
  if (!authToken) {
    pendingFocusNodeId = fleetRuntimeNodeId(endpoint);
    pinnedNodeId = pendingFocusNodeId;
  }
  const config = trustConfig();
  const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
  if (current.includes(endpoint)) {
    await vscode.window.showInformationMessage(`${endpoint} is already in the fleet.`);
    await refreshNetworkCanvasPanel();
    return;
  }
  const target = vscode.workspace.workspaceFolders?.length
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
  await config.update("runtime.fleetEndpoints", [...current, endpoint], target);
  // The canvas itself is the success surface: the runtime node appears/focuses there, and a global
  // VS Code toast can cover the graph exactly when the user needs to inspect the result.
  await refreshNetworkCanvasPanel();
}

function normalizeFleetControlEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (
    trimmed.startsWith("tcp://") ||
    trimmed.startsWith("unix://") ||
    trimmed.length === 0
  ) {
    return trimmed;
  }
  if (/^[^/\s:]+:\d+$/.test(trimmed) || /^\[[^\]]+\]:\d+$/.test(trimmed)) {
    return `tcp://${trimmed}`;
  }
  return trimmed;
}

function fleetRuntimeNodeId(endpoint: string): string {
  return `fleet:${endpoint}:runtime`;
}

function workspaceConfigResource(): vscode.Uri | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri;
}

function trustConfig(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("trust-lsp", workspaceConfigResource());
}

// Add-runtime (§0.4): scaffold a sibling runtime PROJECT via `trust-runtime fleet runtime add`
// (offline), then track its control endpoint in the fleet view. It appears once started.
async function addFleetRuntime(message: Record<string, unknown>): Promise<void> {
  const name = typeof message.name === "string" ? message.name.trim() : "";
  const template = message.template === "empty" ? "empty" : "simulate";
  if (!name || !extensionContext) {
    return;
  }
  const fleetRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!fleetRoot) {
    await vscode.window.showWarningMessage("Open a workspace folder to add a runtime.");
    return;
  }
  const result = await offlineFleetRuntimeAdd(extensionContext, fleetRoot, name, template);
  if (!result) {
    await vscode.window.showWarningMessage(
      `Could not create runtime "${name}" (it may already exist, or needs a newer trust-runtime).`
    );
    return;
  }
  const config = trustConfig();
  const current = config.get<string[]>("runtime.fleetEndpoints", []) ?? [];
  if (!current.includes(result.control_endpoint)) {
    const target = vscode.workspace.workspaceFolders?.length
      ? vscode.ConfigurationTarget.Workspace
      : vscode.ConfigurationTarget.Global;
    await config.update("runtime.fleetEndpoints", [...current, result.control_endpoint], target);
  }
  await vscode.window.showInformationMessage(
    `Created runtime "${result.name}" at ${result.path} (${result.control_endpoint}). Start it to see it on the canvas.`
  );
  await refreshNetworkCanvasPanel();
}

function discoverLabel(protocol: string, host?: string, cidr?: string): string {
  const label = discoverProtocolName(protocol);
  if (host) {
    return `${label} @ ${host}`;
  }
  if (cidr) {
    return `${label} ${cidr}`;
  }
  return label;
}

function discoverProtocolName(protocol: string): string {
  switch (protocol) {
    case "ads":
      return "TwinCAT (ADS)";
    case "discovery":
      return "truST runtimes";
    case "modbus_tcp":
      return "Modbus";
    case "opcua_client":
      return "OPC UA server";
    case "mqtt":
      return "MQTT broker";
    case "ethercat":
      return "EtherCAT";
    case "gpio":
      return "GPIO";
    default:
      return protocol.replace(/_/g, " ");
  }
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
  if (!panel || !extensionContext) {
    return;
  }
  const protocol = typeof message.protocol === "string" ? message.protocol : "ads";
  const target = isRecord(message.target) ? message.target : {};
  const kind = message.kind === "channels" || message.kind === "nodes" ? message.kind : "symbols";
  const connectionName = typeof target.name === "string" ? target.name : undefined;
  // Local "expose globals" (opcua_server/ads_server/openot) + EtherCAT channels read from the
  // project files offline; ADS remote browse uses the target instead.
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const result = await offlineBrowseSymbols(
    extensionContext,
    protocol,
    target,
    kind,
    connectionName,
    projectDir
  );
  void panel.webview.postMessage({
    type: "symbolTree",
    tree: result?.tree ?? [],
    routeMissing: result?.route?.status === "missing",
    // §0.5.2: the route_plan carries ready-to-run AMS route setup scripts (no credentials) so the
    // canvas can show "Create route" instructions for the user to run on the TwinCAT.
    routePlan: result?.route?.route_plan,
    // opcua_client: structured {code,message} failure → the canvas maps it to one recovery action
    // (esp. the explicit cert-trust path). No secrets are included.
    error: result?.error,
  });
}

// Find the configured params for a protocol's endpoint in the current topology (used to re-apply a
// server's FULL config when only one field changes — comm.apply validates the whole config, no merge).
function findEndpointParams(protocol: string): Record<string, unknown> | undefined {
  for (const host of lastTopology?.hosts ?? []) {
    const runtimes = [
      ...(host.runtimes ?? []),
      ...(host.containers ?? []).flatMap((c) => c.runtimes ?? []),
    ];
    for (const rt of runtimes) {
      for (const ep of rt.endpoints ?? []) {
        if (ep.protocol === protocol && isRecord(ep.params)) {
          return ep.params;
        }
      }
    }
  }
  return undefined;
}

// §0.5 "Expose globals": add the picked truST globals to an OPC UA / ADS server's expose[] (and
// writable[] when allowed). comm.apply needs the full config, so we re-apply the endpoint's current
// params with the merged expose list. Globals are exposed by NAME (expose globs match global names).
async function handleAddExpose(message: Record<string, unknown>): Promise<void> {
  const protocol = typeof message.protocol === "string" ? message.protocol : "";
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((p): p is string => typeof p === "string")
    : [];
  const allowWrites = Boolean(message.writable);
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!extensionContext || !projectDir || !protocol || paths.length === 0) {
    return;
  }
  const current = findEndpointParams(protocol);
  if (!current) {
    await vscode.window.showWarningMessage(
      `Configure the ${protocolDisplayName(protocol)} first, then choose globals to expose.`
    );
    return;
  }
  const { names, params } = buildExposeApplyParams(current, paths, allowWrites);
  const result = await offlineCommApply(extensionContext, projectDir, protocol, params, "upsert");
  if (result?.applied) {
    const restart = result.lifecycle_effect === "restart_required" ? " Restart to apply." : "";
    await vscode.window.showInformationMessage(
      `${protocolDisplayName(protocol)}: exposed ${countLabel(names.length, "global")}.${restart}`
    );
    await refreshNetworkCanvasPanel();
  } else {
    const errs = result?.field_errors?.map((e) => e.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not expose globals: ${
        errs ?? result?.message ?? "edit the server config in the inspector first."
      }`
    );
  }
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
  const connection = message.connection as Record<string, unknown> | undefined;
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!extensionContext || !projectDir || !connection) {
    return;
  }
  const points = Array.isArray(connection.points) ? connection.points.length : 0;
  const result = await offlineCommApply(
    extensionContext,
    projectDir,
    "opcua_client",
    { enabled: true, connections: [connection] },
    "add"
  );
  if (result?.applied) {
    await vscode.window.showInformationMessage(
      `Added OPC UA client connection with ${points} node(s).${
        result.lifecycle_effect === "restart_required" ? " Restart the runtime to read it." : ""
      }`
    );
    await refreshNetworkCanvasPanel();
  } else {
    const errs = result?.field_errors?.map((e) => e.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not save the OPC UA client connection: ${
        errs ?? result?.message ?? "check the endpoint and try again."
      }`
    );
  }
}

// EtherCAT "Browse channels" selects PDO channel paths from the configured modules and writes that
// selection back to the EtherCAT driver params. It is NOT an ADS import/tag pipeline.
async function handleAddEthercatChannels(message: Record<string, unknown>): Promise<void> {
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((p): p is string => typeof p === "string" && p.trim().length > 0)
    : [];
  const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!extensionContext || !projectDir || paths.length === 0) {
    return;
  }
  const target = isRecord(message.target) ? message.target : {};
  const current = Object.keys(target).length > 0 ? target : (findEndpointParams("ethercat") ?? {});
  if (Object.keys(current).length === 0) {
    await vscode.window.showWarningMessage(
      "Configure EtherCAT modules first, then browse channels."
    );
    return;
  }
  const selectedChannels = Array.from(new Set(paths.map((path) => path.trim()))).sort();
  const result = await offlineCommApply(
    extensionContext,
    projectDir,
    "ethercat",
    { ...current, selected_channels: selectedChannels },
    "upsert"
  );
  if (result?.applied) {
    const count = selectedChannels.length;
    await vscode.window.showInformationMessage(
      `${countLabel(count, "EtherCAT channel")} selected.${
        result.lifecycle_effect === "restart_required" ? " Restart to apply." : ""
      }`
    );
    await refreshNetworkCanvasPanel();
  } else {
    const errs = result?.field_errors?.map((e) => e.message).join("; ");
    await vscode.window.showWarningMessage(
      `Could not save EtherCAT channels: ${
        errs ?? result?.message ?? "check the configured modules and try again."
      }`
    );
  }
}

// §0.5 ADS "Add tags": write the selected symbols through the existing ADS import pipeline
// (ads.toml points + cached snapshot + generated ST) via the ads.import_symbols.apply control verb.
// Control-only (needs a reachable runtime with a project_root); a LIVE symbol upload needs an
// ads-wire runtime, so the default build returns an honest error here.
async function handleAddTags(message: Record<string, unknown>): Promise<void> {
  const paths = Array.isArray(message.paths)
    ? message.paths.filter((p): p is string => typeof p === "string")
    : [];
  if (paths.length === 0) {
    return;
  }
  if (
    !activeRuntimeTarget ||
    activeRuntimeTarget.status !== "online_reachable" ||
    !activeRuntimeTarget.endpoint
  ) {
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
    const report = await sendRuntimeControlRequest<{
      applied?: boolean;
      selected_count?: number;
      message?: string;
    }>(
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
      const count = report.selected_count ?? paths.length;
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
      // ads.route_add exists (Admin), but the create-route flow needs an ads-wire runtime + the
      // route_plan from a live browse — not wireable from this offline build yet.
      await vscode.window.showInformationMessage(
        "Add the ADS route from the ADS panel's route doctor (needs an ads-wire runtime); the canvas will browse symbols once the route is accepted."
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
    <title>Devices &amp; Connections</title>
    <link rel="stylesheet" href="${styleUri}" />
    <style>
      * { box-sizing: border-box; margin: 0; padding: 0; }
      html, body, #root {
        width: 100%; height: 100%; overflow: hidden;
        font-family: var(--vscode-font-family, -apple-system, "Segoe UI", sans-serif);
        background: var(--trust-canvas, var(--vscode-editor-background, #0f1116));
        color: var(--trust-text, var(--vscode-foreground, #eef1f5));
      }
      .initial-loading {
        width: 100%; height: 100%;
        display: flex; flex-direction: column; align-items: center; justify-content: center;
        gap: 12px; text-align: center;
        background: var(--trust-canvas, var(--vscode-editor-background, #0f1116));
        color: var(--trust-text-muted, var(--vscode-descriptionForeground, #949cab));
      }
      .initial-loading__icon {
        width: 38px; height: 38px;
        color: var(--trust-text-subtle, var(--vscode-disabledForeground, #6b7480));
      }
      .initial-loading__title {
        font-size: 13.5px; font-weight: 600;
      }
      .initial-loading__detail {
        max-width: 300px; font-size: 12px;
        color: var(--trust-text-subtle, var(--vscode-disabledForeground, #6b7480));
      }
    </style>
  </head>
  <body>
    <div id="root">
      <div class="initial-loading" role="status" aria-live="polite">
        <svg class="initial-loading__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="3" y="4.5" width="18" height="6" rx="1.5"></rect>
          <rect x="3" y="13.5" width="18" height="6" rx="1.5"></rect>
          <circle cx="6.6" cy="7.5" r="1" fill="currentColor" stroke="none"></circle>
          <circle cx="6.6" cy="16.5" r="1" fill="currentColor" stroke="none"></circle>
        </svg>
        <div class="initial-loading__title">Loading your devices...</div>
        <div class="initial-loading__detail">Reading the project's runtime and connections.</div>
      </div>
    </div>
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
