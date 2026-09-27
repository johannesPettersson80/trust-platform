import { linearTransferModel } from "./linearTransferModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import { decodeDeclaredAnalysis, type DeclaredControlAnalysis } from "./declaredAnalysis";
import { type GeneralModel } from "./generalModel";

export interface ProportionalControllerLaw { kind: "proportional"; gainSymbol: string }
export interface PidControllerLaw {
  kind: "pid";
  parameterRecordSymbol: string;
  parameterRevisionSymbol: string;
  proportionalGainSymbol: string;
  integralGainSymbol: string;
  derivativeGainSymbol: string;
  antiWindup?: {
    method: "back-calculation";
    trackingGainSymbol: string;
  };
}
export type ControllerLaw = ProportionalControllerLaw | PidControllerLaw;

/** The engineer declares the equation; a symbol name alone cannot establish controller semantics. */
export function decodeProportionalLaw(value: unknown, binding: ParameterBinding | undefined): ProportionalControllerLaw | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const law = value as Record<string, unknown>;
  if (law.kind !== "proportional" || typeof law.gainSymbol !== "string"
      || !Object.keys(law).every(key => key === "kind" || key === "gainSymbol")) return undefined;
  const gain = binding?.parameters.find(parameter => parameter.symbol === law.gainSymbol);
  if (!gain || !["REAL", "LREAL"].includes(gain.iec_type)) return undefined;
  return { kind: "proportional", gainSymbol: law.gainSymbol };
}

/** A PID structure is eligible only through a named record owner and three distinct compiled gains. */
export function decodePidLaw(value: unknown, binding: ParameterBinding | undefined): PidControllerLaw | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const law = value as Record<string, unknown>;
  const expected = ["kind", "parameterRecordSymbol", "parameterRevisionSymbol", "proportionalGainSymbol", "integralGainSymbol", "derivativeGainSymbol"];
  const antiWindup = decodePidAntiWindup(law.antiWindup);
  if (law.kind !== "pid" || !Object.keys(law).every(key => expected.includes(key) || key === "antiWindup") ||
      !expected.every(key => key in law) || (law.antiWindup !== undefined && !antiWindup) ||
      !expected.slice(1).every(key => label(law[key]))) return undefined;
  const symbols = [law.proportionalGainSymbol, law.integralGainSymbol, law.derivativeGainSymbol,
    ...(antiWindup ? [antiWindup.trackingGainSymbol] : [])] as string[];
  if (new Set(symbols).size !== symbols.length || !symbols.every(symbol => {
    const parameter = binding?.parameters.find(value => value.symbol === symbol);
    return parameter !== undefined && (parameter.iec_type === "REAL" || parameter.iec_type === "LREAL");
  })) return undefined;
  if (antiWindup && binding?.parameters.find(parameter => parameter.symbol === antiWindup.trackingGainSymbol)?.unit !== "1/s") return undefined;
  return {
    kind: "pid", parameterRecordSymbol: law.parameterRecordSymbol as string,
    parameterRevisionSymbol: law.parameterRevisionSymbol as string,
    proportionalGainSymbol: law.proportionalGainSymbol as string,
    integralGainSymbol: law.integralGainSymbol as string,
    derivativeGainSymbol: law.derivativeGainSymbol as string,
    ...(antiWindup ? { antiWindup } : {}),
  };
}

export function decodeControllerLaw(value: unknown, binding: ParameterBinding | undefined): ControllerLaw | undefined {
  return decodeProportionalLaw(value, binding) ?? decodePidLaw(value, binding);
}

/** Maps the parallel PID gains into the exact declared ST parameter record. */
export function materializePidGainRecord(loop: ControlLoopDefinition, gains: {
  proportionalGain: number;
  integralGain: number;
  derivativeGain: number;
  backCalculationGain?: number;
}, observed: Readonly<Record<string, number>>): Record<string, number> {
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law || !loop.parameterBinding) throw new Error("A typed PID record adapter is required before materializing controller gains.");
  const entries: [string, number][] = [
    [law.proportionalGainSymbol, gains.proportionalGain],
    [law.integralGainSymbol, gains.integralGain],
    [law.derivativeGainSymbol, gains.derivativeGain],
  ];
  if (gains.backCalculationGain !== undefined) {
    if (!law.antiWindup) throw new Error("A typed PID anti-windup adapter is required before materializing a back-calculation gain.");
    entries.push([law.antiWindup.trackingGainSymbol, gains.backCalculationGain]);
  }
  const values = { ...observed };
  for (const [symbol, value] of entries) {
    const parameter = loop.parameterBinding.parameters.find(candidate => candidate.symbol === symbol);
    if (!parameter || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) {
      throw new Error(`PID gain for ${symbol} is outside its declared binding.`);
    }
    const realized = parameter.iec_type === "REAL" ? Math.fround(value) : value;
    if (!Number.isFinite(realized) || realized < parameter.minimum || realized > parameter.maximum) throw new Error(`Realized PID gain for ${symbol} is outside its declared binding.`);
    values[symbol] = realized;
  }
  return values;
}

function decodePidAntiWindup(value: unknown): PidControllerLaw["antiWindup"] | undefined {
  if (value === undefined) return undefined;
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const candidate = value as Record<string, unknown>;
  if (Object.keys(candidate).length !== 2 || candidate.method !== "back-calculation" || !label(candidate.trackingGainSymbol)) return undefined;
  return { method: "back-calculation", trackingGainSymbol: candidate.trackingGainSymbol };
}

function label(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && value.length <= 1024 && !/[\u0000-\u001f]/.test(value);
}

/** Joins a real selected plant with the independently declared law u=Kp*(SP-PV). */
export function proportionalLoopAnalysis(loop: ControlLoopDefinition, value: GeneralModel, selectedGain: number): DeclaredControlAnalysis {
  const law = decodeProportionalLaw(loop.controller.law, loop.parameterBinding);
  const model = linearTransferModel(value);
  if (!law || !loop.signals.setpoint) throw new Error("Declare the proportional law, gain and setpoint before analysis.");
  if (!model || model.kind !== "transfer-function") throw new Error("This proportional polynomial adapter requires a selected continuous transfer-function model.");
  const gain = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === law.gainSymbol)!;
  const inputUnit = loop.signals.manipulatedVariable.unit;
  const outputUnit = loop.signals.processVariable.unit;
  if (model.inputs[0].unit !== inputUnit || model.outputs[0].unit !== outputUnit
      || !(gain.unit === `${inputUnit}/${outputUnit}` || (inputUnit === outputUnit && gain.unit === "1"))) {
    throw new Error("Selected model and proportional gain units must match the bound output and process coordinates.");
  }
  const analysis = decodeDeclaredAnalysis({ plant: { kind: model.kind, numerator: [...model.numerator], denominator: [...model.denominator],
    delaySeconds: model.delaySeconds, inputUnit, outputUnit, revision: model.revision, source: model.source, validation: model.validation },
    controller: { numerator: [{ parameter: "Kp" }], denominator: [1], parameters: {
      Kp: { value: selectedGain, minimum: gain.minimum, maximum: gain.maximum, unit: gain.unit } } },
    rootLocus: { parameter: "Kp", selectedValue: selectedGain, values: [...new Set([gain.minimum, selectedGain, gain.maximum])].sort((a, b) => a - b) } });
  if (!analysis) throw new Error("The selected gain or connected controller/model declaration is invalid.");
  return analysis;
}
