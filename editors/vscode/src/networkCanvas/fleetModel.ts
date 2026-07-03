import * as os from "os";
import type { FleetTopologyResponse, FleetTopologySlave } from "./fleetTopology";

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
  readonly params?: Record<string, unknown>;
  // v4 (§10.2): intent + fieldbus slaves (EtherCAT segment children).
  readonly category?: string;
  readonly profile?: string;
  readonly display_name?: string;
  readonly children?: readonly FleetTopologySlave[];
}

export interface NetworkCanvasFleetRuntime {
  readonly id: string;
  readonly name: string;
  readonly mode: string;
  readonly health: string;
  readonly detail: string;
  readonly endpointCount: number;
  // The runtime's control endpoint (for per-runtime Connect from the canvas node). Remote runtimes
  // carry it; the local simulator has none (we own its process directly).
  readonly controlEndpoint?: string;
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

function endpointHost(endpoint: string | undefined): string | undefined {
  const value = endpoint?.trim();
  if (!value) {
    return undefined;
  }
  if (value.startsWith("unix://")) {
    return "localhost";
  }
  const normalized = value.includes("://") ? value : `tcp://${value}`;
  try {
    return new URL(normalized).hostname;
  } catch {
    const withoutScheme = value.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "");
    return withoutScheme.split(":")[0]?.replace(/^\[|\]$/g, "") || undefined;
  }
}

function isLoopbackAddress(value: string | undefined): boolean {
  const host = value?.trim().toLowerCase().replace(/^\[|\]$/g, "");
  return (
    host === "localhost" ||
    host === "127.0.0.1" ||
    host === "::1" ||
    host === "0:0:0:0:0:0:0:1" ||
    host === "0.0.0.0"
  );
}

function firstRemoteAddress(host: FleetTopologyResponse["hosts"][number]): string | undefined {
  return (
    host.ips.find((ip) => !isLoopbackAddress(ip)) ??
    host.runtimes
      .map((runtime) => endpointHost(runtime.control_endpoint))
      .find((ip) => ip && !isLoopbackAddress(ip))
  );
}

function localInterfaceAddresses(): Set<string> {
  const addresses = new Set<string>();
  for (const entries of Object.values(os.networkInterfaces())) {
    for (const entry of entries ?? []) {
      const address = entry.address?.trim().toLowerCase().replace(/^\[|\]$/g, "");
      if (address) {
        addresses.add(address);
      }
    }
  }
  return addresses;
}

function hostDisplay(host: FleetTopologyResponse["hosts"][number]): {
  headline: string;
  detail: string;
} {
  const rawDetail = [host.hostname, `${host.os}/${host.arch}`]
    .map((part) => part.trim())
    .filter(Boolean)
    .join(" · ");
  // Synthetic configured peers have no live OS/arch/IP facts. Keep the configured label as the
  // headline even if the stored endpoint is loopback in a test harness; otherwise an auth-failed
  // peer can be mislabeled as "This computer" beside the real local host.
  const hasLiveMachineFacts =
    host.arch.trim().length > 0 || host.os.trim().length > 0 || host.ips.length > 0;
  if (!hasLiveMachineFacts && host.hostname.trim()) {
    return { headline: host.hostname.trim(), detail: rawDetail || host.hostname.trim() };
  }

  // A host with live machine facts is "This computer" only when its reported identity matches the
  // machine VS Code runs on. A loopback CONTROL ENDPOINT alone does NOT prove locality — a remote
  // runtime reached over an SSH tunnel / port-forward also looks loopback yet reports a different
  // machine (its own hostname/arch). Key off the reported identity, not the transport, so two
  // different computers never both read "This computer".
  const localHostname = os.hostname().trim().toLowerCase();
  const reportedHostname = host.hostname.trim();
  const remoteAddress = firstRemoteAddress(host);
  const localAddresses = localInterfaceAddresses();
  const hostReportsLocalAddress =
    host.ips.length === 0 ||
    host.ips.some((ip) => {
      const normalized = ip.trim().toLowerCase().replace(/^\[|\]$/g, "");
      return isLoopbackAddress(normalized) || localAddresses.has(normalized);
    });
  const hasRealHostname =
    reportedHostname.length > 0 && !isLoopbackAddress(reportedHostname);
  const hasRealIp = host.ips.some((ip) => !isLoopbackAddress(ip));
  if (
    localHostname.length > 0 &&
    reportedHostname.toLowerCase() === localHostname &&
    hostReportsLocalAddress
  ) {
    return { headline: "This computer", detail: rawDetail || "local computer" };
  }
  if (hasRealIp && remoteAddress) {
    return { headline: `Computer ${remoteAddress}`, detail: rawDetail || remoteAddress };
  }
  if (hasRealHostname) {
    return { headline: `Computer ${reportedHostname}`, detail: rawDetail || reportedHostname };
  }
  // Only a loopback identity, and not the named local machine → the local loopback context.
  return { headline: "This computer", detail: rawDetail || "local computer" };
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
    const display = hostDisplay(host);
    return {
      id: host.host_id,
      hostname: display.headline,
      label: display.detail,
      // A host's status is its MACHINE reachability — NOT an aggregate of the runtimes on it. It's
      // reachable even when its runtime is stopped. The signals hold on EVERY OS and for REMOTE peers:
      //  • arch/os/ips are reported by each host's OWN runtime via `std::env::consts::ARCH/OS`
      //    (fleet_handlers.rs) — a cross-platform compile-time constant, so always populated on
      //    Linux/Windows/macOS, local or remote. We never key off Linux-only fields (e.g. /proc load).
      //  • a live runtime on the host also proves we reached it.
      // An unreachable configured peer is a synthetic placeholder with arch=""/ips=[]/no live runtime
      // (fleetTopology.ts) → Unreachable. So: reachable here ⇔ we actually have the machine.
      health:
        host.arch.trim().length > 0 ||
        host.ips.length > 0 ||
        allRuntimes.some((runtime) => runtime.health === "connected")
          ? "connected"
          : "runtime_unreachable",
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
      name: endpointDisplayName(endpoint),
      role: endpoint.role,
      health: endpoint.health,
      detail: endpoint.detail,
      owned: endpoint.owned,
      dimmed,
      params: endpoint.params,
      // v4 (§10.2): intent + fieldbus slaves (EtherCAT segment children).
      category: endpoint.category,
      profile: endpoint.profile,
      display_name: endpoint.display_name,
      children: endpoint.children,
    };
  });
  return {
    id: runtime.runtime_id,
    name: runtimeDisplayName(runtime),
    mode: runtime.mode,
    health: aggregateHealth([
      runtime.health,
      ...endpoints.map((endpoint) => endpoint.health),
    ]),
    detail: runtime.detail,
    endpointCount: endpoints.length,
    controlEndpoint: runtime.control_endpoint,
    endpoints,
  };
}

function runtimeDisplayName(
  runtime: FleetTopologyResponse["hosts"][number]["runtimes"][number]
): string {
  const mode = runtime.mode.trim().toLowerCase();
  const rawName = runtime.name.trim();
  const runtimeId = runtime.runtime_id.trim().toLowerCase();
  if (
    runtimeId === "runtime:local" ||
    runtimeId === "runtime:project" ||
    /^local simulator$/i.test(rawName) ||
    (mode === "simulate" && /^trust runtime$/i.test(rawName))
  ) {
    return "Simulator";
  }
  return rawName || "Runtime";
}

function endpointDisplayName(
  endpoint: FleetTopologyResponse["hosts"][number]["runtimes"][number]["endpoints"][number]
): string {
  const rawName = endpoint.name.trim();
  switch (endpoint.protocol.trim().toLowerCase()) {
    case "simulated":
      return "Simulated I/O";
    case "loopback":
      return "Loopback I/O";
    default:
      return rawName || endpoint.protocol.replace(/_/g, " ");
  }
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
  // "configured_policy" = configured from project files but no live health reported → the runtime is
  // not running. That's an honest "Stopped", not a vague "Pending" (which implies a connection in flight).
  if (values.includes("configured_policy")) {
    return "stopped";
  }
  return "pending";
}
