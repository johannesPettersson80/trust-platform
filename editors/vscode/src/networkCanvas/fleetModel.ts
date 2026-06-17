import type { FleetTopologyResponse } from "./fleetTopology";

export interface NetworkCanvasFleetEndpoint {
  readonly id: string;
  readonly kind: string;
  readonly protocol: string;
  readonly name: string;
  readonly role?: string;
  readonly health: string;
  readonly detail: string;
  readonly owned: boolean;
  readonly dimmed: boolean;
}

export interface NetworkCanvasFleetRuntime {
  readonly id: string;
  readonly name: string;
  readonly mode: string;
  readonly health: string;
  readonly detail: string;
  readonly endpointCount: number;
  readonly endpoints: readonly NetworkCanvasFleetEndpoint[];
}

export interface NetworkCanvasFleetContainer {
  readonly id: string;
  readonly name: string;
  readonly image: string;
  readonly status: string;
  readonly runtimes: readonly NetworkCanvasFleetRuntime[];
}

export interface NetworkCanvasFleetHost {
  readonly id: string;
  readonly hostname: string;
  readonly label: string;
  readonly health: string;
  readonly runtimeCount: number;
  readonly endpointCount: number;
  readonly containers: readonly NetworkCanvasFleetContainer[];
  readonly runtimes: readonly NetworkCanvasFleetRuntime[];
}

export interface NetworkCanvasFleetLink {
  readonly id: string;
  readonly from: string;
  readonly to: string;
  readonly protocol: string;
  readonly role: string;
  readonly status: string;
  readonly secure: boolean;
}

export interface NetworkCanvasFleetView {
  readonly hosts: readonly NetworkCanvasFleetHost[];
  readonly links: readonly NetworkCanvasFleetLink[];
  readonly externalCount: number;
  readonly sharedCount: number;
  readonly summary: string;
}

export interface NetworkCanvasFleetFault {
  readonly id: string;
  readonly label: string;
  readonly targetNodeId: string;
  readonly severity: "warning" | "error";
}

export function fleetViewFromTopology(
  topology: FleetTopologyResponse | undefined,
  searchQuery: string | undefined
): NetworkCanvasFleetView | undefined {
  if (!topology || topology.hosts.length === 0) {
    return undefined;
  }
  const query = (searchQuery ?? "").trim().toLowerCase();
  const hosts = topology.hosts.map((host) => {
    const hostHaystack =
      `${host.hostname} ${host.os} ${host.arch} ${(host.ips ?? []).join(" ")}`.toLowerCase();
    const bareRuntimes = host.runtimes.map((runtime) =>
      fleetRuntime(runtime, query, hostHaystack)
    );
    const containers = host.containers.map((container) => {
      const containerHaystack =
        `${hostHaystack} ${container.name} ${container.image} ${container.status}`.toLowerCase();
      return {
        id: container.container_id,
        name: container.name,
        image: container.image,
        status: container.status,
        runtimes: container.runtimes.map((runtime) =>
          fleetRuntime(runtime, query, containerHaystack)
        ),
      };
    });
    const allRuntimes = [
      ...bareRuntimes,
      ...containers.flatMap((container) => container.runtimes),
    ];
    const endpointCount = allRuntimes.reduce(
      (sum, runtime) => sum + runtime.endpointCount,
      0
    );
    return {
      id: host.host_id,
      hostname: host.hostname,
      label: `${host.hostname} · ${host.os}/${host.arch}`,
      health: aggregateHealth(
        allRuntimes.flatMap((runtime) => [
          runtime.health,
          ...runtime.endpoints.map((endpoint) => endpoint.health),
        ])
      ),
      runtimeCount: allRuntimes.length,
      endpointCount,
      containers,
      runtimes: bareRuntimes,
    };
  });
  const links = topology.links.map((link, index) => ({
    // Prefer the runtime-supplied id (schema_version 3); fall back to a stable synthetic key.
    id: link.id ?? `fleet-link:${index}:${link.protocol}`,
    from: link.from,
    to: link.to,
    protocol: link.protocol,
    // truST's own role on the link. Empty until the v3 contract carries it (spec §10.1);
    // the webview derives a per-protocol default from `protocol` when this is empty.
    role: link.role ?? "",
    status: link.status,
    secure: link.secure,
  }));
  const runtimeCount = hosts.reduce((sum, host) => sum + host.runtimeCount, 0);
  const endpointCount = hosts.reduce((sum, host) => sum + host.endpointCount, 0);
  return {
    hosts,
    links,
    externalCount: topology.external.length,
    sharedCount: topology.shared.length,
    summary: `${hosts.length} host${hosts.length === 1 ? "" : "s"} · ${runtimeCount} runtime${runtimeCount === 1 ? "" : "s"} · ${endpointCount} endpoint${endpointCount === 1 ? "" : "s"}`,
  };
}

export function fleetFaultsFromView(
  fleet: NetworkCanvasFleetView | undefined
): readonly NetworkCanvasFleetFault[] {
  const faults: NetworkCanvasFleetFault[] = [];
  for (const host of fleet?.hosts ?? []) {
    for (const runtime of [
      ...host.runtimes,
      ...host.containers.flatMap((container) => container.runtimes),
    ]) {
      for (const endpoint of runtime.endpoints) {
        if (endpoint.health === "error" || endpoint.health === "degraded") {
          faults.push({
            id: `fleet:${endpoint.id}`,
            label: `${endpoint.name}: ${endpoint.detail}`,
            targetNodeId: endpoint.id,
            severity: endpoint.health === "error" ? "error" : "warning",
          });
        }
      }
    }
  }
  return faults;
}

function fleetRuntime(
  runtime: FleetTopologyResponse["hosts"][number]["runtimes"][number],
  query: string,
  parentHaystack: string
): NetworkCanvasFleetRuntime {
  const runtimeHaystack =
    `${parentHaystack} ${runtime.name} ${runtime.mode} ${runtime.health} ${runtime.detail}`.toLowerCase();
  const endpoints = runtime.endpoints.map((endpoint) => {
    const haystack =
      `${runtimeHaystack} ${endpoint.name} ${endpoint.protocol} ${endpoint.kind} ${endpoint.role ?? ""} ${endpoint.detail}`.toLowerCase();
    const dimmed = query.length > 0 && !haystack.includes(query);
    return {
      id: endpoint.id,
      kind: endpoint.kind,
      protocol: endpoint.protocol,
      name: endpoint.name,
      role: endpoint.role,
      health: endpoint.health,
      detail: endpoint.detail,
      owned: endpoint.owned,
      dimmed,
    };
  });
  return {
    id: runtime.runtime_id,
    name: runtime.name,
    mode: runtime.mode,
    health: aggregateHealth([
      runtime.health,
      ...endpoints.map((endpoint) => endpoint.health),
    ]),
    detail: runtime.detail,
    endpointCount: endpoints.length,
    endpoints,
  };
}

function aggregateHealth(values: readonly string[]): string {
  if (values.includes("error")) {
    return "error";
  }
  if (values.includes("degraded")) {
    return "degraded";
  }
  if (values.includes("connected")) {
    return "connected";
  }
  return "pending";
}
