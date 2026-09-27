import { candidateFromControlPid, type ControlPidCandidate } from "./controllerCandidate";
import type { ControlLoopDefinition } from "./loopDefinition";

export function generatedPiCandidate(
  kp: number,
  tiSeconds: number,
  source: "SIMC" | "AMIGO",
  loop?: ControlLoopDefinition
): ControlPidCandidate {
  const rateUp = loop?.limits.rateUpPerS ?? 1_000_000;
  const rateDown = loop?.limits.rateDownPerS ?? 1_000_000;
  return candidateFromControlPid({
    proportionalGain: kp,
    integralGain: kp / tiSeconds,
    derivativeGain: 0,
    proportionalSetpointWeight: 1,
    derivativeSetpointWeight: 0,
    bias: 0,
    outputMinimum: loop?.limits.outputMinimum ?? 0,
    outputMaximum: loop?.limits.outputMaximum ?? 100,
    rateMinimum: -rateDown,
    rateMaximum: rateUp,
    trackingTime: Math.max(tiSeconds, (loop?.controller.taskPeriodMs ?? 10) / 1_000),
    antiWindup: "BackCalculation",
  }, source);
}


export function candidateFromRuntimeParameters(value: Record<string, unknown>): ControlPidCandidate {
  const number = (name: string): number => {
    const result = Number(value[name]);
    if (!Number.isFinite(result)) throw new Error(`Runtime controller parameter ${name} is unavailable.`);
    return result;
  };
  return candidateFromControlPid({
    proportionalGain: number("ProportionalGain"),
    integralGain: number("IntegralGain"),
    derivativeGain: number("DerivativeGain"),
    proportionalSetpointWeight: number("ProportionalSetpointWeight"),
    derivativeSetpointWeight: number("DerivativeSetpointWeight"),
    bias: number("Bias"),
    outputMinimum: number("OutputMinimum"),
    outputMaximum: number("OutputMaximum"),
    rateMinimum: number("RateMinimum"),
    rateMaximum: number("RateMaximum"),
    trackingTime: number("TrackingTime"),
    antiWindup: value.AntiWindup === "Combined" ? "Combined" : "BackCalculation",
  }, "Imported");
}
