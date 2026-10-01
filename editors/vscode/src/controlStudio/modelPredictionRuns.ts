import { engineerVerificationCampaign } from "./engineerVerificationCampaign";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";
import type { SearchConnection } from "./finiteSearchSession";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { RuntimeVerificationClient, type RuntimeVerificationAttempt, type RuntimeVerificationResult } from "./verificationClient";

/** What a model comparison needs from its owner: the loop, where its verifier and runs go, and whether it is still current. */
export interface ModelComparisonOwner {
  readonly loop: ControlLoopDefinition;
  scenarioLoop(): ControlLoopDefinition;
  readonly retainIsolatedOperation?: (attempt: RuntimeVerificationAttempt) => Promise<void>;
  adopt(verifier: RuntimeVerificationClient): void;
  record(run: RuntimeVerificationResult): void;
  current(): boolean;
}

/**
 * Runs the program's actual ST with the current and the given settings on the selected model, in isolated simulation:
 * Validate's model comparison, and Tune's prediction for a law Studio does not calculate itself (Specification 35 §9).
 */
export async function compareOnModel(owner: ModelComparisonOwner, connection: SearchConnection, model: GeneralModel,
  task: EngineerRequirements, observed: Readonly<Record<string, number>>, values: Readonly<Record<string, number>>) {
  const baseline = engineerVerificationPlan(owner.loop, model, task, observed);
  const candidate = engineerVerificationCampaign(owner.scenarioLoop(), model, task, values)[0];
  const verifier = new RuntimeVerificationClient({ source: "simulation", request: connection.request }, owner.retainIsolatedOperation);
  owner.adopt(verifier);
  const predictedBaseline = await verifier.verify(baseline.request, baseline.requirements);
  if (!owner.current()) return undefined;
  owner.record(predictedBaseline);
  const predictedCandidate = await verifier.verify(candidate.request, candidate.requirements);
  if (!owner.current()) return undefined;
  owner.record(predictedCandidate);
  if (predictedBaseline.projectRevision !== predictedCandidate.projectRevision) throw new Error("The project changed during model prediction.");
  return { verifier, baseline, candidate, predictedBaseline, predictedCandidate };
}
