import type { RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import {
  decodePlantContractResponse,
  type PlantContractMessage,
} from "../networkCanvas/plant/contract";
import type { PlantControlStudioHandoff } from "./handoff";

export type PlantMonitorUnavailableReason =
  | "Runtime is stopped"
  | "Runtime is unreachable"
  | "Runtime authorization was rejected"
  | "Plant status is unsupported"
  | "Plant status is malformed"
  | "Plant no longer exists"
  | "Running plant revision is stale";

export type PlantMonitorSnapshot =
  | {
      availability: "unavailable";
      source: "Simulation";
      plantId: string;
      reason: PlantMonitorUnavailableReason;
    }
  | {
      availability: "available";
      source: "Simulation";
      plantId: string;
      activeConfigurationRevision: string;
      runtimeVersion: string;
      simulationTimeNs: string;
      operatingState: "running" | "faulted";
      model: string;
      command: number[];
      appliedInput: number[];
      idealOutput: number[];
      measuredFeedback: number[];
      publishedFeedback: number[];
      state: number[];
      timeScale: number;
      effectiveSeed: string;
      randomAlgorithm: string;
      randomAlgorithmVersion: number;
      actuators: unknown[];
      activeUncertainties: string[];
      activeDropouts: string[];
      pendingEventCount: number;
      delayBufferDepth: number;
      lastSubsteps: number;
      currentFault: string | null;
    };

export interface PlantMonitorDependencies {
  lifecycleSnapshot(): Promise<RuntimeLifecycleSnapshot>;
  authToken(endpoint: string): Promise<string | undefined>;
  request(
    endpoint: string,
    authToken: string | undefined,
    operation: string,
    params: Record<string, unknown>
  ): Promise<unknown>;
}

export async function loadPlantMonitor(
  handoff: PlantControlStudioHandoff,
  dependencies: PlantMonitorDependencies
): Promise<PlantMonitorSnapshot> {
  const lifecycle = await dependencies.lifecycleSnapshot();
  if (lifecycle.status.runtimeState === "stopped" && !lifecycle.status.endpointReachable) {
    return unavailable(handoff, "Runtime is stopped");
  }
  const endpoint = lifecycle.status.endpoint.trim();
  if (!endpoint) return unavailable(handoff, "Runtime is unreachable");
  let response: unknown;
  try {
    response = await dependencies.request(
      endpoint,
      await dependencies.authToken(endpoint),
      "plant.status",
      {
        contract: "trust.plant",
        schema_version: 1,
        project_id: handoff.projectId,
      }
    );
  } catch (error) {
    const message = error instanceof Error ? error.message.toLowerCase() : String(error).toLowerCase();
    return unavailable(
      handoff,
      message.includes("auth") || message.includes("unauthorized")
        ? "Runtime authorization was rejected"
        : message.includes("unsupported")
          ? "Plant status is unsupported"
          : "Runtime is unreachable"
    );
  }
  const decoded = decodePlantContractResponse(response, handoff.projectId);
  if (decoded.kind !== "accepted" || decoded.message.operation !== "plant.status") {
    return unavailable(handoff, "Plant status is malformed");
  }
  return projectPlantMonitor(decoded.message, handoff);
}

export function projectPlantMonitor(
  status: PlantContractMessage,
  handoff: Pick<PlantControlStudioHandoff, "plantId" | "activeConfigurationDigest">
): PlantMonitorSnapshot {
  if (
    handoff.activeConfigurationDigest &&
    status.active_configuration_revision !== handoff.activeConfigurationDigest
  ) return unavailable(handoff, "Running plant revision is stale");
  const plants = Array.isArray(status.plants) ? status.plants : [];
  const plant = plants.find((entry) => record(entry)?.id === handoff.plantId);
  const value = record(plant);
  if (!value) return unavailable(handoff, "Plant no longer exists");
  return {
    availability: "available",
    source: "Simulation",
    plantId: handoff.plantId,
    activeConfigurationRevision: status.active_configuration_revision as string,
    runtimeVersion: status.runtime_version as string,
    simulationTimeNs: value.simulation_time_ns as string,
    operatingState: value.operating_state as "running" | "faulted",
    model: value.model as string,
    command: value.command as number[],
    appliedInput: value.applied_input as number[],
    idealOutput: value.ideal_output as number[],
    measuredFeedback: value.measured_feedback as number[],
    publishedFeedback: value.published_feedback as number[],
    state: value.state as number[],
    timeScale: status.time_scale as number,
    effectiveSeed: status.effective_seed as string,
    randomAlgorithm: status.random_algorithm as string,
    randomAlgorithmVersion: status.random_algorithm_version as number,
    actuators: value.actuators as unknown[],
    activeUncertainties: value.active_uncertainties as string[],
    activeDropouts: value.active_dropouts as string[],
    pendingEventCount: value.pending_event_count as number,
    delayBufferDepth: value.delay_buffer_depth as number,
    lastSubsteps: value.last_substeps as number,
    currentFault: value.current_fault as string | null,
  };
}

function unavailable(
  handoff: Pick<PlantControlStudioHandoff, "plantId">,
  reason: PlantMonitorUnavailableReason
): PlantMonitorSnapshot {
  return { availability: "unavailable", source: "Simulation", plantId: handoff.plantId, reason };
}

function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}
