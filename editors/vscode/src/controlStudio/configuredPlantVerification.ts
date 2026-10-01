import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationRequest } from "./verificationClient";

/** Ordinary validation uses the project's plant; explicit model-variation cases retain their declared model. A cascade's
 * outer loop keeps its model, which contains the closed inner loop: the project's plant would start the program with
 * the inner controller in Manual, away from the outer model's operating point (Specification 35 §1). */
export function configuredPlantRequest(loop: Pick<ControlLoopDefinition, "cascade">, request: RuntimeVerificationRequest): RuntimeVerificationRequest {
  if (loop.cascade?.role === "outer") return request;
  const configured = structuredClone(request);
  delete configured.simulation_override;
  return configured;
}

/** A declared scenario runs on the project's plant unless it supplies its own model, operating outputs or
 * effects; the campaign rejects declared ids that duplicate a generated case, so only generated cases are
 * recognized by their reserved ids. */
export function usesConfiguredPlant(loop: ControlLoopDefinition, scenarioId: string): boolean {
  if (loop.cascade?.role === "outer") return false;
  const declared = loop.verificationScenarios?.find(scenario => scenario.id === scenarioId);
  if (declared) return !(declared.model || declared.operatingOutputs || declared.effects?.length);
  return !["load-rejection", "measurement-noise", "lower-gain", "upper-gain", "constraint-recovery", "state-recovery"].includes(scenarioId);
}
