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
  // Non-secret config params for this endpoint (schema_version 3, from `comm topology`),
  // used to pre-fill the editable inspector with the device's current settings.
  params?: Record<string, unknown>;
  owned: boolean;
  supports_test: boolean;
  // v4 (§10.2): per-protocol intent + fieldbus slaves (EtherCAT segment children).
  category?: string;
  profile?: string;
  display_name?: string;
  children?: FleetTopologySlave[];
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

// A configured fleet peer that isn't reachable yet still appears — as a STOPPED / unreachable
// host + runtime (honest grey, never green) — so adding a host/runtime shows something immediately
// instead of vanishing until the runtime is running. Ids are endpoint-derived; once the peer comes
// online its real fleet.topology (real host_id) replaces this synthetic node.
export function offlineTopologyForTarget(target: RuntimeTarget): FleetTopologyResponse | undefined {
  const endpoint = target.endpoint?.trim();
  if (!endpoint || target.status === "online_reachable") {
    return undefined;
  }
  // Neutral grey "stopped" (the local-sim convention), not red — a just-added peer isn't a fault;
  // the detail says why. `auth_failed` is a real error (red).
  // We KNOW it's configured + not reachable; we do NOT know whether the process is stopped or simply
  // unreachable — so render "unknown" (grey, ghosted), never the over-claim "stopped" (Codex review).
  // auth_failed is a genuine error (red).
  const health = target.status === "auth_failed" ? "error" : "unknown";
  const detail =
    target.status === "auth_failed"
      ? "Authentication failed — check the runtime's auth token."
      : "Configured endpoint not reachable — open it in Devices & Connections to connect or diagnose.";
  const hostId = `fleet:${endpoint}`;
  return {
    schema_version: 3,
    hosts: [
      {
        host_id: hostId,
        hostname: target.label || endpoint,
        arch: "",
        os: "",
        ips: [],
        containers: [],
        runtimes: [
          {
            runtime_id: `${hostId}:runtime`,
            name: target.label || "runtime",
            control_endpoint: endpoint,
            // "unknown" mode (not "online"/"stopped") so the badge doesn't claim running OR stopped.
            mode: target.status === "auth_failed" ? "error" : "unknown",
            cycle_ms: 0,
            health,
            detail,
            endpoints: [],
          },
        ],
      },
    ],
    links: [],
    shared: [],
    external: [],
  };
}

// Fetch fleet.topology from every reachable target; for configured-but-unreachable targets,
// synthesize a stopped node so they still show. Merge into one fleet view.
export async function fetchAndMergeFleetTopologies(
  runtimes: readonly RuntimeTarget[],
  timeoutMs = 2000
): Promise<FleetTopologyResponse | undefined> {
  const responses = await Promise.all(
    runtimes.map(async (runtime) =>
      runtime.status === "online_reachable"
        ? await fetchFleetTopology(runtime, timeoutMs).catch(() => undefined)
        : offlineTopologyForTarget(runtime)
    )
  );
  if (responses.every((r) => r === undefined)) {
    return undefined;
  }
  return mergeFleetTopologies(responses);
}
