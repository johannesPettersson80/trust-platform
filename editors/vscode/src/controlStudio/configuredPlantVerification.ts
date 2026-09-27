import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationRequest } from "./verificationClient";

/** Ordinary validation uses the project's plant; explicit model-variation cases retain their declared model. */
export function configuredPlantRequest(request: RuntimeVerificationRequest): RuntimeVerificationRequest {
  const configured = structuredClone(request);
  delete configured.simulation_override;
  return configured;
}

export function usesConfiguredPlant(loop: ControlLoopDefinition, scenarioId: string): boolean {
  const declared = loop.verificationScenarios?.find(scenario => scenario.id === scenarioId);
  if (declared?.model || declared?.operatingOutputs || declared?.effects?.length) return false;
  return !["load-rejection", "measurement-noise", "lower-gain", "upper-gain", "constraint-recovery", "state-recovery"].includes(scenarioId);
}
