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

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}
