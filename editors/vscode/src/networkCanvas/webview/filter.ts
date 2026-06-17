import type { NCGraph, NCRuntime } from "./types";

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

// Hide endpoints (and their links + orphaned externals) whose protocol is filtered out.
export function applyFilter(graph: NCGraph, hidden: ReadonlySet<string>): NCGraph {
  if (hidden.size === 0) {
    return graph;
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
  return { ...graph, hosts, links, external };
}
