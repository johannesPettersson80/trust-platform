export interface BackCalculationStepInput {
  readonly retainedIntegral: number;
  readonly integralGain: number;
  readonly error: number;
  readonly cycleSeconds: number;
  readonly proportionalContribution: number;
  readonly outputMinimum: number;
  readonly outputMaximum: number;
  readonly trackingTimeSeconds: number;
}

export interface BackCalculationStep {
  readonly trialIntegral: number;
  readonly requestedOutput: number;
  readonly appliedOutput: number;
  readonly trackingCorrection: number;
  readonly nextIntegral: number;
}

/** One positional PI back-calculation step; the unlimited request remains observable. */
export function calculateBackCalculationStep(input: BackCalculationStepInput): BackCalculationStep {
  if (!Number.isFinite(input.trackingTimeSeconds) || input.trackingTimeSeconds <= 0) {
    throw new Error("Anti-windup tracking time must be positive and finite.");
  }
  if (!Number.isFinite(input.outputMinimum) || !Number.isFinite(input.outputMaximum) ||
      input.outputMinimum >= input.outputMaximum) {
    throw new Error("Anti-windup output limits must be finite and ordered.");
  }
  const numeric = [input.retainedIntegral, input.integralGain, input.error, input.cycleSeconds,
    input.proportionalContribution];
  if (!numeric.every(Number.isFinite)) throw new Error("Anti-windup inputs must be finite.");
  if (input.cycleSeconds <= 0) throw new Error("Anti-windup cycle time must be positive.");
  const trialIntegral = input.retainedIntegral + input.integralGain * input.error * input.cycleSeconds;
  const requestedOutput = input.proportionalContribution + trialIntegral;
  const appliedOutput = Math.max(input.outputMinimum, Math.min(input.outputMaximum, requestedOutput));
  const trackingCorrection = input.cycleSeconds / input.trackingTimeSeconds * (appliedOutput - requestedOutput);
  return {
    trialIntegral: rounded(trialIntegral),
    requestedOutput: rounded(requestedOutput),
    appliedOutput: rounded(appliedOutput),
    trackingCorrection: rounded(trackingCorrection),
    nextIntegral: rounded(trialIntegral + trackingCorrection),
  };
}

function rounded(value: number): number {
  return Math.round(value * 1e12) / 1e12;
}
