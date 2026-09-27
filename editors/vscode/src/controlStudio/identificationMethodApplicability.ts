import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";

export type IdentificationMethod = "configured" | "automatic" | "mv_step" | "mv_pulse" | "setpoint_step" | "relay" | "prbs" | "multisine";

export interface IdentificationMethodApplicability {
  available: boolean;
  reason?: string;
}

/** Pure UI capability boundary for the methods already owned by identification. */
export function identificationMethodApplicability(loop: ControlLoopDefinition, method: IdentificationMethod): IdentificationMethodApplicability {
  if (method === "configured") {
    const configured = loop.experiment?.kind;
    const supported: IdentificationMethod[] = ["automatic", "mv_step", "mv_pulse", "setpoint_step", "relay", "prbs", "multisine"];
    return identificationMethodApplicability(loop, supported.includes(configured as IdentificationMethod) ? configured as IdentificationMethod : "automatic");
  }
  if (method === "automatic") return { available: true };
  const inputs = loop.channels?.inputs ?? [];
  const outputs = loop.channels?.outputs ?? [];
  const multichannel = inputs.length > 1 || outputs.length > 1;
  const actuators = inputs.map(input => input.actuator);
  const binary = actuators.length > 0 && actuators.every(actuator => actuator.kind === "on-off");
  if (method === "relay") {
    if (!loop.experiment?.relayDirection) return { available: false, reason: "Relay identification needs an explicitly declared process direction." };
    if (multichannel) return { available: false, reason: "Multichannel relay identification needs explicit feedback pairing." };
    if (!loop.controller.manualOutputSignal || !loop.parameterBinding?.mode) return { available: false, reason: "Relay identification needs Manual command and mode ownership." };
  }
  if ((method === "prbs" || method === "multisine") && (Boolean(loop.controller.onOff) || actuators.some(actuator => actuator.kind === "on-off"))) {
    return { available: false, reason: "Frequency excitation needs intermediate output levels; choose a binary step or pulse." };
  }
  if ((method === "mv_step" || method === "mv_pulse") && multichannel && !binary) {
    return { available: false, reason: "Continuous multichannel step or pulse identification is unavailable; choose PRBS or multisine." };
  }
  if (method === "setpoint_step") {
    if (!loop.signals.setpoint) return { available: false, reason: "Setpoint-step identification needs the configured reference signal." };
    if (multichannel) return { available: false, reason: "Multichannel setpoint-step identification is unavailable; choose PRBS or multisine." };
  }
  if ((method === "mv_step" || method === "mv_pulse" || method === "prbs" || method === "multisine")
    && (!loop.controller.manualOutputSignal || !loop.parameterBinding?.mode)) {
    return { available: false, reason: "Open-loop identification needs the declared Manual command and mode bindings." };
  }
  if (method === "setpoint_step" && !loop.parameterBinding?.mode) {
    return { available: false, reason: "Closed-loop setpoint identification needs the declared controller mode binding." };
  }
  if (loop.channels) {
    for (const input of inputs) loopSignal(loop, input.signalId);
    for (const output of outputs) loopSignal(loop, output.signalId);
  }
  return { available: true };
}
