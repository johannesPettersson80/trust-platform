import type { SaturationWindupAssessment } from "./saturationWindup";

export interface SaturationReturnConditions {
  readonly loopId: string;
  readonly loopRevision: string;
  readonly source: "simulation" | "physical";
  readonly scenarioDigest: string;
  readonly plantInitialConditionDigest: string;
  readonly controllerInitialConditionDigest: string;
  readonly samplePeriodMs: number;
  readonly durationMs: number;
  readonly outputMinimum: number;
  readonly outputMaximum: number;
}

export interface AntiWindupComparisonRun {
  readonly trackingGain: number;
  readonly conditions: SaturationReturnConditions;
  readonly assessment: SaturationWindupAssessment;
  readonly oscillationDetected: boolean;
}

export type AntiWindupComparison =
  | {
    readonly status: "qualified";
    readonly recommendation: "recommend_tracking" | "do_not_recommend_tracking";
    readonly baselineRecoverySeconds: number;
    readonly candidateRecoverySeconds: number;
    readonly baselineIntegralChange: number;
    readonly candidateIntegralChange: number;
    readonly recoveryImprovementSeconds: number;
    readonly integralReduction: number;
    readonly reason: string;
  }
  | { readonly status: "inconclusive"; readonly reason: string };

/**
 * Compares only immutable matched saturation-return trials. The caller must
 * obtain the digests from the executed runtime scenario; this helper never
 * manufactures initial-state or scenario identity.
 */
export function compareAntiWindupRuns(input: {
  readonly baseline: AntiWindupComparisonRun;
  readonly candidate: AntiWindupComparisonRun;
}): AntiWindupComparison {
  const conditions = matchingConditions(input.baseline.conditions, input.candidate.conditions);
  if (conditions) return { status: "inconclusive", reason: conditions };
  if (input.baseline.trackingGain !== 0) {
    return { status: "inconclusive", reason: "The baseline must prove an untracked back-calculation gain of zero." };
  }
  if (!Number.isFinite(input.candidate.trackingGain) || input.candidate.trackingGain <= 0) {
    return { status: "inconclusive", reason: "The candidate must prove a finite positive back-calculation gain." };
  }
  if (input.baseline.assessment.status !== "qualified" || input.candidate.assessment.status !== "qualified") {
    return { status: "inconclusive", reason: "Both runs need qualified saturation-return evidence." };
  }
  const baselineRecoverySeconds = input.baseline.assessment.recoverySeconds;
  const candidateRecoverySeconds = input.candidate.assessment.recoverySeconds;
  const baselineIntegralChange = input.baseline.assessment.integralChangeDuringSaturation;
  const candidateIntegralChange = input.candidate.assessment.integralChangeDuringSaturation;
  const recoveryImprovementSeconds = rounded(baselineRecoverySeconds - candidateRecoverySeconds);
  const integralReduction = rounded(Math.abs(baselineIntegralChange) - Math.abs(candidateIntegralChange));
  const recommended = recoveryImprovementSeconds > 0 && integralReduction > 0 && !input.candidate.oscillationDetected;
  return {
    status: "qualified",
    recommendation: recommended ? "recommend_tracking" : "do_not_recommend_tracking",
    baselineRecoverySeconds,
    candidateRecoverySeconds,
    baselineIntegralChange,
    candidateIntegralChange,
    recoveryImprovementSeconds,
    integralReduction,
    reason: recommended
      ? "The matched tracked record reduces retained-integral excess and leaves saturation sooner without recorded oscillation."
      : input.candidate.oscillationDetected
        ? "The matched tracked record has recorded oscillation; do not recommend it despite the recovery metrics."
        : "The matched tracked record does not improve both retained-integral excess and saturation recovery.",
  };
}

function matchingConditions(a: SaturationReturnConditions, b: SaturationReturnConditions): string | undefined {
  if (a.loopId !== b.loopId || a.loopRevision !== b.loopRevision) return "The candidate belongs to a different loop revision.";
  if (a.source !== b.source) return "The candidate uses a different execution source.";
  if (a.scenarioDigest !== b.scenarioDigest) return "The candidate does not use the same reviewed saturation-return scenario.";
  if (a.plantInitialConditionDigest !== b.plantInitialConditionDigest) return "The candidate did not start from the same plant initial condition.";
  if (a.controllerInitialConditionDigest !== b.controllerInitialConditionDigest) return "The candidate did not start from the same retained controller state.";
  if (a.samplePeriodMs !== b.samplePeriodMs || a.durationMs !== b.durationMs) return "The candidate does not use the same sample timing and duration.";
  if (a.outputMinimum !== b.outputMinimum || a.outputMaximum !== b.outputMaximum) return "The candidate does not use the same output limits.";
  return undefined;
}

function rounded(value: number): number {
  return Math.round(value * 1e9) / 1e9;
}
