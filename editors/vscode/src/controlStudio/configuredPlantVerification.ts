import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationRequest } from "./verificationClient";

/** A configured simulated plant's bindings, as the project's saved configuration states them (`plant.read`, simulation.toml `[[plants]]`). */
export interface ConfiguredPlantBinding {
  readonly id: string;
  /** Addresses the plant reads its inputs from: the loop's plant input must be one of them. */
  readonly commandSources: readonly string[];
  /** Addresses the plant writes its measurements to: the loop's measured output must be one of them. */
  readonly feedbackTargets: readonly string[];
}

/** The plant an isolated Validate run runs against (Specification 35, B5.3); its evidence names this choice (A9). */
export type VerificationPlant = { readonly kind: "configured"; readonly plantId: string } | { readonly kind: "model" };

type LoopPlantSignals = Pick<ControlLoopDefinition, "plantId" | "signals">;

/**
 * The configured plant drives the loop when the plant's feedback reaches the loop's measured output and the loop's plant input
 * reaches the plant's commands. Couplings between plants are not followed: only direct bindings count.
 */
export function configuredPlantDrivesLoop(loop: LoopPlantSignals, plant: ConfiguredPlantBinding | undefined): boolean {
  if (!plant || !loop.plantId || plant.id !== loop.plantId) return false;
  const measured = normalizedAddress(loop.signals.processVariable.address);
  const input = normalizedAddress(loop.signals.manipulatedVariable.address);
  return measured !== undefined && input !== undefined
    && plant.feedbackTargets.some(target => normalizedAddress(target) === measured)
    && plant.commandSources.some(source => normalizedAddress(source) === input);
}

/** B5.3: the configured plant when the target is simulated and that plant drives the loop's signals; otherwise the qualified model. */
export function verificationPlant(loop: LoopPlantSignals, target: { readonly simulated: boolean },
  plant: ConfiguredPlantBinding | undefined): VerificationPlant {
  if (!target.simulated || !plant || !configuredPlantDrivesLoop(loop, plant)) return { kind: "model" };
  return { kind: "configured", plantId: plant.id };
}

/**
 * Reads the loop's configured plant from the connected runtime (`plant.read`). A runtime that cannot answer, or a plant the
 * saved configuration does not describe, leaves no binding: the model is then the plant that runs.
 */
export async function readConfiguredPlantBinding(request: (operation: string, params: Record<string, unknown>) => Promise<unknown>,
  plantId: string | undefined): Promise<ConfiguredPlantBinding | undefined> {
  if (!plantId) return undefined;
  let response: unknown;
  try { response = await request("plant.read", { contract: "trust.plant", schema_version: 1 }); }
  catch { return undefined; }
  const plants: unknown[] = isRecord(response) && Array.isArray(response.plants) ? response.plants : [];
  const plant = plants.find((entry): entry is Record<string, unknown> => isRecord(entry) && entry.id === plantId);
  if (!plant || !isStringArray(plant.command_sources) || !isStringArray(plant.feedback_targets)) return undefined;
  return { id: plantId, commandSources: plant.command_sources, feedbackTargets: plant.feedback_targets };
}

/** Ordinary validation runs on the plant the choice names; a model choice keeps each request's own model (B5.3). */
export function configuredPlantRequest(loop: Pick<ControlLoopDefinition, "cascade">, request: RuntimeVerificationRequest,
  plant: VerificationPlant): RuntimeVerificationRequest {
  // A cascade's outer loop keeps its model, which contains the closed inner loop (Specification 35 §1).
  if (plant.kind === "model" || loop.cascade?.role === "outer") return request;
  const configured = structuredClone(request);
  delete configured.simulation_override;
  return configured;
}

/** A declared scenario runs on the chosen plant unless it supplies its own model, operating outputs or effects. */
export function usesConfiguredPlant(loop: ControlLoopDefinition, scenarioId: string, plant: VerificationPlant): boolean {
  if (plant.kind === "model" || loop.cascade?.role === "outer") return false;
  const declared = loop.verificationScenarios?.find(scenario => scenario.id === scenarioId);
  if (declared) return !(declared.model || declared.operatingOutputs || declared.effects?.length);
  // Generated model-variation cases keep their model, so only generated cases are recognized by their reserved ids.
  return !["load-rejection", "measurement-noise", "lower-gain", "upper-gain", "constraint-recovery", "state-recovery"].includes(scenarioId);
}

/** The evidence name of an isolated run's plant (A9): the configured plant's id, or the model with its revision. */
export function isolatedPlantName(plant: VerificationPlant, model: { readonly revision: string }): string {
  return plant.kind === "configured" ? `configured plant ${plant.plantId}` : `model ${model.revision}`;
}

function normalizedAddress(address: string | undefined): string | undefined {
  const text = address?.trim().toUpperCase();
  return text ? text : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(item => typeof item === "string");
}
