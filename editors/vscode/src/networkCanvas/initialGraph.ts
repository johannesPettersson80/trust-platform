import type { LifecyclePhase } from "../lifecycleEntryFailure";
import type { RuntimeLifecycleTarget } from "../runtimeLifecycleModel";
import { getSelectedRuntimeId } from "../selectedRuntime";
import {
  remoteLabelFromEndpoint,
  SIMULATOR_RUNTIME_ID,
} from "../trustHomeModel";
import { buildCanvasGraph } from "./graphData";
import { projectCanvasLifecycleAuthority } from "./lifecycleAuthorityProjection";
import { buildNetworkCanvasModel, type NetworkCanvasStage } from "./model";
import { AUTHORITY_CHECK_RUNTIME_NODE_ID, type NCGraph } from "./webview/types";

/** A synchronous, lifecycle-owned graph for the webview's very first paint. */
export function initialNetworkCanvasGraph(
  phase: LifecyclePhase,
  stage: NetworkCanvasStage = "welcome",
  selectedRunTargetId: string = getSelectedRuntimeId(),
  authorityTarget: RuntimeLifecycleTarget | null | undefined = fallbackTarget(
    phase,
    selectedRunTargetId,
  ),
): NCGraph {
  // `null` explicitly means that an accepted non-simulator session has not
  // yet been checked against managed-runtime inventory. First paint stays
  // read-only until the asynchronous inventory refresh establishes authority.
  const authorityPending = authorityTarget === null;
  const validatedAuthorityTarget = authorityPending
    ? undefined
    : authorityTarget;
  const simulatorOwned = validatedAuthorityTarget?.kind === "simulator";
  const graph = buildCanvasGraph(
    buildNetworkCanvasModel({
      stage,
      starting: phase === "starting" && simulatorOwned,
      runtime:
        phase === "running" && simulatorOwned
          ? {
              running: true,
              runtimeState: "running",
              runtimeMode: "simulate",
            }
          : undefined,
    }),
    undefined,
    undefined,
    undefined,
    [],
    selectedRunTargetId,
  );
  if (authorityPending) {
    return checkingActiveRuntimeGraph(graph);
  }
  return projectCanvasLifecycleAuthority(graph, {
    phase,
    target: validatedAuthorityTarget,
  });
}

function checkingActiveRuntimeGraph(graph: NCGraph): NCGraph {
  const runtime = graph.hosts[0]?.runtimes[0];
  if (runtime) {
    runtime.id = AUTHORITY_CHECK_RUNTIME_NODE_ID;
    runtime.name = "Checking active runtime…";
    runtime.mode = "remote";
    runtime.health = "starting";
    runtime.detail =
      "Checking the current runtime before connection controls are enabled.";
    runtime.runTarget = false;
    runtime.attached = false;
    runtime.controlEndpoint = undefined;
  }
  graph.summary = "Checking active runtime…";
  graph.banner = {
    kind: "info",
    text: "Checking the active runtime and its local inventory…",
    actions: [],
  };
  return graph;
}

function fallbackTarget(
  phase: LifecyclePhase,
  selectedRunTargetId: string,
): RuntimeLifecycleTarget | undefined {
  if (phase === "starting" || phase === "running") {
    return { kind: "simulator" };
  }
  if (phase === "connected" && selectedRunTargetId !== SIMULATOR_RUNTIME_ID) {
    return {
      kind: "remote",
      endpoint: selectedRunTargetId,
      label: remoteLabelFromEndpoint(selectedRunTargetId),
    };
  }
  return undefined;
}
