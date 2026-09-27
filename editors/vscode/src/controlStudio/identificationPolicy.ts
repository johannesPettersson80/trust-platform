import type { ControlLoopDefinition } from "./loopDefinition";
export interface IdentificationPolicy {
  maximumNormalizedError?: number;
  maximumOrder?: number;
  openLoopPermitted: boolean;
  sustainedHoldPermitted: boolean;
  information: "basic" | "frequency" | "multiple-modes";
}
export function validIdentificationPolicy(value: unknown): value is IdentificationPolicy {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const policy = value as Record<string, unknown>;
  return (policy.maximumNormalizedError === undefined || typeof policy.maximumNormalizedError === "number" && Number.isFinite(policy.maximumNormalizedError) && policy.maximumNormalizedError > 0 && policy.maximumNormalizedError < 1)
    && (policy.maximumOrder === undefined || typeof policy.maximumOrder === "number" && Number.isInteger(policy.maximumOrder) && policy.maximumOrder >= 1 && policy.maximumOrder <= 8)
    && typeof policy.openLoopPermitted === "boolean" && typeof policy.sustainedHoldPermitted === "boolean"
    && ["basic", "frequency", "multiple-modes"].includes(String(policy.information));
}
/** Explicit experiment choices are retained; automatic fills only an unchosen method. */
export function identificationLoop(loop: ControlLoopDefinition): ControlLoopDefinition {
  const requested = loop.experiment?.kind;
  const policy = loop.identificationPolicy;
  if (requested === "load_step") throw new Error("Load-step identification requires an explicit disturbance model channel; choose a supported acquisition method instead of substituting an MV step.");
  if (requested && requested !== "observation") {
    if (policy?.openLoopPermitted === false && ["mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(requested)) {
      throw new Error("The pinned identification method conflicts with the declared closed-loop-only acquisition policy.");
    }
    if (requested === "mv_step" && policy?.sustainedHoldPermitted === false) throw new Error("The pinned sustained step conflicts with the declared hold restriction.");
    return loop;
  }
  const multiple = (loop.channels?.inputs.length ?? 1) > 1 || (loop.channels?.outputs.length ?? 1) > 1;
  const switching = loop.channels?.inputs.some(input => input.actuator.kind === "on-off") || !!loop.controller.onOff;
  const kind = policy?.openLoopPermitted === false || !loop.controller.manualOutputSignal ? "setpoint_step"
    : switching ? policy?.sustainedHoldPermitted === false ? "mv_pulse" : "mv_step"
    : multiple || policy?.information === "frequency" ? "prbs" : policy?.information === "multiple-modes" ? "multisine"
    : policy?.sustainedHoldPermitted === false ? "mv_pulse" : "mv_step";
  return { ...loop, experiment: { execution: "control-studio", amplitude: 0, durationSeconds: 10, seed: 1,
    ...loop.experiment, kind } };
}
