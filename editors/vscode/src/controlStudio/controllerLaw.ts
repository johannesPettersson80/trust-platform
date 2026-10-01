import { linearTransferModel } from "./linearTransferModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import { decodeDeclaredAnalysis, type DeclaredControlAnalysis } from "./declaredAnalysis";
import { type GeneralModel } from "./generalModel";

export interface ProportionalControllerLaw { kind: "proportional"; gainSymbol: string }
export interface PidControllerLaw {
  kind: "pid";
  parameterRecordSymbol: string;
  /** An optional program counter a project may bump when it adopts a new record; Studio does not require one. */
  parameterRevisionSymbol?: string;
  proportionalGainSymbol: string;
  integralGainSymbol: string;
  derivativeGainSymbol: string;
  /** Record field β in P = Kp·(β·SP − PV): the proportional setpoint weight, dimensionless in [0, 1]. */
  proportionalSetpointWeightSymbol?: string;
  /** Record field γ in D = Kd·(γ·dSP/dt − dPV/dt): the derivative setpoint weight, dimensionless in [0, 1]. */
  derivativeSetpointWeightSymbol?: string;
  /** Program setting Tf of the first-order filter on the measurement the controller uses, in seconds; 0 bypasses it. */
  measurementFilterTimeSymbol?: string;
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
  const expected = ["kind", "parameterRecordSymbol", "proportionalGainSymbol", "integralGainSymbol", "derivativeGainSymbol"];
  const antiWindup = decodePidAntiWindup(law.antiWindup);
  const optional = ["parameterRevisionSymbol", "antiWindup", "proportionalSetpointWeightSymbol", "derivativeSetpointWeightSymbol", "measurementFilterTimeSymbol"];
  if (law.kind !== "pid" || !Object.keys(law).every(key => expected.includes(key) || optional.includes(key)) ||
      !expected.every(key => key in law) || (law.antiWindup !== undefined && !antiWindup) ||
      (law.proportionalSetpointWeightSymbol !== undefined && !label(law.proportionalSetpointWeightSymbol)) ||
      (law.derivativeSetpointWeightSymbol !== undefined && !label(law.derivativeSetpointWeightSymbol)) ||
      (law.measurementFilterTimeSymbol !== undefined && !label(law.measurementFilterTimeSymbol)) ||
      (law.parameterRevisionSymbol !== undefined && !label(law.parameterRevisionSymbol)) ||
      !expected.slice(1).every(key => label(law[key]))) return undefined;
  const weight = law.proportionalSetpointWeightSymbol as string | undefined;
  const derivativeWeight = law.derivativeSetpointWeightSymbol as string | undefined;
  const filter = law.measurementFilterTimeSymbol as string | undefined;
  const symbols = [law.proportionalGainSymbol, law.integralGainSymbol, law.derivativeGainSymbol,
    ...(antiWindup ? [antiWindup.trackingGainSymbol] : []), ...(weight ? [weight] : []), ...(derivativeWeight ? [derivativeWeight] : []),
    ...(filter ? [filter] : [])] as string[];
  if (new Set(symbols).size !== symbols.length || !symbols.every(symbol => {
    const parameter = binding?.parameters.find(value => value.symbol === symbol);
    return parameter !== undefined && (parameter.iec_type === "REAL" || parameter.iec_type === "LREAL");
  })) return undefined;
  if (antiWindup && binding?.parameters.find(parameter => parameter.symbol === antiWindup.trackingGainSymbol)?.unit !== "1/s") return undefined;
  for (const symbol of [weight, derivativeWeight]) {
    const parameter = symbol ? binding?.parameters.find(entry => entry.symbol === symbol) : undefined;
    if (symbol && (parameter?.unit !== "1" || parameter.minimum < 0 || parameter.maximum > 1)) return undefined;
  }
  const filterParameter = filter ? binding?.parameters.find(parameter => parameter.symbol === filter) : undefined;
  if (filter && (filterParameter?.unit !== "s" || filterParameter.minimum < 0)) return undefined;
  return {
    kind: "pid", parameterRecordSymbol: law.parameterRecordSymbol as string,
    ...(law.parameterRevisionSymbol !== undefined ? { parameterRevisionSymbol: law.parameterRevisionSymbol as string } : {}),
    proportionalGainSymbol: law.proportionalGainSymbol as string,
    integralGainSymbol: law.integralGainSymbol as string,
    derivativeGainSymbol: law.derivativeGainSymbol as string,
    ...(weight ? { proportionalSetpointWeightSymbol: weight } : {}),
    ...(derivativeWeight ? { derivativeSetpointWeightSymbol: derivativeWeight } : {}),
    ...(filter ? { measurementFilterTimeSymbol: filter } : {}),
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
  proportionalSetpointWeight?: number;
  derivativeSetpointWeight?: number;
  measurementFilterSeconds?: number;
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
  if (law.proportionalSetpointWeightSymbol) {
    // A calculated candidate is the standard form; only an engineer-entered or pinned weight differs from 1.
    entries.push([law.proportionalSetpointWeightSymbol, gains.proportionalSetpointWeight ?? 1]);
  } else if (gains.proportionalSetpointWeight !== undefined && gains.proportionalSetpointWeight !== 1) {
    throw new Error("A declared proportional setpoint weight is required before materializing a weighted controller.");
  }
  if (law.derivativeSetpointWeightSymbol) {
    // A calculated candidate differentiates the measurement (γ = 0), so a target step adds no derivative kick.
    entries.push([law.derivativeSetpointWeightSymbol, gains.derivativeSetpointWeight ?? 0]);
  } else if (gains.derivativeSetpointWeight !== undefined && gains.derivativeSetpointWeight !== 0) {
    throw new Error("A declared derivative setpoint weight is required before materializing a controller that differentiates the setpoint.");
  }
  if (law.measurementFilterTimeSymbol) {
    // A calculated candidate is designed without a measurement filter; Tf = 0 bypasses it unless Tf is entered or pinned.
    entries.push([law.measurementFilterTimeSymbol, gains.measurementFilterSeconds ?? 0]);
  } else if (gains.measurementFilterSeconds !== undefined && gains.measurementFilterSeconds !== 0) {
    throw new Error("A declared measurement filter time is required before materializing a filtered controller.");
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
