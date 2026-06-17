// Maps the honest NetworkCanvasModel into the React Flow canvas graph contract.
// One canvas, always populated: a live fleet, or — for a new/unconfigured
// project — the local simulator runtime node itself (never a separate screen).
import type { FleetTopologyResponse } from "./fleetTopology";
import type {
  NetworkCanvasDevice,
  NetworkCanvasModel,
  NetworkDeviceStatus,
} from "./model";
import type { NetworkCanvasFleetRuntime } from "./fleetModel";
import type { NCFault, NCGraph, NCRuntime } from "./webview/types";

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
    endpoints: rt.endpoints.map((ep) => ({
      id: ep.id,
      kind: ep.kind,
      protocol: ep.protocol,
      name: ep.name,
      role: ep.role,
      health: ep.health,
      detail: ep.detail,
      dimmed: ep.dimmed,
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
    title: "Network Canvas",
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
    external: (topology?.external ?? []).map((ext) => ({
      id: ext.id,
      name: ext.name,
      kind: ext.kind,
    })),
    faults: mapFaults(model),
    searchQuery: model.searchQuery,
  };
}

// New/unconfigured project: the local simulator IS the canvas. Always a node,
// honest health (pending until proven, error on start failure) — no screen.
function localRuntimeGraph(model: NetworkCanvasModel): NCGraph {
  const failure = model.failure;
  const running = model.runtime.state === "running";
  const health = failure ? "error" : running ? "connected" : "pending";
  const detail = failure
    ? failure.message
    : running
      ? model.runtime.statusText
      : "Starting local simulator…";
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
      targetNodeId: "runtime:local",
      severity: "error",
    });
  }
  return {
    kind: "graph",
    title: "Network Canvas",
    summary: running
      ? `1 host · 1 runtime · ${endpoints.length} endpoint${endpoints.length === 1 ? "" : "s"}`
      : "Local simulator",
    hosts: [
      {
        id: "host:this-computer",
        hostname: model.runtime.hostLabel || "this computer",
        label: "local host · simulator",
        health,
        containers: [],
        runtimes: [
          {
            id: "runtime:local",
            name: model.runtime.name || "Local simulator",
            mode: model.runtime.mode,
            health,
            detail,
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
          text: failure.message,
          actions: [
            { label: "Retry", action: "startLocalSimulator" },
            { label: "Open logs", action: "openRuntimeLogs" },
          ],
        }
      : undefined,
    searchQuery: model.searchQuery,
  };
}

export function buildCanvasGraph(
  model: NetworkCanvasModel,
  topology: FleetTopologyResponse | undefined
): NCGraph {
  if (model.fleet) {
    return fleetGraph(model, topology);
  }
  return localRuntimeGraph(model);
}
