import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { decodeControllerInitialization, type ControllerInitialization } from "./controllerInitialization";

export interface OnOffControllerBinding {
  hysteresisSymbol: string;
  minimumOnTimeSymbol: string;
  minimumOffTimeSymbol: string;
  bandConvention: "half-band" | "full-band";
  lowOutput: number;
  highOutput: number;
  noiseBand?: number;
  initialization: ControllerInitialization;
}
export interface OnOffControllerDesign {
  status: "provisional";
  candidate: { applied: false; structure: "ON/OFF"; parameters: Readonly<Record<string, number>>; proportionalGain?: never };
  selectedModel: { revision: string; provenance: string };
  actualSTVerify: true;
  scope: string;
}
export function decodeOnOffController(value: unknown, binding?: ParameterBinding): OnOffControllerBinding | undefined {
  if (!record(value) || !binding || !["half-band", "full-band"].includes(String(value.bandConvention))
    || !finite(value.lowOutput) || !finite(value.highOutput) || value.lowOutput >= value.highOutput
    || value.noiseBand !== undefined && (!finite(value.noiseBand) || value.noiseBand < 0)
    || !decodeControllerInitialization(value.initialization)) return undefined;
  const symbols = [value.hysteresisSymbol, value.minimumOnTimeSymbol, value.minimumOffTimeSymbol];
  if (new Set(symbols).size !== 3 || !symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))) return undefined;
  if (![value.minimumOnTimeSymbol, value.minimumOffTimeSymbol].every(symbol => binding.parameters.find(parameter => parameter.symbol === symbol)?.unit === "s")) return undefined;
  return structuredClone(value) as unknown as OnOffControllerBinding;
}
export function designOnOffController(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements): OnOffControllerDesign {
  const definition = decodeOnOffController(loop.controller.onOff, loop.parameterBinding);
  if (!definition || !loop.signals.setpoint) throw new Error("Declare the switching controller's hysteresis, dwell, initialization and reference bindings.");
  if (definition.lowOutput < loop.limits.outputMinimum || definition.highOutput > loop.limits.outputMaximum) throw new Error("Switching levels exceed the available actuator limits.");
  const hysteresis = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === definition.hysteresisSymbol)!;
  if (hysteresis.unit !== loop.signals.processVariable.unit) throw new Error("Switching hysteresis must use the measured process unit.");
  const actuator = loop.channels?.inputs[0]?.actuator;
  const halfBand = Math.max(definition.noiseBand ?? 0, task.settlingBand);
  if (halfBand >= task.maximumError) throw new Error("The declared noise/process band leaves no switching design inside the permitted error band.");
  const period = loop.controller.taskPeriodMs / 1000;
  const parameter = (symbol: string, proposed: number) => {
    const binding = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === symbol)!;
    const value = binding.iec_type === "REAL" ? Math.fround(proposed) : proposed;
    if (!Number.isFinite(value) || value < binding.minimum || value > binding.maximum) throw new Error(`Switching setting ${symbol} is outside its representable bounds.`);
    return value;
  };
  const dwell = (symbol: string, minimum: number) => {
    const binding = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === symbol)!;
    const seconds = Math.ceil(Math.max(period, minimum, binding.minimum) / period) * period;
    const value = parameter(symbol, seconds);
    if (value < minimum) throw new Error(`Rounded dwell for ${symbol} is below the equipment minimum.`);
    return value;
  };
  const parameters = {
    [definition.hysteresisSymbol]: parameter(definition.hysteresisSymbol, halfBand * (definition.bandConvention === "full-band" ? 2 : 1)),
    [definition.minimumOnTimeSymbol]: dwell(definition.minimumOnTimeSymbol, actuator?.minimumOnSeconds ?? 0),
    [definition.minimumOffTimeSymbol]: dwell(definition.minimumOffTimeSymbol, actuator?.minimumOffSeconds ?? 0),
  };
  return { status: "provisional", candidate: { applied: false, structure: "ON/OFF", parameters },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: "Switching-band and equipment-dwell candidate in the existing ST conventions. Actual-ST target, switching and recovery requirements determine qualification." };
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
