import { rootLocus } from "./numerics";

export interface DeclaredParameter {
  value: number;
  minimum: number;
  maximum: number;
  unit: string;
}
export type DeclaredCoefficient = number | { parameter: string; scale?: number };
export interface DeclaredControlAnalysis {
  plant: {
    kind: "transfer-function";
    numerator: readonly number[];
    denominator: readonly number[];
    delaySeconds: number;
    inputUnit: string;
    outputUnit: string;
    revision: string;
    source: "provided" | "fitted" | "simulation";
    validation: "unvalidated" | "validated";
  };
  controller: {
    numerator: readonly DeclaredCoefficient[];
    denominator: readonly DeclaredCoefficient[];
    parameters: Readonly<Record<string, DeclaredParameter>>;
  };
  rootLocus: { parameter: string; values: readonly number[]; selectedValue: number };
}
export interface DeclaredRootLocusResult {
  parameter: string;
  parameterUnit: string;
  selectedValue: number;
  fixedParameters: Readonly<Record<string, { value: number; unit: string }>>;
  provenance: string;
  approximation: string;
  branches: readonly { gain: number; poles: readonly { real: number; imaginary: number }[] }[];
}

/** Validates explicitly declared mathematics, independently of PID implementation or a live runtime. */
export function decodeDeclaredAnalysis(value: unknown): DeclaredControlAnalysis | undefined {
  if (!record(value) || !record(value.plant) || !record(value.controller) || !record(value.rootLocus)) return undefined;
  const { plant, controller, rootLocus: sweep } = value;
  if (plant.kind !== "transfer-function" || !polynomial(plant.numerator) || !polynomial(plant.denominator)
      || plant.denominator[0] === 0 || !finite(plant.delaySeconds) || plant.delaySeconds < 0
      || !text(plant.inputUnit) || !text(plant.outputUnit) || !text(plant.revision)
      || !["provided", "fitted", "simulation"].includes(String(plant.source))
      || !["unvalidated", "validated"].includes(String(plant.validation))
      || !record(controller.parameters)) return undefined;
  const parameters = controller.parameters;
  const names = Object.keys(parameters);
  if (names.length === 0 || names.length > 64 || !names.every(name => {
    const p = parameters[name];
    return text(name) && record(p) && finite(p.value) && finite(p.minimum) && finite(p.maximum)
      && p.minimum <= p.maximum && p.value >= p.minimum && p.value <= p.maximum && text(p.unit);
  })) return undefined;
  const coefficients = (value: unknown): boolean => Array.isArray(value) && value.length > 0 && value.length <= 17
    && value.every(coefficient => finite(coefficient) || (record(coefficient)
      && Object.keys(coefficient).every(key => key === "parameter" || key === "scale")
      && typeof coefficient.parameter === "string" && Object.prototype.hasOwnProperty.call(parameters, coefficient.parameter)
      && (coefficient.scale === undefined || finite(coefficient.scale))));
  if (!coefficients(controller.numerator) || !coefficients(controller.denominator)
      || typeof sweep.parameter !== "string" || !Object.prototype.hasOwnProperty.call(parameters, sweep.parameter)
      || !Array.isArray(sweep.values) || sweep.values.length === 0 || sweep.values.length > 1001
      || !finite(sweep.selectedValue) || !sweep.values.includes(sweep.selectedValue)) return undefined;
  const parameter = parameters[sweep.parameter] as unknown as DeclaredParameter;
  if (!sweep.values.every(value => finite(value) && value >= parameter.minimum && value <= parameter.maximum)) return undefined;
  const analysis = value as unknown as DeclaredControlAnalysis;
  for (const gain of analysis.rootLocus.values) {
    const evaluate = (coefficient: DeclaredCoefficient) => typeof coefficient === "number" ? coefficient :
      (coefficient.scale ?? 1) * (coefficient.parameter === analysis.rootLocus.parameter ? gain : analysis.controller.parameters[coefficient.parameter].value);
    const denominator = analysis.controller.denominator.map(evaluate);
    if (!denominator.every(Number.isFinite) || !denominator.some(value => value !== 0)
        || !analysis.controller.numerator.map(evaluate).every(Number.isFinite)) return undefined;
  }
  return analysis;
}

export function calculateDeclaredRootLocus(input: DeclaredControlAnalysis): DeclaredRootLocusResult {
  const analysis = decodeDeclaredAnalysis(input);
  if (!analysis) throw new Error("The declared model or parameter sweep is invalid.");
  const { plant, controller, rootLocus: sweep } = analysis;
  let plantNumerator = [...plant.numerator];
  let plantDenominator = [...plant.denominator];
  if (plant.delaySeconds > 0) {
    plantNumerator = multiply(plantNumerator, [-plant.delaySeconds / 2, 1]);
    plantDenominator = multiply(plantDenominator, [plant.delaySeconds / 2, 1]);
  }
  const substitute = (coefficients: readonly DeclaredCoefficient[], gain: number): number[] => coefficients.map(coefficient => {
    if (typeof coefficient === "number") return coefficient;
    return (coefficient.scale ?? 1) * (coefficient.parameter === sweep.parameter ? gain : controller.parameters[coefficient.parameter].value);
  });
  const branches = sweep.values.map(gain => {
    const numerator = multiply(plantNumerator, substitute(controller.numerator, gain));
    const denominator = multiply(plantDenominator, substitute(controller.denominator, gain));
    if (!denominator.some(value => value !== 0)) throw new Error(`Controller denominator is zero at ${sweep.parameter} = ${gain}.`);
    const result = rootLocus({ numerator, denominator, delaySeconds: 0 }, [1]) as {
      status: string; reason?: string; branches?: { poles: { real: number; imaginary: number }[] }[];
    };
    if (result.status !== "qualified" || !result.branches?.[0]) throw new Error(result.reason ?? "Root solver could not qualify this model.");
    return { gain, poles: [...result.branches[0].poles].sort((a, b) => a.real - b.real || a.imaginary - b.imaginary) };
  });
  return { parameter: sweep.parameter, parameterUnit: controller.parameters[sweep.parameter].unit,
    selectedValue: sweep.selectedValue,
    fixedParameters: Object.fromEntries(Object.entries(controller.parameters).filter(([name]) => name !== sweep.parameter)
      .map(([name, parameter]) => [name, { value: parameter.value, unit: parameter.unit }])),
    provenance: `${plant.source} model ${plant.revision} · ${plant.validation}`,
    approximation: `Declared continuous linear model; nonlinear limits and task sample/hold are not represented.${plant.delaySeconds > 0 ? " Delay uses first-order Padé approximation." : " No model delay."}`,
    branches };
}
function multiply(a: readonly number[], b: readonly number[]): number[] {
  const result = Array<number>(a.length + b.length - 1).fill(0);
  a.forEach((x, i) => b.forEach((y, j) => { result[i + j] += x * y; }));
  return result;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function polynomial(value: unknown): value is number[] { return Array.isArray(value) && value.length > 0 && value.length <= 17 && value.every(finite); }
