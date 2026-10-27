// Maps the graph that `trust-lsp.devicesTopology` answers into the React Flow canvas graph contract.
// Every rule of the graph is the server's (crates/trust-devices/src/canvas; specification 25 §5.15):
// the drawn simulator and its connections, the managed runtimes, the session's marks, a failed
// Connect, the faults, the banner and the summary. Here the answer gets the canvas's title, the
// simulator's speed read and the project's simulated plants.
import type { CanvasFleet, CanvasFleetRuntime } from "./fleetModel";
import { canvasGraphSummary, LOCAL_RUNTIME_NODE_ID } from "./webview/types";
import { latestSimulatorSpeedRead } from "../simulationSpeed";
import type { NCGraph, NCPlant, NCRuntime } from "./webview/types";

export { canvasGraphSummary, runtimeEmptyMessage } from "./webview/types";

export function attachPlantsToRuntime(
  graph: NCGraph,
  runtimeId: string,
  plants: ReadonlyArray<NCPlant>
): void {
  for (const host of graph.hosts) {
    const runtime = [
      ...host.runtimes,
      ...host.containers.flatMap((container) => container.runtimes),
    ].find((candidate) => candidate.id === runtimeId);
    if (!runtime) continue;
    runtime.plants = plants.map((plant) => ({ ...plant }));
    graph.summary = canvasGraphSummary(graph);
    return;
  }
}

export function projectPlantRuntimeId(graph: NCGraph): string | undefined {
  const runtimes = graph.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ]);
  const local = runtimes.find((runtime) => runtime.id === LOCAL_RUNTIME_NODE_ID);
  if (local) return local.id;
  const selected = runtimes.filter((runtime) => runtime.runTarget);
  if (selected.length === 1) return selected[0].id;
  return runtimes.length === 1 ? runtimes[0].id : undefined;
}

function mapRuntime(rt: CanvasFleetRuntime): NCRuntime {
  const runtime: NCRuntime = {
    id: rt.id,
    name: rt.name,
    mode: rt.mode,
    health: rt.health,
    lifecycleState: rt.lifecycleState,
    detail: rt.detail,
    controlEndpoint: rt.controlEndpoint,
    attached: rt.attached,
    managed: rt.managed,
    managedName: rt.managedName,
    runTarget: rt.runTarget,
    identify: rt.identify,
    controls: rt.controls as NCRuntime["controls"],
    discovery: rt.discovery,
    endpoints: rt.endpoints.map((ep) => ({
      id: ep.id,
      kind: ep.kind,
      protocol: ep.protocol,
      name: ep.name,
      role: ep.role,
      health: ep.health,
      detail: ep.detail,
      dimmed: ep.dimmed,
      live: ep.live,
      connector: ep.connector,
      params: ep.params,
      browseTarget: ep.browseTarget,
      attention: ep.attention,
      adsSummary: ep.adsSummary,
      children: ep.children ? ep.children.map((s) => ({ ...s })) : undefined,
    })),
  };
  // The speed is shown only while the drawn simulator runs, from its latest status read
  // (specification 35, B9.2); a stopped simulator carries none.
  const speed =
    rt.id === LOCAL_RUNTIME_NODE_ID && rt.health === "connected"
      ? latestSimulatorSpeedRead()
      : undefined;
  if (speed) {
    runtime.simulationTimeScale = speed.configured;
    runtime.simulationAchievedTimeScale = speed.achieved;
    runtime.simulationSpeedText = speed.speedText;
  }
  return runtime;
}

/**
 * The canvas graph of the server's answer. Without an answer (the language server does not run) the
 * canvas draws nothing but the reason as its banner.
 */
export function canvasGraph(
  fleet: CanvasFleet | undefined,
  options: { readonly searchQuery?: string; readonly topologyError?: string } = {}
): NCGraph {
  if (!fleet) {
    return {
      kind: "graph",
      title: "Devices & Connections",
      summary: "",
      hosts: [],
      links: [],
      external: [],
      faults: [],
      banner: options.topologyError
        ? { kind: "error", text: options.topologyError, actions: [] }
        : undefined,
      searchQuery: options.searchQuery,
    };
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
      runtimes: host.runtimes.map(mapRuntime),
      containers: host.containers.map((container) => ({
        id: container.id,
        name: container.name,
        image: container.image,
        status: container.status,
        runtimes: container.runtimes.map(mapRuntime),
      })),
    })),
    links: fleet.links.map((link) => ({ ...link })),
    external: fleet.external.map((external) => ({ ...external })),
    faults: fleet.faults.map((fault) => ({ ...fault })),
    banner: fleet.banner
      ? { kind: fleet.banner.kind, text: fleet.banner.text, actions: fleet.banner.actions.map((action) => ({ ...action })) }
      : undefined,
    searchQuery: options.searchQuery,
  };
}
