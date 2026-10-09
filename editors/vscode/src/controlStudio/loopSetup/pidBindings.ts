import { PID_PARAMETERS, pidCandidates, type CompiledCatalog } from "./compiledCatalog";
import type { MeasuredSignal } from "./loopBindings";

export type PidParameterField = typeof PID_PARAMETERS[number];

/** Control.PID record fields as bound parameters, with units derived from the measurement and output units. */
export function pidParameters(record: string, pv: MeasuredSignal, mv: MeasuredSignal, fields: readonly PidParameterField[] = PID_PARAMETERS) {
  const span = mv.maximum - mv.minimum;
  // Gains are signed: their sign follows the controller action. The limit 1e6 applies to the magnitude of each gain.
  const bounds: Record<PidParameterField, [string, number, number]> = {
    ProportionalGain: [`${mv.unit}/${pv.unit}`, -1e6, 1e6],
    IntegralGain: [`${mv.unit}/(${pv.unit}*s)`, -1e6, 1e6],
    DerivativeGain: [`${mv.unit}*s/${pv.unit}`, -1e6, 1e6],
    ProportionalSetpointWeight: ["1", 0, 1],
    DerivativeSetpointWeight: ["1", 0, 1],
    // An offset added to the output, not an output value: it may shift the output by its span either way.
    Bias: [mv.unit, -span, span],
    OutputMinimum: [mv.unit, mv.minimum, mv.maximum],
    OutputMaximum: [mv.unit, mv.minimum, mv.maximum],
    // The Plant library's PID pair writes ±1e9, which Control.PID takes as no rate limit, so the bounds admit it.
    RateMinimum: [`${mv.unit}/s`, -1e9, 0],
    RateMaximum: [`${mv.unit}/s`, 0, 1e9],
    TrackingTime: ["s", 0.001, 1e4],
  };
  return fields.map(field => ({ symbol: `${record}.${field}`, iec_type: "LREAL" as const, unit: bounds[field][0],
    convention: `Control.PID ${field}`, minimum: bounds[field][1], maximum: bounds[field][2] }));
}

/** The seven retained Control.PID states, started from the model's operating point on the member's own channels. */
export function pidStates(instance: string, pv: MeasuredSignal, mv: MeasuredSignal, channel = 0) {
  const span = mv.maximum - mv.minimum, pvSpan = pv.maximum - pv.minimum;
  const state = (field: string, unit: string, minimum: number, maximum: number, initial: number | { source: "model-output" | "model-input"; index: number }) => ({
    symbol: `${instance}.StateValue.${field}`, iec_type: "LREAL" as const, unit, minimum, maximum, initial, postScan: "bounded" as const });
  return [
    state("Setpoint", pv.unit, pv.minimum, pv.maximum, { source: "model-output", index: channel }),
    state("SetpointRate", `${pv.unit}/s`, -1000 * pvSpan, 1000 * pvSpan, 0),
    state("ProcessValue", pv.unit, pv.minimum, pv.maximum, { source: "model-output", index: channel }),
    state("ProcessValueRate", `${pv.unit}/s`, -1000 * pvSpan, 1000 * pvSpan, 0),
    state("FeedForward", mv.unit, -span, span, 0),
    state("NominalOutput", mv.unit, mv.minimum - 10 * span, mv.maximum + 10 * span, { source: "model-input", index: channel }),
    state("SaturatedOutput", mv.unit, mv.minimum, mv.maximum, { source: "model-input", index: channel }),
  ];
}

/** A BOOL the program sets when its startup code has run; validation starts with it TRUE so that code keeps the seeded values. */
export function startupState(flag: string) {
  return { symbol: flag, iec_type: "BOOL" as const, unit: "1", minimum: 0, maximum: 1, initial: 1, postScan: "unchanged" as const };
}

/** `manualOutputFollowsOutput` when the code copies the PID's output into the chosen manual output, so Manual starts where Auto
 * left off: after an Auto scan that manual output is the output, not the value a transfer wrote (Specification 35 §17.1). */
export function manualOutputTracking(catalog: CompiledCatalog, instance: string, manualOutput: string): { manualOutputFollowsOutput?: true } {
  const candidate = pidCandidates(catalog).find(entry => entry.instance === instance);
  return candidate?.manualOutputFollowsOutput && candidate.manualOutput === manualOutput ? { manualOutputFollowsOutput: true } : {};
}
