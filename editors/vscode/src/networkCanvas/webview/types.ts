// Data contract sent from the extension panel to the React Flow webview.
import type { RuntimeNodeControl } from "./runtimeNodeControls";
import type { CanvasAdsClientSummary } from "../fleetModel";

// One canvas, fed from the graph that `trust-lsp.devicesTopology` answers.

// The local simulator runtime node's stable id — the contract for "this is the process we own"
// (Start/Stop), vs. a remote/fleet runtime (Connect/Disconnect). Shared so the inspector, layout, and
// graph builder don't each hardcode the literal.
export const LOCAL_RUNTIME_NODE_ID = "runtime:local";

export function runtimeEmptyMessage(): string {
  return "No devices or simulated plants yet";
}

// v4 (§10.2): one slave/module of a fieldbus segment (e.g. an EtherCAT terminal).
export interface NCFieldSlave {
  id: string;
  kind: string; // "field_slave"
  slot: number;
  name: string;
  model?: string;
  profile?: string;
  channels?: number;
  source?: string; // "config" | "observed"
  health?: string;
  detail?: string;
}

export interface NCConnectorStatus {
  connector_id: string;
  state: string;
  health: string;
  confidence: string;
  point_counts: {
    total: number;
    good: number;
    degraded: number;
    unavailable: number;
  };
}

// The tags of one ADS port, as the server answers them (`trust-lsp.devicesTopology` and
// `trust-lsp.devicesCanvasAct`): each port once, its paths sorted.
export interface AdsTagPortSelection {
  readonly port: number;
  readonly paths: readonly string[];
}

// One tag of one ADS port, which the tag browser removes.
export interface AdsTagPortPath {
  readonly port: number;
  readonly path: string;
}

// One port's part of an ADS tag add or removal, as the server answers it.
export interface AdsTagPortImportResult {
  readonly port: number;
  readonly paths: readonly string[];
  readonly applied: boolean;
  readonly addedCount: number;
  readonly message: string;
}

// The result of an ADS tag add or removal, with the summary the tag browser shows.
export interface AdsTagBatchImportResult {
  readonly operation?: "add" | "remove";
  readonly applied: boolean;
  readonly addedCount: number;
  readonly removedCount?: number;
  readonly restartRequired: boolean;
  readonly ports: readonly AdsTagPortImportResult[];
  readonly summary: string;
}

export interface NCEndpoint {
  id: string;
  kind: string;
  protocol: string;
  name: string;
  role?: string;
  health: string;
  runtimeHealth?: string;
  detail: string;
  dimmed?: boolean;
  // The target the browse pane opens with, for an ADS or OPC UA client connection (server's answer).
  browseTarget?: Record<string, unknown>;
  // Whether a filter that hides it reports it (server's answer).
  attention?: boolean;
  // The ADS client's summary (server's answer).
  adsSummary?: CanvasAdsClientSummary;
  live?: {
    value?: unknown;
    last_seen_ms?: number;
    rtt_ms?: number;
  };
  connector?: NCConnectorStatus;
  params?: Record<string, unknown>;
  // v4 (§10.2)
  category?: string;
  profile?: string;
  display_name?: string;
  children?: NCFieldSlave[];
}

export interface NCRuntime {
  id: string;
  name: string;
  mode: string;
  health: string;
  // Authoritative managed lifecycle state, kept separate from semantic health/tone.
  lifecycleState?: string;
  detail: string;
  // Per-runtime control endpoint (remote runtimes only; the local simulator has none).
  controlEndpoint?: string;
  // Honest "does the extension hold a live connection to THIS runtime?" — distinct from `health`,
  // which is the runtime's OWN reported health. Drives Connect vs Disconnect / Start vs Stop.
  attached?: boolean;
  // A managed local runtime (fleet.toml project on this computer we own — Phase 9). When true, the node
  // gets Start/Stop/Logs via the fleet lifecycle (managedName), not Connect/Disconnect.
  managed?: boolean;
  managedName?: string;
  // Projection of the shared selected run target store. This is visual feedback only; the sidebar
  // remains the lifecycle/action owner.
  runTarget?: boolean;
  // The local simulator's speed while it runs (specification 46, §6.1): configured, and achieved once measured;
  // and the speed words the runtime gives for them (specification 35, B9.2).
  simulationTimeScale?: number;
  simulationAchievedTimeScale?: number;
  simulationSpeedText?: string;
  endpoints: NCEndpoint[];
  plants?: NCPlant[];
  // `Identify`: whether it can be asked now, and why not (server's answer).
  identify?: { enabled: boolean; reason?: string };
  // The controls the language server gives the node, the primary one first (specification 25 §5.5).
  controls?: RuntimeNodeControl[];
  // Whether a search can start from the runtime, and why not (specification 25 §2).
  discovery?: { ready: boolean; reason?: string };
}

export type NCPlantState =
  | "Draft"
  | "Configured"
  | "Disabled"
  | "Waiting for samples"
  | "Running"
  | "Restart required"
  | "Faulted"
  | "Status unavailable";

export interface NCPlantLiveSample {
  command: number[];
  feedback: number[];
}

/** A simulation process owned by one runtime, distinct from communication and I/O endpoints. */
export interface NCPlant {
  id: string;
  projectId?: string;
  displayName: string;
  model: string;
  startingPointName?: string;
  enabled: boolean;
  commandSources: string[];
  feedbackTargets: string[];
  mappedSymbols?: string[];
  draft?: boolean;
  statusAvailability: "not_requested" | "available" | "unavailable";
  activeOperatingState?: "running" | "faulted";
  hasCompletedSample?: boolean;
  liveSample?: NCPlantLiveSample;
  savedConfigurationRevision?: string;
  activeConfigurationRevision?: string;
  configuration?: {
    preservedConfiguration?: Record<string, unknown>;
    description: string;
    parameters: Record<string, number | number[] | number[][]>;
    initialState: number[];
    enabledDefaulted?: boolean;
    initialStateDefaulted?: boolean;
    stepMs?: number;
    initialInput?: number[];
    inputScale?: number[];
    inputOffset?: number[];
    outputScale?: number[];
    outputOffset?: number[];
    actuator?: Record<string, number>;
    uncertainties?: Array<Record<string, unknown>>;
    scenarioEvents?: Array<{
      enabled: boolean;
      atMs: number;
      kind: string;
      channel?: number;
      parameter?: string;
      value?: number;
    }>;
  };
}

export interface NCContainer {
  id: string;
  name: string;
  image: string;
  status: string;
  runtimes: NCRuntime[];
}

export interface NCHost {
  id: string;
  hostname: string;
  label: string;
  health: string;
  containers: NCContainer[];
  runtimes: NCRuntime[];
}

export interface NCLink {
  id: string;
  from: string;
  to: string;
  protocol: string;
  role: string;
  status: string;
  detail?: string;
  secure: boolean;
  dimmed?: boolean;
}

export interface NCExternal {
  id: string;
  name: string;
  kind: string;
  dimmed?: boolean;
}

export interface NCFault {
  id: string;
  label: string;
  targetNodeId: string;
  severity: "warning" | "error";
  // The fault of the add form's refused result, hidden while the form is edited (server's answer).
  form?: boolean;
}

export interface NCEmptyAction {
  label: string;
  action: string;
}

export interface NCGraph {
  kind: "graph";
  title: string;
  summary: string;
  // Thin inline strip: a neutral hint ("info") or a start-failure ("error", default).
  banner?: { text: string; actions: NCEmptyAction[]; kind?: "error" | "info" };
  hosts: NCHost[];
  links: NCLink[];
  external: NCExternal[];
  faults: NCFault[];
  searchQuery?: string;
}

export function canvasGraphSummary(graph: NCGraph): string {
  const runtimes = graph.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ]);
  const runtimeCount = runtimes.length;
  const endpointCount = runtimes.reduce((sum, runtime) => sum + runtime.endpoints.length, 0);
  const plantCount = runtimes.reduce(
    (sum, runtime) => sum + (runtime.plants?.length ?? 0),
    0
  );
  const base = `${graph.hosts.length} host${graph.hosts.length === 1 ? "" : "s"} · ${runtimeCount} runtime${runtimeCount === 1 ? "" : "s"}`;
  if (plantCount > 0) {
    return `${base} · ${endpointCount} device${endpointCount === 1 ? "" : "s"} · ${plantCount} simulated plant${plantCount === 1 ? "" : "s"}`;
  }
  return `${base} · ${endpointCount} endpoint${endpointCount === 1 ? "" : "s"}`;
}

// Custom-node payloads --------------------------------------------------------
export interface HostNodeData extends Record<string, unknown> {
  label: string;
  sub: string;
  health: string;
  runtimeCount: number;
  endpointCount: number;
}
export interface ContainerNodeData extends Record<string, unknown> {
  label: string;
  image: string;
  status: string;
}
export interface RuntimeNodeData extends Record<string, unknown> {
  label: string;
  mode: string;
  health: string;
  detail: string;
  endpointCount: number;
  plantCount: number;
  container?: string;
  // Per-runtime control inputs for the inspector (see NCRuntime).
  controlEndpoint?: string;
  attached?: boolean;
  managed?: boolean;
  managedName?: string;
  runTarget?: boolean;
  // The local simulator's speed while it runs (see NCRuntime).
  simulationTimeScale?: number;
  simulationAchievedTimeScale?: number;
  simulationSpeedText?: string;
  plantSimulationSupported: boolean;
  plantDropState?: "eligible" | "current" | "ineligible";
  plantDropLabel?: string;
}
export interface EndpointNodeData extends Record<string, unknown> {
  name: string;
  protocol: string;
  kind: string;
  role: string;
  detail: string;
  health: string;
  dimmed: boolean;
  browseTarget?: Record<string, unknown>;
  live?: {
    value?: unknown;
    last_seen_ms?: number;
    rtt_ms?: number;
  };
  connector?: NCConnectorStatus;
  params?: Record<string, unknown>;
  // v4 (§10.2)
  category?: string;
  profile?: string;
  display_name?: string;
  children?: NCFieldSlave[];
}
export interface ExternalNodeData extends Record<string, unknown> {
  label: string;
  sub: string;
  dimmed: boolean;
}
// v4 (§0.4): an empty-slot placeholder rendered in Edit mode (dashed ghost cell).
export interface SlotNodeData extends Record<string, unknown> {
  label: string;
  slot: { add: "device" | "runtime" | "host"; category?: string; targetId?: string };
}
