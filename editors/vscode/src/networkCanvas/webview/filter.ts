import { primaryPlantState } from "../plant/nodeState";
import { canvasGraphSummary, type NCGraph, type NCEndpoint, type NCRuntime } from "./types";

export const SIMULATED_PLANTS_FILTER_ID = "simulated_plants";

// Protocols present anywhere in the graph (for the filter checkboxes).
export function protocolsInGraph(graph: NCGraph): string[] {
  const set = new Set<string>();
  for (const host of graph.hosts) {
    const runtimes = [...host.runtimes, ...host.containers.flatMap((c) => c.runtimes)];
    for (const rt of runtimes) {
      for (const ep of rt.endpoints) {
        set.add(ep.protocol);
      }
    }
  }
  if (plantsInGraph(graph).length > 0) set.add(SIMULATED_PLANTS_FILTER_ID);
  return [...set].sort();
}

export interface FilterReport {
  readonly hiddenEndpointCount: number;
  readonly hiddenPlantCount: number;
  readonly hiddenAttentionCount: number;
  readonly hiddenFaultCount: number;
  readonly hiddenWarningCount: number;
  readonly hiddenErrorCount: number;
}

function endpointsInGraph(graph: NCGraph): NCEndpoint[] {
  const endpoints: NCEndpoint[] = [];
  for (const host of graph.hosts) {
    const runtimes = [...host.runtimes, ...host.containers.flatMap((c) => c.runtimes)];
    for (const rt of runtimes) {
      endpoints.push(...rt.endpoints);
    }
  }
  return endpoints;
}

function plantsInGraph(graph: NCGraph) {
  return graph.hosts.flatMap((host) =>
    [...host.runtimes, ...host.containers.flatMap((container) => container.runtimes)].flatMap(
      (runtime) => runtime.plants ?? []
    )
  );
}

export function graphSummaryFromVisibleGraph(graph: NCGraph): string {
  return canvasGraphSummary(graph);
}

export function withVisibleGraphSummary(graph: NCGraph): NCGraph {
  return { ...graph, summary: graphSummaryFromVisibleGraph(graph) };
}

// Whether a connection needs attention is the server's word (`trust-lsp.devicesTopology`).
function needsAttention(endpoint: NCEndpoint): boolean {
  return endpoint.attention === true;
}

export function filterReport(graph: NCGraph, hidden: ReadonlySet<string>): FilterReport {
  const hiddenEndpoints = endpointsInGraph(graph).filter((ep) => hidden.has(ep.protocol));
  const hiddenPlants = hidden.has(SIMULATED_PLANTS_FILTER_ID)
    ? plantsInGraph(graph).filter((plant) => primaryPlantState(plant) !== "Faulted")
    : [];
  const hiddenEndpointIds = new Set(hiddenEndpoints.map((ep) => ep.id));
  const hiddenFaults = graph.faults.filter((fault) => hiddenEndpointIds.has(fault.targetNodeId));
  return {
    hiddenEndpointCount: hiddenEndpoints.length,
    hiddenPlantCount: hiddenPlants.length,
    hiddenAttentionCount: hiddenEndpoints.filter(needsAttention).length,
    hiddenFaultCount: hiddenFaults.length,
    hiddenWarningCount: hiddenFaults.filter((fault) => fault.severity === "warning").length,
    hiddenErrorCount: hiddenFaults.filter((fault) => fault.severity === "error").length,
  };
}

// Hide endpoints (and their links + orphaned externals) whose protocol is filtered out.
export function applyFilter(graph: NCGraph, hidden: ReadonlySet<string>): NCGraph {
  if (hidden.size === 0) {
    return withVisibleGraphSummary(graph);
  }
  const hiddenEndpoints = new Set<string>();
  const keepRuntime = (rt: NCRuntime): NCRuntime => ({
    ...rt,
    endpoints: rt.endpoints.filter((ep) => {
      if (hidden.has(ep.protocol)) {
        hiddenEndpoints.add(ep.id);
        return false;
      }
      return true;
    }),
    plants: hidden.has(SIMULATED_PLANTS_FILTER_ID)
      ? (rt.plants ?? []).filter((plant) => primaryPlantState(plant) === "Faulted")
      : rt.plants,
  });
  const hosts = graph.hosts.map((host) => ({
    ...host,
    runtimes: host.runtimes.map(keepRuntime),
    containers: host.containers.map((c) => ({ ...c, runtimes: c.runtimes.map(keepRuntime) })),
  }));
  const links = graph.links.filter(
    (l) => !hiddenEndpoints.has(l.from) && !hiddenEndpoints.has(l.to)
  );
  const referenced = new Set<string>();
  for (const l of links) {
    referenced.add(l.from);
    referenced.add(l.to);
  }
  const external = graph.external.filter((x) => referenced.has(x.id));
  return withVisibleGraphSummary({ ...graph, hosts, links, external });
}
