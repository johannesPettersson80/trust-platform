// The `fleet.topology` report of a runtime (specification 11, "Offline communication operation and
// fleet-topology truth"), and its request. The canvas sends the reports it gathers to
// `trust-lsp.devicesTopology`, which merges them (canvasCommands.ts); nothing here merges or judges.
import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";

export interface FleetTopologyResponse {
  schema_version: number;
  hosts: FleetTopologyHost[];
  links: FleetTopologyLink[];
  shared: FleetTopologyShared[];
  external: FleetTopologyExternal[];
}

export interface FleetTopologyHost {
  host_id: string;
  hostname: string;
  board?: string;
  arch: string;
  os: string;
  ips: string[];
  temp_c?: number;
  uptime_s?: number;
  load?: number;
  containers: FleetTopologyContainer[];
  runtimes: FleetTopologyRuntime[];
}

export interface FleetTopologyContainer {
  container_id: string;
  name: string;
  image: string;
  status: string;
  runtimes: FleetTopologyRuntime[];
}

export interface FleetTopologyRuntime {
  runtime_id: string;
  name: string;
  control_endpoint?: string;
  web_listen?: string;
  mode: string;
  cycle_ms: number;
  load?: number;
  health: string;
  detail: string;
  endpoints: FleetTopologyEndpoint[];
  source?: string;
  last_seen_ms?: number;
}

export interface FleetTopologyEndpoint {
  id: string;
  kind: "field" | "service" | "peer" | string;
  protocol: string;
  name: string;
  address?: string;
  role?: string;
  health: string;
  detail: string;
  live?: {
    value?: unknown;
    last_seen_ms?: number;
    rtt_ms?: number;
  };
  connector?: FleetTopologyConnectorStatus;
  // Non-secret config params for this endpoint (schema_version 3, from `comm topology`),
  // used to pre-fill the editable inspector with the device's current settings.
  params?: Record<string, unknown>;
  owned: boolean;
  supports_test: boolean;
  source?: string;
  // v4 (§10.2): per-protocol intent + fieldbus slaves (EtherCAT segment children).
  category?: string;
  profile?: string;
  display_name?: string;
  children?: FleetTopologySlave[];
}

export interface FleetTopologyConnectorStatus {
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

// v4 (§10.2): one slave/module on a fieldbus segment (EtherCAT terminal), from `comm topology`.
export interface FleetTopologySlave {
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

export interface FleetTopologyLink {
  // `id` + `role` arrive in the schema_version 3 contract (spec §10.1). Optional so a
  // v2 runtime still type-checks; the mapper synthesizes an id and derives a role when absent.
  id?: string;
  from: string;
  to: string;
  protocol: string;
  // truST's own role on the link: client | server | master | peer | publish_subscribe.
  role?: string;
  direction: string;
  same_host: boolean;
  status: string;
  secure: boolean;
  detail?: string;
}

export interface FleetTopologyShared {
  id: string;
  kind: string;
  name: string;
  address: string;
  used_by: string[];
}

export interface FleetTopologyExternal {
  id: string;
  kind: string;
  name: string;
  via_protocol: string[];
  direction: string;
}

export async function fetchFleetTopology(
  runtime: RuntimeTarget,
  timeoutMs = 2000
): Promise<FleetTopologyResponse | undefined> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return undefined;
  }
  return await sendRuntimeControlRequest<FleetTopologyResponse>(
    runtime.endpoint,
    runtime.authToken,
    "fleet.topology",
    undefined,
    { timeoutMs }
  );
}
