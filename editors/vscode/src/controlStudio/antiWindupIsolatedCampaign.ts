import type { AntiWindupCandidateReview } from "./antiWindupCampaign";
import { compareAntiWindupRuns, type AntiWindupComparison, type AntiWindupComparisonRun } from "./antiWindupComparison";
import { antiWindupRunFromRecordedSt } from "./antiWindupRecordedRun";
import { buildAntiWindupVerificationPlan } from "./antiWindupVerificationPlan";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RunRequirement } from "./runValidation";
import type { SaturationWindupAssessment } from "./saturationWindup";
import type { RuntimeVerificationRequest, RuntimeVerificationResult } from "./verificationClient";

export interface AntiWindupIsolatedCampaignResult {
  readonly baseline: RuntimeVerificationResult;
  readonly candidate: RuntimeVerificationResult;
  readonly comparison: AntiWindupComparison;
}

/** Executes the reviewed untracked/tracked records sequentially against one isolated ST snapshot. */
export async function runAntiWindupIsolatedCampaign(input: {
  readonly loop: ControlLoopDefinition;
  readonly candidate: AntiWindupCandidateReview;
  readonly saturationWindup: SaturationWindupAssessment;
  readonly verify: (request: RuntimeVerificationRequest, requirements: readonly RunRequirement[]) => Promise<RuntimeVerificationResult>;
  readonly toEvidence?: (loop: ControlLoopDefinition, run: RuntimeVerificationResult["run"]) => AntiWindupComparisonRun;
  readonly compare?: (input: { baseline: AntiWindupComparisonRun; candidate: AntiWindupComparisonRun }) => AntiWindupComparison;
}): Promise<AntiWindupIsolatedCampaignResult> {
  const plan = buildAntiWindupVerificationPlan({
    loop: input.loop,
    baselineValues: input.candidate.review.observed.values,
    trackingTimeSeconds: input.candidate.trackingTimeSeconds,
    saturationWindup: input.saturationWindup,
  });
  const baseline = await input.verify(plan.baseline, plan.requirements);
  const candidate = await input.verify(plan.candidate, plan.requirements);
  if (baseline.projectRevision !== candidate.projectRevision) {
    throw new Error("The project changed between the baseline and tracked actual-ST runs.");
  }
  const toEvidence = input.toEvidence ?? antiWindupRunFromRecordedSt;
  const compare = input.compare ?? compareAntiWindupRuns;
  return { baseline, candidate, comparison: compare({ baseline: toEvidence(input.loop, baseline.run), candidate: toEvidence(input.loop, candidate.run) }) };
}
