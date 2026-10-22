import type { RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import type { PlantControlStudioHandoff } from "./handoff";
import type { PlantMonitorPayload } from "./messages";
import type { PlantMonitorRequest } from "./generated/PlantMonitorRequest";
import type { PlantMonitorResult } from "./generated/PlantMonitorResult";
import type { EngineAnswer } from "./engine/engineClient";

/**
 * The plant summary the panel posts: the engine's answer to `controlStudio/plantMonitor` (specification 35, §0.1a), with its own
 * source. The engine reads the runtime's `plant.status` and decides whether the plant is available, and why it is not. When the engine
 * answers a failure instead, the summary is that failure's sentence and names no source.
 */
export type PlantMonitorSnapshot = PlantMonitorPayload;

export interface PlantMonitorDependencies {
  /** The lifecycle service's facts: the runtime's state, whether its endpoint answers, and the endpoint. */
  lifecycleSnapshot(): Promise<RuntimeLifecycleSnapshot>;
  authToken(endpoint: string): Promise<string | undefined>;
  monitor(request: PlantMonitorRequest): Promise<EngineAnswer<PlantMonitorResult>>;
}

/**
 * Asks the engine for the plant summary. The lifecycle service gives the facts the engine needs (the runtime's state, its endpoint
 * and the control token); the engine decides the summary. An engine that cannot answer shows the plant as unavailable, with the
 * sentence its failure gives.
 */
export async function loadPlantMonitor(
  handoff: PlantControlStudioHandoff,
  dependencies: PlantMonitorDependencies
): Promise<PlantMonitorSnapshot> {
  try {
    const lifecycle = await dependencies.lifecycleSnapshot();
    const endpoint = lifecycle.status.endpoint;
    const token = await dependencies.authToken(endpoint);
    const answer = await dependencies.monitor({
      projectId: handoff.projectId,
      plantId: handoff.plantId,
      activeConfigurationDigest: handoff.activeConfigurationDigest,
      runtimeStopped: lifecycle.status.runtimeState === "stopped",
      endpointReachable: lifecycle.status.endpointReachable,
      connection: { endpoint, ...(token ? { token } : {}) },
    });
    return "result" in answer
      ? answer.result
      : unavailable(handoff, answer.failure.detail || answer.failure.message);
  } catch (error) {
    return unavailable(handoff, error instanceof Error ? error.message : String(error));
  }
}

function unavailable(handoff: Pick<PlantControlStudioHandoff, "plantId">, reason: string): PlantMonitorSnapshot {
  return { availability: "unavailable", plantId: handoff.plantId, reason };
}
