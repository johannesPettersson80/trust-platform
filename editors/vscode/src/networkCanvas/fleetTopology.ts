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
    value?: string;
    last_seen_ms?: number;
    rtt_ms?: number;
  };
  owned: boolean;
  supports_test: boolean;
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

// §10/§12.10 hybrid source: each runtime reports its OWN host tree (+ mesh-discovered
// peers); the EXTENSION queries every configured runtime and merges so a host appears
// once. Pure + deterministic so it can be unit-tested without a live fleet. First report
// wins for a host's scalar facts; runtimes and containers are unioned by id; links/shared/
// external are deduped. Order-independent within each map.
export function mergeFleetTopologies(
  responses: ReadonlyArray<FleetTopologyResponse | undefined>
): FleetTopologyResponse {
  const hostsById = new Map<string, FleetTopologyHost>();
  const linksByKey = new Map<string, FleetTopologyLink>();
  const sharedById = new Map<string, FleetTopologyShared>();
  const externalById = new Map<string, FleetTopologyExternal>();
  let schemaVersion = 0;

  for (const res of responses) {
    if (!res) {
      continue;
    }
    schemaVersion = Math.max(schemaVersion, res.schema_version ?? 0);
    for (const host of res.hosts ?? []) {
      mergeHost(hostsById, host);
    }
    for (const link of res.links ?? []) {
      const key = link.id ?? `${link.from}|${link.to}|${link.protocol}`;
      if (!linksByKey.has(key)) {
        linksByKey.set(key, link);
      }
    }
    for (const shared of res.shared ?? []) {
      const existing = sharedById.get(shared.id);
      if (existing) {
        existing.used_by = unionStrings(existing.used_by, shared.used_by);
      } else {
        sharedById.set(shared.id, { ...shared, used_by: [...(shared.used_by ?? [])] });
      }
    }
    for (const external of res.external ?? []) {
      if (!externalById.has(external.id)) {
        externalById.set(external.id, external);
      }
    }
  }

  return {
    schema_version: schemaVersion,
    hosts: [...hostsById.values()],
    links: [...linksByKey.values()],
    shared: [...sharedById.values()],
    external: [...externalById.values()],
  };
}

function mergeHost(hostsById: Map<string, FleetTopologyHost>, host: FleetTopologyHost): void {
  const existing = hostsById.get(host.host_id);
  if (!existing) {
    hostsById.set(host.host_id, {
      ...host,
      runtimes: [...(host.runtimes ?? [])],
      containers: (host.containers ?? []).map((c) => ({ ...c, runtimes: [...(c.runtimes ?? [])] })),
    });
    return;
  }
  // Same host reported by another runtime → union its runtimes + containers by id.
  existing.runtimes = unionById(existing.runtimes, host.runtimes ?? [], (r) => r.runtime_id);
  for (const container of host.containers ?? []) {
    const match = existing.containers.find((c) => c.container_id === container.container_id);
    if (match) {
      match.runtimes = unionById(match.runtimes, container.runtimes ?? [], (r) => r.runtime_id);
    } else {
      existing.containers.push({ ...container, runtimes: [...(container.runtimes ?? [])] });
    }
  }
}

function unionById<T>(base: T[], extra: T[], key: (item: T) => string): T[] {
  const seen = new Set(base.map(key));
  const out = [...base];
  for (const item of extra) {
    if (!seen.has(key(item))) {
      seen.add(key(item));
      out.push(item);
    }
  }
  return out;
}

function unionStrings(a: readonly string[], b: readonly string[]): string[] {
  return [...new Set([...(a ?? []), ...(b ?? [])])];
}

// Fetch fleet.topology from every reachable target and merge into one fleet view.
// Unreachable targets contribute nothing (fetchFleetTopology returns undefined).
export async function fetchAndMergeFleetTopologies(
  runtimes: readonly RuntimeTarget[],
  timeoutMs = 2000
): Promise<FleetTopologyResponse | undefined> {
  const responses = await Promise.all(
    runtimes.map((runtime) =>
      fetchFleetTopology(runtime, timeoutMs).catch(() => undefined)
    )
  );
  if (responses.every((r) => r === undefined)) {
    return undefined;
  }
  return mergeFleetTopologies(responses);
}
