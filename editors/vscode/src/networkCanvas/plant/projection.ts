import type { NCPlant } from "../webview/types";
import type { PlantContractMessage } from "./contract";
import { scenarioEventsForPlant } from "./scenarioEventEditor";
import { preservedPlantConfiguration } from "./preservedConfiguration";
import { plantModelDescriptor } from "./modelDescriptors";

export interface PlantTopologyService {
  read(): Promise<PlantContractMessage>;
  status(projectId: string): Promise<PlantContractMessage>;
}

export async function loadPlantTopology(service: PlantTopologyService): Promise<NCPlant[]> {
  const saved = await service.read();
  try {
    const status = await service.status(saved.project_id);
    return normalizePlantTopology(saved, status, true);
  } catch {
    return normalizePlantTopology(saved, undefined, true);
  }
}

/** Normalizes decoded service responses into the webview's plant topology contract. */
export function normalizePlantTopology(
  saved: PlantContractMessage,
  status?: PlantContractMessage,
  statusRequested = false
): NCPlant[] {
  const savedPlants = records(saved.plants);
  const savedConfiguration = record(saved.saved_configuration);
  const configuredProjectStep = record(savedConfiguration?.simulation)?.plant_step_ms;
  const projectStepMs = savedConfiguration && configuredProjectStep === undefined ? 10
    : typeof configuredProjectStep === "number" && Number.isSafeInteger(configuredProjectStep) && configuredProjectStep > 0
      ? configuredProjectStep : undefined;
  const configuredPlants = records(savedConfiguration?.plants);
  const statusIsCurrent =
    status?.operation === "plant.status" &&
    status.project_id === saved.project_id &&
    status.available === true;
  const statusPlants = statusIsCurrent ? records(status.plants) : [];
  const liveById = new Map(
    statusPlants
      .filter((plant) => typeof plant.id === "string")
      .map((plant) => [plant.id as string, plant] as const)
  );

  return savedPlants.flatMap((descriptor) => {
    if (
      typeof descriptor.id !== "string" ||
      typeof descriptor.display_name !== "string" ||
      typeof descriptor.model !== "string" ||
      typeof descriptor.enabled !== "boolean" ||
      !stringArray(descriptor.command_sources) ||
      !stringArray(descriptor.feedback_targets)
    ) {
      return [];
    }
    const live = liveById.get(descriptor.id);
    const configured = configuredPlants.find((candidate) => candidate.id === descriptor.id);
    const plant: NCPlant = {
      id: descriptor.id,
      projectId: saved.project_id,
      displayName: descriptor.display_name,
      model: descriptor.model,
      enabled: descriptor.enabled,
      commandSources: descriptor.command_sources,
      feedbackTargets: descriptor.feedback_targets,
      statusAvailability:
        statusIsCurrent && live
          ? "available"
          : statusRequested
            ? "unavailable"
            : "not_requested",
      savedConfigurationRevision:
        typeof saved.configuration_revision === "string"
          ? saved.configuration_revision
          : undefined,
      activeConfigurationRevision:
        typeof status?.active_configuration_revision === "string"
          ? status.active_configuration_revision
          : typeof saved.active_configuration_revision === "string"
            ? saved.active_configuration_revision
            : undefined,
    };
    const parameters = configured ? numericRecord(configured.parameters) : undefined;
    const enabledDefaulted = configured !== undefined && configured.enabled === undefined;
    const initialStateDefaulted = configured !== undefined && configured.initial_state === undefined;
    const stateCount = descriptor.model === "state_space"
      ? (Array.isArray(parameters?.a) ? parameters.a.length : 0)
      : plantModelDescriptor(descriptor.model).states;
    const initialState = initialStateDefaulted
      ? Array<number>(stateCount).fill(0)
      : configured ? numberArray(configured.initial_state) : undefined;
    if (configured && parameters && initialState) {
      plant.configuration = {
        description: typeof configured.description === "string" ? configured.description : "",
        parameters,
        initialState,
        ...(projectStepMs === undefined ? {} : { projectStepMs }),
        ...(enabledDefaulted ? { enabledDefaulted: true } : {}),
        ...(initialStateDefaulted ? { initialStateDefaulted: true } : {}),
      };
      const preserved = preservedPlantConfiguration(configured);
      if (Object.keys(preserved).length > 0) plant.configuration.preservedConfiguration = preserved;
      if (typeof configured.step_ms === "number") plant.configuration.stepMs = configured.step_ms;
      for (const [key, value] of [
        ["initialInput", numberArray(configured.initial_input)],
        ["inputScale", numberArray(configured.input_scale)],
        ["inputOffset", numberArray(configured.input_offset)],
        ["outputScale", numberArray(configured.output_scale)],
        ["outputOffset", numberArray(configured.output_offset)],
      ] as const) {
        if (value) plant.configuration[key] = value;
      }
      const actuator = numericScalarRecord(configured.actuator);
      if (actuator && Object.keys(actuator).length > 0) plant.configuration.actuator = actuator;
      if (Array.isArray(configured.uncertainties)) {
        plant.configuration.uncertainties = records(configured.uncertainties).map((effect) => ({ ...effect, enabled: true }));
      }
      plant.configuration.scenarioEvents = scenarioEventsForPlant(savedConfiguration, descriptor.id);
    }
    if (!live) return [plant];

    if (live.operating_state === "running" || live.operating_state === "faulted") {
      plant.activeOperatingState = live.operating_state;
    }
    const command = numberArray(live.command);
    const feedback = numberArray(live.published_feedback);
    if (command && feedback) {
      plant.hasCompletedSample = true;
      plant.liveSample = { command, feedback };
    }
    return [plant];
  });
}

function records(value: unknown): Record<string, unknown>[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (entry): entry is Record<string, unknown> =>
      typeof entry === "object" && entry !== null && !Array.isArray(entry)
  );
}

function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function numericRecord(value: unknown): Record<string, number | number[] | number[][]> | undefined {
  const source = record(value);
  if (!source) return undefined;
  const result: Record<string, number | number[] | number[][]> = {};
  for (const [key, entry] of Object.entries(source)) {
    const numbers = Array.isArray(entry) ? entry.flat(Infinity) : [entry];
    if (numbers.some((number) => typeof number !== "number" || !Number.isFinite(number))) return undefined;
    result[key] = entry as number | number[] | number[][];
  }
  return result;
}

function numericScalarRecord(value: unknown): Record<string, number> | undefined {
  const source = record(value);
  if (!source) return undefined;
  const result: Record<string, number> = {};
  for (const [key, entry] of Object.entries(source)) {
    if (typeof entry !== "number" || !Number.isFinite(entry)) return undefined;
    result[key] = entry;
  }
  return result;
}

function stringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string");
}

function numberArray(value: unknown): number[] | undefined {
  return Array.isArray(value) &&
    value.every((entry) => typeof entry === "number" && Number.isFinite(entry))
    ? value
    : undefined;
}
