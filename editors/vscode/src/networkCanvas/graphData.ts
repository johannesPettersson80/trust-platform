// Maps the honest NetworkCanvasModel into the React Flow canvas graph contract.
// One canvas, always populated: a live fleet, or — for a new/unconfigured
// project — the local simulator runtime node itself (never a separate screen).
import type {
  FleetTopologyHost,
  FleetTopologyResponse,
  FleetTopologyRuntime,
} from "./fleetTopology";
import type {
  NetworkCanvasDevice,
  NetworkCanvasModel,
  NetworkDeviceStatus,
} from "./model";
import type { NetworkCanvasFleetRuntime } from "./fleetModel";
import { LOCAL_RUNTIME_NODE_ID } from "./webview/types";
import type { ManagedRuntime } from "../localRuntimeModel";
import type { NCFault, NCGraph, NCHost, NCRuntime } from "./webview/types";

function deviceHealth(status: NetworkDeviceStatus): string {
  switch (status) {
    case "connected":
      return "connected";
    case "degraded":
      return "degraded";
    case "error":
      return "error";
    default:
      return "pending";
  }
}

function mapFleetRuntime(rt: NetworkCanvasFleetRuntime): NCRuntime {
  return {
    id: rt.id,
    name: rt.name,
    mode: rt.mode,
    health: rt.health,
    detail: rt.detail,
    controlEndpoint: rt.controlEndpoint,
    endpoints: rt.endpoints.map((ep) => ({
      id: ep.id,
      kind: ep.kind,
      protocol: ep.protocol,
      name: ep.name,
      role: ep.role,
      health: ep.health,
      detail: ep.detail,
      dimmed: ep.dimmed,
      params: ep.params,
      category: ep.category,
      profile: ep.profile,
      display_name: ep.display_name,
      children: ep.children ? ep.children.map((s) => ({ ...s })) : undefined,
    })),
  };
}

function mapFaults(model: NetworkCanvasModel): NCFault[] {
  return model.faults.map((fault) => ({
    id: fault.id,
    label: fault.label,
    targetNodeId: fault.targetNodeId,
    severity: fault.severity,
  }));
}

function fleetGraph(
  model: NetworkCanvasModel,
  topology: FleetTopologyResponse | undefined
): NCGraph {
  const fleet = model.fleet;
  if (!fleet) {
    return localRuntimeGraph(model);
  }
  return {
    kind: "graph",
    title: "Devices & Connections",
    summary: fleet.summary,
    hosts: fleet.hosts.map((host) => ({
      id: host.id,
      hostname: host.hostname,
      label: host.label,
      health: host.health,
      runtimes: host.runtimes.map(mapFleetRuntime),
      containers: host.containers.map((container) => ({
        id: container.id,
        name: container.name,
        image: container.image,
        status: container.status,
        runtimes: container.runtimes.map(mapFleetRuntime),
      })),
    })),
    links: fleet.links.map((link) => ({
      id: link.id,
      from: link.from,
      to: link.to,
      protocol: link.protocol,
      role: link.role,
      status: link.status,
      secure: link.secure,
    })),
    external: [
      ...(topology?.shared ?? []).map((shared) => ({
        id: shared.id,
        name: shared.name || shared.address,
        kind: shared.kind,
      })),
      ...(topology?.external ?? []).map((ext) => ({
        id: ext.id,
        name: ext.name,
        kind: ext.kind,
      })),
    ],
    faults: mapFaults(model),
    searchQuery: model.searchQuery,
  };
}

// No fleet configured: the canvas still shows a host with the local simulator as a node,
// but it is NEVER auto-started — it renders STOPPED until the user starts it (honest health:
// stopped → starting → connected, or error on a failed start).
function localRuntimeGraph(model: NetworkCanvasModel): NCGraph {
  const failure = model.failure;
  const state = model.runtime.state; // "not_started" | "starting" | "running"
  const running = state === "running";
  // The simulator is NEVER auto-started: a not-yet-started one shows as STOPPED, and the
  // user starts it from the node. "starting" only appears after the user initiates it.
  const health = failure
    ? "error"
    : running
      ? "connected"
      : state === "starting"
        ? "pending"
        : "stopped";
  const detail = failure
    ? failure.message
    : running
      ? model.runtime.statusText
      : state === "starting"
        ? "Starting local simulator…"
        : "Stopped — start the simulator to run it.";
  const endpoints = model.devices.map((device: NetworkCanvasDevice) => ({
    id: device.id,
    kind: "field",
    protocol: device.protocol,
    name: device.name,
    role: "owned_driver",
    health: deviceHealth(device.status),
    detail: device.statusText,
    dimmed: false,
  }));
  const faults = mapFaults(model);
  if (failure) {
    faults.unshift({
      id: "fault:runtime",
      label: `Local simulator: ${failure.message}`,
      targetNodeId: LOCAL_RUNTIME_NODE_ID,
      severity: "error",
    });
  }
  return {
    kind: "graph",
    title: "Devices & Connections",
    summary: running
      ? `1 host · 1 runtime · ${endpoints.length} endpoint${endpoints.length === 1 ? "" : "s"}`
      : "1 host · 1 runtime · local simulator (stopped)",
    hosts: [
      {
        id: "host:this-computer",
        hostname: "This computer",
        label: model.runtime.hostLabel
          ? `local simulator · ${model.runtime.hostLabel}`
          : "local simulator",
        // This computer is always reachable (we're running on it) — its status is reachability, NOT the
        // simulator's run state. The runtime node below carries the Start/Stop lifecycle.
        health: "connected",
        containers: [],
        runtimes: [
          {
            id: LOCAL_RUNTIME_NODE_ID,
            name: model.runtime.name || "Local simulator",
            mode: model.runtime.mode,
            health,
            detail,
            // The local simulator has no remote control endpoint — we own its process.
            endpoints,
          },
        ],
      },
    ],
    links: [],
    external: [],
    faults,
    banner: failure
      ? {
          kind: "error",
          text: failure.message,
          actions: [
            { label: "Retry", action: "startLocalSimulator" },
            { label: "Open logs", action: "openRuntimeLogs" },
          ],
        }
      : state === "not_started"
        ? {
            kind: "info",
            text: "Local simulator is stopped.",
            actions: [{ label: "Start simulator", action: "startLocalSimulator" }],
          }
        : running
          ? {
              kind: "info",
              text: "Local simulator running.",
              actions: [{ label: "Stop simulator", action: "stopLocalSimulator" }],
            }
          : undefined,
    searchQuery: model.searchQuery,
  };
}

// Configured fleet peers (added hosts) come as a SEPARATE topology so they show on BOTH the fleet
// view and the local-simulator view (the local sim node is preserved, peers are appended) — an
// added host never vanishes just because the local runtime is stopped. Honest: peer hosts keep
// their own health (a not-running peer stays grey/stopped, never green).
function mapTopoRuntime(rt: FleetTopologyRuntime): NCRuntime {
  return {
    id: rt.runtime_id,
    name: rt.name,
    mode: rt.mode,
    health: rt.health,
    detail: rt.detail,
    controlEndpoint: rt.control_endpoint,
    endpoints: rt.endpoints.map((ep) => ({
      id: ep.id,
      kind: ep.kind,
      protocol: ep.protocol,
      name: ep.name,
      role: ep.role,
      health: ep.health,
      detail: ep.detail,
      dimmed: false,
      params: ep.params,
      category: ep.category,
      profile: ep.profile,
      display_name: ep.display_name,
      children: ep.children ? ep.children.map((s) => ({ ...s })) : undefined,
    })),
  };
}

// Host roll-up: green ONLY if every runtime is connected (never fabricate green).
function peerHostHealth(healths: string[]): string {
  if (healths.some((h) => h === "error")) {
    return "error";
  }
  if (healths.some((h) => h === "degraded")) {
    return "degraded";
  }
  if (healths.length > 0 && healths.every((h) => h === "connected")) {
    return "connected";
  }
  if (healths.some((h) => h === "connected")) {
    return "degraded"; // mixed → not all-green
  }
  return healths[0] ?? "stopped";
}

function mapPeerHost(host: FleetTopologyHost): NCHost {
  const bareRuntimes = host.runtimes.map(mapTopoRuntime);
  const containerRuntimes = host.containers.flatMap((c) => c.runtimes.map(mapTopoRuntime));
  return {
    id: host.host_id,
    hostname: host.hostname,
    label: [host.os, host.arch].filter(Boolean).join("/") || "fleet peer",
    health: peerHostHealth([...bareRuntimes, ...containerRuntimes].map((r) => r.health)),
    containers: host.containers.map((c) => ({
      id: c.container_id,
      name: c.name,
      image: c.image,
      status: c.status,
      runtimes: c.runtimes.map(mapTopoRuntime),
    })),
    runtimes: bareRuntimes,
  };
}

export function buildCanvasGraph(
  model: NetworkCanvasModel,
  topology: FleetTopologyResponse | undefined,
  peerTopology?: FleetTopologyResponse,
  // The control endpoint the extension currently holds a live connection to (an attached remote), or
  // undefined. Used to mark exactly ONE remote runtime as `attached` (→ "Disconnect") — never every
  // healthy remote (which would be a fabricated connection).
  attachedEndpoint?: string,
  // Managed local runtimes (fleet.toml projects we own — Phase 9). Injected as nodes so the user can
  // Start/Stop/Logs them on the canvas; deduped against runtimes already shown via fleet.topology.
  managed: ReadonlyArray<ManagedRuntime> = []
): NCGraph {
  const base = model.fleet ? fleetGraph(model, topology) : localRuntimeGraph(model);
  const peerHosts = (peerTopology?.hosts ?? []).filter(
    (host) => !base.hosts.some((existing) => existing.id === host.host_id)
  );
  const graph =
    peerHosts.length === 0
      ? base
      : (() => {
          const hosts = [...base.hosts, ...peerHosts.map(mapPeerHost)];
          const runtimeCount = hosts.reduce(
            (n, h) =>
              n + h.runtimes.length + h.containers.reduce((m, c) => m + c.runtimes.length, 0),
            0
          );
          return {
            ...base,
            summary: `${hosts.length} host${hosts.length === 1 ? "" : "s"} · ${runtimeCount} runtime${runtimeCount === 1 ? "" : "s"}`,
            hosts,
          };
        })();
  injectManagedRuntimes(graph, managed);
  annotateAttached(graph, attachedEndpoint);
  return graph;
}

// Surface managed local runtimes (fleet.toml projects we own) so Start/Stop/Logs live ON the Devices &
// Connections node (§0.6 / Phase 9). A managed runtime already shown via fleet.topology (or the synthetic
// offline node) is ANNOTATED in place (marked managed, so it KEEPS owned lifecycle — not left as an
// ordinary remote); only managed runtimes with no existing node are injected under a "this computer" host.
function injectManagedRuntimes(
  graph: NCGraph,
  managed: ReadonlyArray<ManagedRuntime>
): void {
  if (managed.length > 0) {
    const byEndpoint = new Map<string, ManagedRuntime>();
    const byName = new Map<string, ManagedRuntime>();
    for (const local of managed) {
      if (local.controlEndpoint) {
        byEndpoint.set(local.controlEndpoint, local);
      }
      byName.set(local.name, local);
    }
    // Annotate any existing node that IS a managed runtime, so the visible node keeps OWNED lifecycle
    // (Start/Stop/Logs) instead of falling back to remote attach. Match by control endpoint OR by name:
    // the live fleet.topology can report a runtime under a slightly different endpoint string than the
    // managed list (e.g. 0.0.0.0 vs 127.0.0.1), so an endpoint-only match would miss it and we'd inject a
    // DUPLICATE node (F-12 twinning). Name is the stable identity for a fleet runtime.
    const annotated = new Set<string>();
    const annotate = (rt: NCRuntime): void => {
      const local =
        (rt.controlEndpoint ? byEndpoint.get(rt.controlEndpoint) : undefined) ?? byName.get(rt.name);
      if (!local) {
        return;
      }
      rt.managed = true;
      rt.managedName = local.name;
      annotated.add(local.name);
    };
    for (const host of graph.hosts) {
      host.runtimes.forEach(annotate);
      host.containers.forEach((container) => container.runtimes.forEach(annotate));
    }
    // Inject only managed runtimes with NO existing node anywhere (e.g. stopped, not in topology).
    const fresh = managed.filter((local) => !annotated.has(local.name));
    if (fresh.length > 0) {
      const host =
        graph.hosts.find((candidate) => candidate.id === "host:this-computer") ??
        graph.hosts.find((candidate) => candidate.hostname === "This computer");
      const runtimes = fresh.map((local) => ({
          id: `managed:${local.name}`,
          name: local.name,
          mode: "managed",
          health: local.state === "running" ? "connected" : "stopped",
          detail:
            local.state === "running"
              ? "Running (managed local runtime)."
              : "Stopped — Start it from this node.",
          controlEndpoint: local.controlEndpoint || undefined,
          managed: true,
          managedName: local.name,
          endpoints: [],
        }));
      if (host) {
        host.runtimes.push(...runtimes);
      } else {
        graph.hosts.push({
          id: "host:managed-local",
          hostname: "This computer",
          label: "managed runtimes",
          // This computer is always reachable — host status is reachability, not the runtimes' run state.
          health: "connected",
          containers: [],
          runtimes,
        });
      }
    }
    graph.summary = graphSummary(graph);
  }
  // De-twin (F-12): a runtime must never ALSO appear as a separate "external system" node. The live
  // topology can advertise one of our own runtimes back as an external peer; drop any external whose
  // name is a known runtime (managed or topology), so one runtime = exactly one node.
  if (graph.external.length > 0) {
    const runtimeNames = new Set<string>();
    for (const host of graph.hosts) {
      host.runtimes.forEach((rt) => runtimeNames.add(rt.name));
      host.containers.forEach((c) => c.runtimes.forEach((rt) => runtimeNames.add(rt.name)));
    }
    for (const local of managed) {
      runtimeNames.add(local.name);
    }
    graph.external = graph.external.filter((ext) => !runtimeNames.has(ext.name));
  }
}

function graphSummary(graph: NCGraph): string {
  const runtimeCount = graph.hosts.reduce(
    (sum, host) =>
      sum + host.runtimes.length + host.containers.reduce((nested, container) => nested + container.runtimes.length, 0),
    0
  );
  const endpointCount = graph.hosts.reduce(
    (sum, host) =>
      sum +
      host.runtimes.reduce((nested, runtime) => nested + runtime.endpoints.length, 0) +
      host.containers.reduce(
        (nested, container) =>
          nested + container.runtimes.reduce((rtSum, runtime) => rtSum + runtime.endpoints.length, 0),
        0
      ),
    0
  );
  return `${graph.hosts.length} host${graph.hosts.length === 1 ? "" : "s"} · ${runtimeCount} runtime${runtimeCount === 1 ? "" : "s"} · ${endpointCount} endpoint${endpointCount === 1 ? "" : "s"}`;
}

// Honest per-runtime "are we connected to it?" flag. Local simulator: attached === running (its own
// health). Remote: attached ONLY if the extension's live connection points at THIS runtime's control
// endpoint — a healthy remote we are not attached to stays "Connect", never "Disconnect".
function annotateAttached(graph: NCGraph, attachedEndpoint: string | undefined): void {
  const mark = (rt: NCRuntime): void => {
    if (rt.id === LOCAL_RUNTIME_NODE_ID) {
      rt.attached = rt.health === "connected";
      return;
    }
    rt.attached =
      !!rt.controlEndpoint &&
      !!attachedEndpoint &&
      rt.controlEndpoint === attachedEndpoint;
  };
  for (const host of graph.hosts) {
    host.runtimes.forEach(mark);
    host.containers.forEach((container) => container.runtimes.forEach(mark));
  }
}
