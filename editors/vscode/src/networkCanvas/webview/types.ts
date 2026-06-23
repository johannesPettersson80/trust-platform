// Data contract sent from the extension panel to the React Flow webview.
// One canvas, fed from the honest buildNetworkCanvasModel() output.

// The local simulator runtime node's stable id — the contract for "this is the process we own"
// (Start/Stop), vs. a remote/fleet runtime (Connect/Disconnect). Shared so the inspector, layout, and
// graph builder don't each hardcode the literal.
export const LOCAL_RUNTIME_NODE_ID = "runtime:local";

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

export interface NCEndpoint {
  id: string;
  kind: string;
  protocol: string;
  name: string;
  role?: string;
  health: string;
  detail: string;
  dimmed?: boolean;
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
}

export interface NCExternal {
  id: string;
  name: string;
  kind: string;
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
  banner?: { text: string; actions: NCEmptyAction[]; kind?: "error" | "info" };
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
}
export interface EndpointNodeData extends Record<string, unknown> {
  name: string;
  protocol: string;
  kind: string;
  role: string;
  detail: string;
  health: string;
  dimmed: boolean;
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
}
// v4 (§0.4): an empty-slot placeholder rendered in Edit mode (dashed ghost cell).
export interface SlotNodeData extends Record<string, unknown> {
  label: string;
  slot: { add: "device" | "runtime" | "host"; category?: string; targetId?: string };
}
