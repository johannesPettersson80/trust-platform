// Data contract sent from the extension panel to the React Flow webview.
// One canvas, fed from the honest buildNetworkCanvasModel() output.

export interface NCEndpoint {
  id: string;
  kind: string;
  protocol: string;
  name: string;
  role?: string;
  health: string;
  detail: string;
  dimmed?: boolean;
}

export interface NCRuntime {
  id: string;
  name: string;
  mode: string;
  health: string;
  detail: string;
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
  // Thin inline strip shown only on a runtime start failure (not a screen).
  banner?: { text: string; actions: NCEmptyAction[] };
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
}
export interface EndpointNodeData extends Record<string, unknown> {
  name: string;
  protocol: string;
  kind: string;
  role: string;
  detail: string;
  health: string;
  dimmed: boolean;
}
export interface ExternalNodeData extends Record<string, unknown> {
  label: string;
  sub: string;
}
