export type ControllerStructure = "P" | "PI" | "PD" | "PID";
export type CandidateSource = "Manual" | "Imported" | "SIMC" | "AMIGO" | "Constrained comparison";

export interface ControlPidParameters {
  proportionalGain: number;
  integralGain: number;
  derivativeGain: number;
  proportionalSetpointWeight: number;
  derivativeSetpointWeight: number;
  bias: number;
  outputMinimum: number;
  outputMaximum: number;
  rateMinimum: number;
  rateMaximum: number;
  trackingTime: number;
  antiWindup: "BackCalculation" | "Combined";
}

export interface ControlPidState {
  setpoint: number;
  setpointRate: number;
  processValue: number;
  processValueRate: number;
  feedForward: number;
  nominalOutput: number;
  saturatedOutput: number;
}

export interface ControlPidInput {
  setpoint: number;
  processValue: number;
  feedForward: number;
  manualOutput: number;
  trackingOutput: number;
  mode: "Auto" | "Manual" | "Track";
  cycleTime: number;
}

export interface ControlPidCandidate {
  applied: false;
  source: CandidateSource;
  structure: ControllerStructure;
  parameters: ControlPidParameters;
}

export interface CandidateAssessment {
  candidate: ControlPidCandidate;
  passes: boolean;
  performanceScore: number;
}

export type CandidateRankingObjective = "fast" | "balanced" | "robust";
export interface QualifiedCandidateMetrics {
  iae: number;
  ise: number;
  itae: number;
  mvMovement: number;
  peakAbsoluteMv: number;
  maximumMvRate: number;
  noiseMvRms: number;
  worstCaseVariationIae: number;
  gainMarginDb: number;
  phaseMarginDegrees: number;
}
export interface QualifiedCandidateAssessment {
  id: string;
  method: "SIMC" | "AMIGO" | "Constrained comparison";
  candidate: ControlPidCandidate;
  feasible: boolean;
  metrics: QualifiedCandidateMetrics;
}
export interface RankedCandidateAssessment extends QualifiedCandidateAssessment { score: number }

export function candidateFromControlPid(
  parameters: ControlPidParameters,
  source: CandidateSource
): ControlPidCandidate {
  validateParameters(parameters);
  const hasIntegral = parameters.integralGain !== 0;
  const hasDerivative = parameters.derivativeGain !== 0;
  const structure: ControllerStructure = hasIntegral
    ? (hasDerivative ? "PID" : "PI")
    : (hasDerivative ? "PD" : "P");
  return { applied: false, source, structure, parameters: { ...parameters } };
}

export function updateControlPid(
  parameters: ControlPidParameters,
  state: ControlPidState,
  input: ControlPidInput
): { output: number; state: ControlPidState } {
  validateParameters(parameters);
  if (!Number.isFinite(input.cycleTime) || input.cycleTime <= 0) {
    throw new Error("Control.PID cycle time must be finite and positive.");
  }
  const lowerLimit = Math.max(
    parameters.outputMinimum,
    state.saturatedOutput + input.cycleTime * parameters.rateMinimum
  );
  const upperLimit = Math.min(
    parameters.outputMaximum,
    state.saturatedOutput + input.cycleTime * parameters.rateMaximum
  );
  const setpointRate = (input.setpoint - state.setpoint) / input.cycleTime;
  const processValueRate = (input.processValue - state.processValue) / input.cycleTime;
  let retainedNominal = input.mode === "Track" ? input.trackingOutput : state.nominalOutput;
  let nominalOutput: number;

  if (input.mode === "Manual") {
    nominalOutput = input.manualOutput;
  } else if (parameters.integralGain === 0) {
    nominalOutput = parameters.bias
      + parameters.proportionalGain * (input.setpoint - input.processValue)
      + parameters.derivativeGain
        * (parameters.derivativeSetpointWeight * setpointRate - processValueRate)
      + input.feedForward;
  } else {
    const proportionalIncrement = parameters.proportionalGain
      * (parameters.proportionalSetpointWeight * (input.setpoint - state.setpoint)
        - (input.processValue - state.processValue));
    const integralIncrement = parameters.integralGain * input.cycleTime
      * (input.setpoint - input.processValue);
    const derivativeIncrement = parameters.derivativeGain
      * (parameters.derivativeSetpointWeight * (setpointRate - state.setpointRate)
        - (processValueRate - state.processValueRate));
    const feedForwardIncrement = input.feedForward - state.feedForward;
    nominalOutput = retainedNominal + proportionalIncrement + integralIncrement
      + derivativeIncrement + feedForwardIncrement;
    const saturatedCandidate = clamp(nominalOutput, lowerLimit, upperLimit);
    if (parameters.antiWindup === "Combined") {
      const saturationError = nominalOutput - saturatedCandidate;
      if (integralIncrement * saturationError > 0) {
        const correction = Math.min(Math.abs(integralIncrement), Math.abs(saturationError));
        nominalOutput += saturationError > 0 ? -correction : correction;
      }
      nominalOutput -= Math.min(input.cycleTime / parameters.trackingTime, 1)
        * (nominalOutput - saturatedCandidate);
    } else {
      nominalOutput -= input.cycleTime / parameters.trackingTime
        * (nominalOutput - saturatedCandidate);
    }
  }
  retainedNominal = nominalOutput;
  const output = clamp(nominalOutput, lowerLimit, upperLimit);
  return {
    output,
    state: {
      setpoint: input.setpoint,
      setpointRate,
      processValue: input.processValue,
      processValueRate,
      feedForward: input.feedForward,
      nominalOutput: retainedNominal,
      saturatedOutput: output,
    },
  };
}

export function selectSimplestPassingCandidate(
  assessments: readonly CandidateAssessment[],
  materialBenefitFraction = 0.1
): ControlPidCandidate | undefined {
  if (!Number.isFinite(materialBenefitFraction) || materialBenefitFraction < 0) {
    throw new Error("Material benefit must be a finite non-negative fraction.");
  }
  const passing = assessments
    .filter(({ passes, performanceScore }) => passes && Number.isFinite(performanceScore) && performanceScore >= 0)
    .sort((left, right) => complexity(left.candidate.structure) - complexity(right.candidate.structure)
      || left.performanceScore - right.performanceScore);
  return passing[0]?.candidate;
}

/**
 * Ranks only candidates that already passed mandatory validation. Values are
 * normalized within this one comparison so engineering units cannot silently
 * dominate the selected objective.
 */
export function rankQualifiedCandidates(
  assessments: readonly QualifiedCandidateAssessment[],
  objective: CandidateRankingObjective
): { winner?: ControlPidCandidate; ranked: readonly RankedCandidateAssessment[] } {
  const feasible = assessments.filter((item) => item.feasible && finiteMetrics(item.metrics));
  const minima = {
    tracking: minimumPositive(feasible.map((item) => item.metrics.iae + item.metrics.ise + item.metrics.itae)),
    actuator: minimumPositive(feasible.map((item) => item.metrics.mvMovement + item.metrics.peakAbsoluteMv
      + item.metrics.maximumMvRate + item.metrics.noiseMvRms)),
    variation: minimumPositive(feasible.map((item) => item.metrics.worstCaseVariationIae)),
  };
  const weights = objective === "fast" ? [3, 1, 1] : objective === "robust" ? [1, 2, 3] : [2, 2, 2];
  const scored = feasible.map((item): RankedCandidateAssessment => {
    const tracking = (item.metrics.iae + item.metrics.ise + item.metrics.itae) / minima.tracking;
    const actuator = (item.metrics.mvMovement + item.metrics.peakAbsoluteMv
      + item.metrics.maximumMvRate + item.metrics.noiseMvRms) / minima.actuator;
    const robustness = marginPenalty(item.metrics.gainMarginDb, 6)
      + marginPenalty(item.metrics.phaseMarginDegrees, 30)
      + item.metrics.worstCaseVariationIae / minima.variation;
    return { ...item, score: tracking * weights[0] + actuator * weights[1] + robustness * weights[2] };
  });
  const scoredIds = new Set(scored.map((item) => item.id));
  const rejected = assessments.filter((item) => !scoredIds.has(item.id))
    .map((item): RankedCandidateAssessment => ({ ...item, score: Number.POSITIVE_INFINITY }));
  const simplestComplexity = Math.min(...scored.map((item) => complexity(item.candidate.structure)));
  const selected = scored.filter((item) => complexity(item.candidate.structure) === simplestComplexity)
    .sort((left, right) => left.score - right.score ||
    methodOrder(left.method) - methodOrder(right.method) || left.id.localeCompare(right.id));
  const winner = selected[0];
  const ranked = [...scored].sort((left, right) =>
    complexity(left.candidate.structure) - complexity(right.candidate.structure)
    || left.score - right.score
    || methodOrder(left.method) - methodOrder(right.method)
    || left.id.localeCompare(right.id)).concat(rejected);
  return { winner: winner?.candidate, ranked };
}

function validateParameters(parameters: ControlPidParameters): void {
  const numeric = [
    parameters.proportionalGain, parameters.integralGain, parameters.derivativeGain,
    parameters.proportionalSetpointWeight, parameters.derivativeSetpointWeight,
    parameters.bias, parameters.outputMinimum, parameters.outputMaximum,
    parameters.rateMinimum, parameters.rateMaximum, parameters.trackingTime,
  ];
  if (!numeric.every(Number.isFinite)) throw new Error("Control.PID settings must be finite.");
  if (parameters.outputMinimum >= parameters.outputMaximum) {
    throw new Error("Control.PID output minimum must be below output maximum.");
  }
  if (parameters.rateMinimum > parameters.rateMaximum) {
    throw new Error("Control.PID rate minimum must not exceed rate maximum.");
  }
  if (parameters.trackingTime <= 0) throw new Error("Control.PID tracking time must be positive.");
}

function complexity(structure: ControllerStructure): number {
  return structure === "P" ? 0 : structure === "PID" ? 2 : 1;
}

function finiteMetrics(metrics: QualifiedCandidateMetrics): boolean {
  return Object.values(metrics).every((value) => Number.isFinite(value) || value === Number.POSITIVE_INFINITY)
    && metrics.gainMarginDb >= 0 && metrics.phaseMarginDegrees >= 0;
}

function minimumPositive(values: readonly number[]): number {
  const finite = values.filter((value) => Number.isFinite(value) && value > 0);
  return finite.length ? Math.min(...finite) : 1;
}

function marginPenalty(value: number, required: number): number {
  return value === Number.POSITIVE_INFINITY ? 0 : required / Math.max(value, Number.EPSILON);
}

function methodOrder(method: QualifiedCandidateAssessment["method"]): number {
  return method === "SIMC" ? 0 : method === "AMIGO" ? 1 : 2;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}
