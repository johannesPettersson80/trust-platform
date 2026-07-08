import type { NCGraph, NCEndpoint, NCRuntime } from "./types";

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
  return [...set].sort();
}

export interface FilterReport {
  readonly hiddenEndpointCount: number;
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

export function graphSummaryFromVisibleGraph(graph: NCGraph): string {
  const hostCount = graph.hosts.length;
  const runtimeCount = graph.hosts.reduce(
    (sum, host) =>
      sum +
      host.runtimes.length +
      host.containers.reduce((nested, container) => nested + container.runtimes.length, 0),
    0
  );
  const endpointCount = endpointsInGraph(graph).length;
  return `${hostCount} host${hostCount === 1 ? "" : "s"} · ${runtimeCount} runtime${runtimeCount === 1 ? "" : "s"} · ${endpointCount} endpoint${endpointCount === 1 ? "" : "s"}`;
}

export function withVisibleGraphSummary(graph: NCGraph): NCGraph {
  return { ...graph, summary: graphSummaryFromVisibleGraph(graph) };
}

function needsAttention(endpoint: NCEndpoint): boolean {
  switch (endpoint.health) {
    case "connected":
    case "configured":
    case "configured_policy":
    case "disabled":
    case "stopped":
    case "simulate":
      return false;
    default:
      return endpoint.health.trim().length > 0;
  }
}

export function filterReport(graph: NCGraph, hidden: ReadonlySet<string>): FilterReport {
  const hiddenEndpoints = endpointsInGraph(graph).filter((ep) => hidden.has(ep.protocol));
  const hiddenEndpointIds = new Set(hiddenEndpoints.map((ep) => ep.id));
  const hiddenFaults = graph.faults.filter((fault) => hiddenEndpointIds.has(fault.targetNodeId));
  return {
    hiddenEndpointCount: hiddenEndpoints.length,
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
