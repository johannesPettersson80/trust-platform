// Data contract sent from the extension panel to the React Flow webview.
// One canvas, fed from the honest buildNetworkCanvasModel() output.

// The local simulator runtime node's stable id — the contract for "this is the process we own"
// (Start/Stop), vs. a remote/fleet runtime (Connect/Disconnect). Shared so the inspector, layout, and
// graph builder don't each hardcode the literal.
export const LOCAL_RUNTIME_NODE_ID = "runtime:local";
export const AUTHORITY_CHECK_RUNTIME_NODE_ID = "runtime:checking-active";

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

export interface NCEndpoint {
  id: string;
  kind: string;
  protocol: string;
  name: string;
  role?: string;
  health: string;
  detail: string;
  dimmed?: boolean;
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
  endpoints: NCEndpoint[];
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
  banner?: {
    text: string;
    actions: NCEmptyAction[];
    kind?: "error" | "info";
    /** Faults already presented here with a primary recovery action. */
    representedFaultIds?: readonly string[];
  };
  hosts: NCHost[];
  links: NCLink[];
  external: NCExternal[];
  faults: NCFault[];
  searchQuery?: string;
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
  container?: string;
  // Per-runtime control inputs for the inspector (see NCRuntime).
  controlEndpoint?: string;
  attached?: boolean;
  managed?: boolean;
  managedName?: string;
  runTarget?: boolean;
}
export interface EndpointNodeData extends Record<string, unknown> {
  name: string;
  protocol: string;
  kind: string;
  role: string;
  detail: string;
  health: string;
  dimmed: boolean;
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
  slot: {
    add: "device" | "runtime" | "host";
    category?: string;
    targetId?: string;
  };
}
