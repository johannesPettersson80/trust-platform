import type { ControlExperimentKind } from "./experiment";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { AvailableLoopSample, LoopMonitorSample } from "./loopMonitor";

export interface IdentificationPolicy {
  maximumNormalizedError?: number;
  maximumOrder?: number;
  openLoopPermitted: boolean;
  sustainedHoldPermitted: boolean;
  information: "basic" | "frequency" | "multiple-modes";
  /** Declares the process integrating, so a held step is fitted as an integrator without the ambiguity check (B1.4, N2.3a). */
  process?: "integrating";
}
export function validIdentificationPolicy(value: unknown): value is IdentificationPolicy {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const policy = value as Record<string, unknown>;
  return (policy.maximumNormalizedError === undefined || typeof policy.maximumNormalizedError === "number" && Number.isFinite(policy.maximumNormalizedError) && policy.maximumNormalizedError > 0 && policy.maximumNormalizedError < 1)
    && (policy.maximumOrder === undefined || typeof policy.maximumOrder === "number" && Number.isInteger(policy.maximumOrder) && policy.maximumOrder >= 1 && policy.maximumOrder <= 8)
    && typeof policy.openLoopPermitted === "boolean" && typeof policy.sustainedHoldPermitted === "boolean"
    && ["basic", "frequency", "multiple-modes"].includes(String(policy.information))
    && (policy.process === undefined || policy.process === "integrating");
}

/** The method one identification runs: an explicit choice as declared, or what Automatic chooses (Specification 35, B3.3). */
export interface IdentificationMethod {
  readonly kind: ControlExperimentKind;
  /** The experiment writes the plant input in Manual, so the loop changes to Manual and returns to its mode afterwards (A6.2). */
  readonly manual: boolean;
  readonly automatic: boolean;
}

/** An explicit method is kept as declared. Automatic is decided from the live controller mode and the declared policy. */
export function identificationMethod(loop: ControlLoopDefinition, sample: LoopMonitorSample): IdentificationMethod {
  const requested = loop.experiment?.kind;
  if (requested && requested !== "observation") {
    validateExplicitMethod(loop, requested);
    return { kind: requested, manual: requested !== "setpoint_step", automatic: false };
  }
  return automaticMethod(loop, sample.availability === "available" ? sample.controller.mode : "Unavailable");
}

/** B3.3: a setpoint step in Auto for a loop with a declared setpoint; a plant-input step in Manual otherwise. */
export function automaticMethod(loop: ControlLoopDefinition, mode: "Auto" | "Manual" | "Unavailable"): IdentificationMethod {
  if (loop.topology === "open-loop") return plantInputMethod(loop);
  if (mode === "Unavailable") {
    throw new Error("Automatic identification cannot choose a method: the loop's Auto/Manual mode is unavailable. Reconnect the runtime, or choose a method.");
  }
  const openLoopOk = loop.identificationPolicy?.openLoopPermitted !== false && !!loop.controller.manualOutputSignal;
  const singleInput = (loop.channels?.inputs.length ?? 1) <= 1;
  if (loop.signals.setpoint && singleInput && mode === "Auto") {
    return { kind: "setpoint_step", manual: false, automatic: true };
  }
  if (!openLoopOk && loop.signals.setpoint && singleInput) {
    // Automatic never puts a loop in Manual into Auto: the controller may not hold the loop yet.
    throw new Error("Automatic identification has no safe method: the loop is in Manual, and open-loop tests are not permitted by the identification policy or the missing Manual command. Put the loop in Auto with settings that hold it and identify again, or choose a method.");
  }
  if (!openLoopOk) {
    throw new Error(loop.signals.setpoint
      ? "Automatic identification has no safe method here: several plant inputs need open-loop tests, which the identification policy or the missing Manual command does not permit. Choose a method."
      : "Automatic identification has no safe method: the loop has no declared setpoint binding, and open-loop tests are not permitted by the identification policy or the missing Manual command. Declare the setpoint binding, or choose a method.");
  }
  return plantInputMethod(loop);
}

/** A plant-input step in Manual: a pulse when sustained holds are not permitted, PRBS when the policy asks for frequency content. */
function plantInputMethod(loop: ControlLoopDefinition): IdentificationMethod {
  const policy = loop.identificationPolicy;
  const kind: ControlExperimentKind = policy?.information === "frequency" ? "prbs"
    : policy?.sustainedHoldPermitted === false ? "mv_pulse" : "mv_step";
  return { kind, manual: true, automatic: true };
}

function validateExplicitMethod(loop: ControlLoopDefinition, requested: ControlExperimentKind): void {
  const policy = loop.identificationPolicy;
  if (requested === "load_step") throw new Error("Load-step identification requires an explicit disturbance model channel; choose a supported acquisition method instead of substituting an MV step.");
  if (policy?.openLoopPermitted === false && ["mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(requested)) {
    throw new Error("The pinned identification method conflicts with the declared closed-loop-only acquisition policy.");
  }
  if (requested === "mv_step" && policy?.sustainedHoldPermitted === false) throw new Error("The pinned sustained step conflicts with the declared hold restriction.");
}

/** Whether the input is an on/off actuator, which steps between its two levels (B3.3). */
export function onOffActuator(loop: ControlLoopDefinition): boolean {
  return loop.controller.onOff !== undefined || loop.channels?.inputs.some(input => input.actuator.kind === "on-off") === true;
}

export interface IdentificationAmplitudeInput {
  /** The loop of the input being stepped: its limits, signals and declared amplitude. */
  readonly loop: ControlLoopDefinition;
  /** The input's fresh sample, read at the step. */
  readonly sample: LoopMonitorSample;
  readonly kind: ControlExperimentKind;
  readonly automatic: boolean;
  /** The noise σ measured on the steady baseline. */
  readonly noise: number;
  /** The independent model check uses the opposite excitation (B5.2). */
  readonly validation: boolean;
  /** The amplitude an explicit method uses when none is declared. */
  readonly fallbackAmplitude: number;
  readonly onOff: boolean;
}

/** The signed excitation of one experiment: the declared amplitude, or Automatic's size and direction (B3.3, B5.2). */
export function identificationAmplitude(input: IdentificationAmplitudeInput): number {
  const sample = input.sample;
  if (sample.availability !== "available") throw new Error("Fresh synchronized SP, PV, and MV are required.");
  const reduced = input.validation && input.kind !== "setpoint_step" ? 0.75 : 1;
  if (!input.automatic) {
    const configured = input.loop.experiment?.amplitude || input.fallbackAmplitude;
    return (input.validation ? -1 : 1) * (Math.sign(configured) || 1) * Math.abs(configured) * reduced;
  }
  // An on/off actuator has two levels: its check steps to the other level as identification did, since there is no opposite one.
  const direction = stepDirection(input.loop, sample, input.kind, input.onOff);
  if (input.validation && !input.onOff) {
    // B5.2: the model check uses a different size or sign. It steps the other way when there is room there; otherwise it keeps
    // the direction, and its reduced size is the difference.
    try { return automaticAmplitude(input, sample, -direction, reduced); }
    catch { /* No headroom the other way. */ }
  }
  return automaticAmplitude(input, sample, direction, reduced);
}

/** Automatic steps toward the larger headroom; an on/off actuator steps toward its other level. */
function stepDirection(loop: ControlLoopDefinition, sample: AvailableLoopSample, kind: ControlExperimentKind, onOff: boolean): 1 | -1 {
  if (kind === "prbs") return 1;
  if (kind === "setpoint_step") {
    const { minimum, maximum } = loop.signals.processVariable.range;
    const value = sample.processVariable.value;
    return maximum - value >= value - minimum ? 1 : -1;
  }
  const { outputMinimum: minimum, outputMaximum: maximum } = loop.limits;
  const value = sample.manipulatedVariable.value;
  if (onOff) return value <= (minimum + maximum) / 2 ? 1 : -1;
  return maximum - value >= value - minimum ? 1 : -1;
}

/** B3.3 sizes. A setpoint step is the larger of 20σ and 2 % of the output's range, capped at 10 % of it and by the headroom to
 * the permitted output range. A plant-input step is 10 % of the input range, capped by the headroom to the input limits. */
function automaticAmplitude(input: IdentificationAmplitudeInput, sample: AvailableLoopSample, direction: number, reduced: number): number {
  const loop = input.loop;
  let amplitude: number;
  if (input.kind === "setpoint_step") {
    const { minimum, maximum } = loop.signals.processVariable.range;
    const span = maximum - minimum;
    const headroom = direction > 0 ? maximum - sample.processVariable.value : sample.processVariable.value - minimum;
    amplitude = direction * Math.max(0, Math.min(Math.max(20 * input.noise, 0.02 * span), 0.1 * span, headroom));
  } else {
    const { outputMinimum: minimum, outputMaximum: maximum } = loop.limits;
    const value = sample.manipulatedVariable.value;
    if (input.kind === "prbs") {
      amplitude = Math.max(0, Math.min(0.1 * (maximum - minimum), value - minimum, maximum - value));
    } else if (input.onOff) {
      amplitude = direction > 0 ? maximum - value : minimum - value;
    } else {
      const headroom = direction > 0 ? maximum - value : value - minimum;
      amplitude = direction * Math.max(0, Math.min(0.1 * (maximum - minimum) * reduced, headroom));
    }
  }
  if (!Number.isFinite(amplitude) || amplitude === 0) {
    const signal = input.kind === "setpoint_step" ? loop.signals.processVariable : loop.signals.manipulatedVariable;
    const current = input.kind === "setpoint_step" ? sample.processVariable : sample.manipulatedVariable;
    throw new Error(`Automatic identification has no headroom to step ${signal.symbol} from ${current.value} ${current.unit}. Open Limits… to widen its permitted range, or choose another method.`);
  }
  return amplitude;
}
