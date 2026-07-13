import type { LifecyclePhase } from "../lifecycleEntryFailure";
import type { RuntimeLifecycleTarget } from "../runtimeLifecycleModel";
import type { NCGraph, NCHost, NCRuntime } from "./webview/types";
import { LOCAL_RUNTIME_NODE_ID } from "./webview/types";

export interface CanvasLifecycleAuthority {
  readonly phase: LifecyclePhase;
  readonly target?: RuntimeLifecycleTarget;
}

/**
 * Projects the single lifecycle owner without changing the configured fleet.
 * An accepted attach absent from fleet settings is rendered ephemerally; a
 * remote transition never paints the local Simulator as Starting.
 */
export function projectCanvasLifecycleAuthority(
  graph: NCGraph,
  authority: CanvasLifecycleAuthority,
): NCGraph {
  if (!authority.target || authority.phase === "stopped") {
    return graph;
  }

  const runtimes = allRuntimes(graph);
  for (const runtime of runtimes) {
    runtime.runTarget = false;
    runtime.attached = false;
  }

  const owned =
    findOwnedRuntime(runtimes, authority.target) ??
    injectEphemeralRuntime(graph, authority.target, authority.phase);
  if (!owned) {
    return graph;
  }

  owned.runTarget = true;
  owned.attached =
    authority.phase === "running" || authority.phase === "connected";
  switch (authority.phase) {
    case "starting":
      owned.health = "starting";
      owned.detail =
        authority.target.kind === "remote"
          ? `Connecting to ${owned.name}…`
          : `Starting ${owned.name}…`;
      break;
    case "running":
      owned.health = "connected";
      owned.detail = "Running.";
      break;
    case "connected":
      owned.health = "connected";
      owned.detail = `Connected to ${owned.name}.`;
      break;
  }
  graph.summary = lifecycleSummary(graph, authority.phase, owned.name);
  return graph;
}

function findOwnedRuntime(
  runtimes: NCRuntime[],
  target: RuntimeLifecycleTarget,
): NCRuntime | undefined {
  switch (target.kind) {
    case "simulator":
      return runtimes.find((runtime) => runtime.id === LOCAL_RUNTIME_NODE_ID);
    case "remote":
      const endpointMatches = runtimes.filter(
        (runtime) =>
          Boolean(target.endpoint) &&
          runtime.controlEndpoint === target.endpoint,
      );
      return endpointMatches.length === 1 ? endpointMatches[0] : undefined;
    case "managed":
      const managedMatches = runtimes.filter(
        (runtime) =>
          (runtime.managedName === target.id ||
            runtime.id === target.id ||
            runtime.name === target.id) &&
          (!target.endpoint ||
            !runtime.controlEndpoint ||
            runtime.controlEndpoint === target.endpoint),
      );
      return managedMatches.length === 1 ? managedMatches[0] : undefined;
  }
}

function injectEphemeralRuntime(
  graph: NCGraph,
  target: RuntimeLifecycleTarget,
  phase: LifecyclePhase,
): NCRuntime | undefined {
  if (target.kind === "simulator") {
    return undefined;
  }
  const name =
    target.kind === "remote"
      ? target.label?.trim() || endpointLabel(target.endpoint)
      : target.id;
  const runtime: NCRuntime = {
    id:
      target.kind === "remote"
        ? `runtime:active:${target.endpoint}`
        : `managed:${target.id}`,
    name,
    mode: target.kind === "remote" ? "remote" : "managed",
    health: phase === "starting" ? "starting" : "connected",
    detail:
      phase === "starting" ? `Connecting to ${name}…` : `Connected to ${name}.`,
    ...(target.kind === "remote"
      ? { controlEndpoint: target.endpoint }
      : {
          managed: true,
          managedName: target.id,
          ...(target.endpoint ? { controlEndpoint: target.endpoint } : {}),
        }),
    endpoints: [],
  };
  const host = activeSessionHost(graph, target);
  host.runtimes.push(runtime);
  return runtime;
}

function activeSessionHost(
  graph: NCGraph,
  target: Exclude<RuntimeLifecycleTarget, { kind: "simulator" }>,
): NCHost {
  if (target.kind === "managed") {
    const local = graph.hosts.find(
      (host) =>
        host.id === "host:this-computer" || host.hostname === "This computer",
    );
    if (local) {
      return local;
    }
  }
  const id =
    target.kind === "remote"
      ? `host:active:${target.endpoint}`
      : "host:managed-local";
  const host: NCHost = {
    id,
    hostname:
      target.kind === "remote"
        ? endpointLabel(target.endpoint)
        : "This computer",
    label: target.kind === "remote" ? "Active connection" : "managed runtimes",
    health: "connected",
    containers: [],
    runtimes: [],
  };
  graph.hosts.push(host);
  return host;
}

function allRuntimes(graph: NCGraph): NCRuntime[] {
  return graph.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ]);
}

function endpointLabel(endpoint: string): string {
  const withoutScheme = endpoint.trim().replace(/^[a-z]+:\/\//i, "");
  return withoutScheme.split("/")[0] || "Remote runtime";
}

function lifecycleSummary(
  graph: NCGraph,
  phase: Exclude<LifecyclePhase, "stopped">,
  target: string,
): string {
  const runtimes = allRuntimes(graph).length;
  const state =
    phase === "starting"
      ? `Connecting ${target}`
      : phase === "running"
        ? `${target} running`
        : `${target} connected`;
  return `${graph.hosts.length} host${graph.hosts.length === 1 ? "" : "s"} · ${runtimes} runtime${runtimes === 1 ? "" : "s"} · ${state}`;
}
