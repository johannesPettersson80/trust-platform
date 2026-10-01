import { isControllerFamily } from "./controllerFamilies";
import { controllerAdapter } from "./controllerAdapters";
import type { ControlLoopDefinition } from "./loopDefinition";
import { decodePidLaw } from "./controllerLaw";
import { proportionalParameter } from "./engineerVerificationPlan";
import type { ControllerFamily, ControllerGains } from "./controllerAdapters";

export interface ControllerSettingsPins {
  loopRevision: string;
  family: ControllerFamily;
  values: Readonly<Record<string, number>>;
}
export function decodeControllerSettingsPins(value: unknown): ControllerSettingsPins | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const pin = value as Record<string, unknown>;
  if (typeof pin.loopRevision !== "string" || !pin.loopRevision || !isControllerFamily(pin.family)
    || !pin.values || typeof pin.values !== "object" || Array.isArray(pin.values)
    || Object.entries(pin.values).length > 64 || !Object.entries(pin.values).every(([symbol, value]) => symbol.length > 0 && typeof value === "number" && Number.isFinite(value))) return undefined;
  return structuredClone(pin) as unknown as ControllerSettingsPins;
}
export function pinnedControllerGains(loop: ControlLoopDefinition, family: ControllerFamily, gains: ControllerGains,
  pin: ControllerSettingsPins | undefined): ControllerGains {
  if (!pin) return gains;
  if (pin.loopRevision !== loop.revision || pin.family !== family) throw new Error("Pinned settings belong to a different loop or controller; explicitly clear them before changing the design.");
  if (loop.controller.stateFeedback || loop.controller.onOff || loop.controller.project) {
    const combined = { ...gains, parameters: { ...gains.parameters, ...pin.values } };
    controllerAdapter(loop).materialize(combined, {});
    return combined;
  }
  const symbols = controllerGainSymbols(loop);
  const result = { ...gains };
  for (const [symbol, value] of Object.entries(pin.values)) {
    const key = symbols[symbol];
    const parameter = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol);
    if (!key || !parameter || value < parameter.minimum || value > parameter.maximum || parameter.iec_type === "REAL" && Math.fround(value) !== value) {
      throw new Error(`Pinned setting ${symbol} does not match the bound ST parameter and representable bounds.`);
    }
    result[key] = value;
  }
  if ((family === "P" || family === "PD") && result.integralGain || (family === "P" || family === "PI") && result.derivativeGain) {
    throw new Error("Pinned integral/derivative settings conflict with the accepted controller family.");
  }
  return result;
}

/** The PID or proportional ST parameters that hold each controller setting. */
function controllerGainSymbols(loop: ControlLoopDefinition): Record<string, Exclude<keyof ControllerGains, "parameters">> {
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  return pid ? {
    [pid.proportionalGainSymbol]: "proportionalGain", [pid.integralGainSymbol]: "integralGain", [pid.derivativeGainSymbol]: "derivativeGain",
    ...(pid.antiWindup ? { [pid.antiWindup.trackingGainSymbol]: "backCalculationGain" as const } : {}),
    ...(pid.proportionalSetpointWeightSymbol ? { [pid.proportionalSetpointWeightSymbol]: "proportionalSetpointWeight" as const } : {}),
    ...(pid.derivativeSetpointWeightSymbol ? { [pid.derivativeSetpointWeightSymbol]: "derivativeSetpointWeight" as const } : {}),
    ...(pid.measurementFilterTimeSymbol ? { [pid.measurementFilterTimeSymbol]: "measurementFilterSeconds" as const } : {}),
  } : { [proportionalParameter(loop).symbol]: "proportionalGain" };
}

/** The running controller's settings, read from its bound ST parameter values. */
export function currentControllerGains(loop: ControlLoopDefinition, values: Readonly<Record<string, number>>): ControllerGains | undefined {
  if (loop.controller.stateFeedback || loop.controller.onOff || loop.controller.project) return undefined;
  const gains: ControllerGains = {};
  for (const [symbol, key] of Object.entries(controllerGainSymbols(loop))) {
    const value = values[symbol];
    if (typeof value === "number" && Number.isFinite(value)) gains[key] = value;
  }
  return typeof gains.proportionalGain === "number" ? gains : undefined;
}
