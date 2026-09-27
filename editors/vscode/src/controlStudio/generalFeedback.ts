import { analyzeGeneralModelFrequency, decodeGeneralModel, type GeneralModel, type GeneralFrequencyPoint, type GeneralModelFrequencyResult } from "./generalModel";
import { classicalStabilityMargins, eigenvalues, nyquistStability, type ComplexValue } from "./numerics";
import { analyzeTransferFunction, rootsOfRealPolynomial } from "./numerics/classicalAnalysis";
import { linearTransferModel } from "./linearTransferModel";
export interface GeneralFeedbackDefinition {
  plant: GeneralModel;
  controller: GeneralModel;
  plantChannel: { inputId: string; outputId: string };
  controllerChannel: { inputId: string; outputId: string };
  feedback: "negative";
  declaredController?: DeclaredFeedbackController;
}
export interface DeclaredFeedbackController {
  kind: "declared-rational";
  numerator: readonly number[];
  denominator: readonly number[];
  parameter: string;
  selectedValue: number;
  revision: string;
  inputUnit: string;
  outputUnit: string;
}
export interface GeneralFeedbackResult {
  status: "qualified";
  definition: GeneralFeedbackDefinition;
  openLoop: readonly GeneralFrequencyPoint[];
  nyquistScope: string;
  sensitivity?: readonly {
    frequencyRadPerSecond: number;
    sensitivityMagnitudeDb: number;
    complementarySensitivityMagnitudeDb: number;
    controlSensitivityMagnitudeDb: number;
  }[];
  nyquistStability?: { status: "qualified"; encirclementsCounterClockwise: number; openLoopRightHalfPlanePoles: number; closedLoopRightHalfPlanePoles: number; closedLoopStable: boolean; minimumDistanceToMinusOne: number; endpointMagnitude: number }
    | { status: "rejected"; reason: string };
  closedLoop: { status: "qualified"; poles: readonly ComplexValue[]; stable: boolean; scope: string }
    | { status: "unavailable"; reason: string };
  margins: { status: "estimated"; gainCrossoverRadPerSecond: number; phaseMarginDegrees: number; gainMarginDb?: number; scope: string }
    | { status: "unavailable"; reason: string };
}
interface Realization { a: number[][]; b: number[]; c: number[]; d: number }
const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };
export function decodeGeneralFeedback(value: unknown): GeneralFeedbackDefinition | undefined {
  if (!record(value) || value.feedback !== "negative" || !record(value.plantChannel) || !record(value.controllerChannel)) return undefined;
  const plant = decodeGeneralModel(value.plant); const controller = decodeGeneralModel(value.controller);
  if (!plant || !controller) return undefined;
  const pi = plant.inputs.find(c => c.id === (value.plantChannel as Record<string, unknown>).inputId);
  const po = plant.outputs.find(c => c.id === (value.plantChannel as Record<string, unknown>).outputId);
  const ci = controller.inputs.find(c => c.id === (value.controllerChannel as Record<string, unknown>).inputId);
  const co = controller.outputs.find(c => c.id === (value.controllerChannel as Record<string, unknown>).outputId);
  if (!pi || !po || !ci || !co || pi.unit !== co.unit || po.unit !== ci.unit) return undefined;
  if ((plant.kind === "discrete-state-space") !== (controller.kind === "discrete-state-space")) return undefined;
  if (plant.kind === "discrete-state-space" && controller.kind === "discrete-state-space" && plant.sampleTimeSeconds !== controller.sampleTimeSeconds) return undefined;
  const declaredController = value.declaredController === undefined ? undefined : decodeDeclaredController(value.declaredController);
  if (value.declaredController !== undefined && (!declaredController || declaredController.inputUnit !== po.unit || declaredController.outputUnit !== pi.unit)) return undefined;
  return { plant, controller, plantChannel: { inputId: pi.id, outputId: po.id }, controllerChannel: { inputId: ci.id, outputId: co.id }, feedback: "negative", ...(declaredController ? { declaredController } : {}) };
}
export function analyzeGeneralFeedback(input: unknown, frequencies: readonly number[]): GeneralFeedbackResult | { status: "rejected"; reason: string } {
  const definition = decodeGeneralFeedback(input);
  if (!definition) return { status: "rejected", reason: "invalid-feedback-model-channels-units-or-domain" };
  const { plant, controller } = definition;
  const plantResponse = analyzeGeneralModelFrequency(plant, frequencies);
  if (plantResponse.status !== "qualified") return plantResponse;
  if (definition.declaredController) return analyzeDeclaredFeedback(definition, frequencies, plantResponse);
  const p = realization(plant, definition.plantChannel); const c = realization(controller, definition.controllerChannel);
  const feedthrough = 1 + p.d * c.d;
  if (plant.delaySeconds + controller.delaySeconds === 0 && (!Number.isFinite(feedthrough) || Math.abs(feedthrough) <= 1e-10 * Math.max(1, Math.abs(p.d * c.d)))) return { status: "rejected", reason: "singular-algebraic-loop" };
  const controllerResponse = analyzeGeneralModelFrequency(controller, frequencies);
  if (controllerResponse.status !== "qualified") return controllerResponse;
  const pg = plantResponse.channels.find(channel => channel.input.id === definition.plantChannel.inputId && channel.output.id === definition.plantChannel.outputId)!;
  const cg = controllerResponse.channels.find(channel => channel.input.id === definition.controllerChannel.inputId && channel.output.id === definition.controllerChannel.outputId)!;
  const openLoop = pg.points.map((value, i) => {
    const cv = cg.points[i]; const real = value.real * cv.real - value.imaginary * cv.imaginary;
    const imaginary = value.real * cv.imaginary + value.imaginary * cv.real;
    return { frequencyRadPerSecond: value.frequencyRadPerSecond, real, imaginary, magnitude: Math.hypot(real, imaginary), phaseRadians: Math.atan2(imaginary, real) };
  });
  if (!openLoop.every(point => Object.values(point).every(Number.isFinite))) return { status: "rejected", reason: "nonfinite-feedback-response" };
  const sensitivity = openLoop.map((loop, i) => {
    const controllerPoint = cg.points[i];
    const denominator = { real: 1 + loop.real, imaginary: loop.imaginary };
    const denominatorMagnitudeSquared = denominator.real ** 2 + denominator.imaginary ** 2;
    const sMagnitude = 1 / Math.sqrt(denominatorMagnitudeSquared);
    const tMagnitude = loop.magnitude * sMagnitude;
    const cMagnitude = controllerPoint.magnitude * sMagnitude;
    return { frequencyRadPerSecond: loop.frequencyRadPerSecond,
      sensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, sMagnitude)),
      complementarySensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, tMagnitude)),
      controlSensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, cMagnitude)) };
  });
  const nyquist = plant.kind === "transfer-function" && controller.kind === "transfer-function"
    ? nyquistStability({ numerator: multiply(plant.numerator, controller.numerator), denominator: multiply(plant.denominator, controller.denominator), delaySeconds: plant.delaySeconds + controller.delaySeconds }, {
      minimumFrequencyRadPerSecond: frequencies[0], maximumFrequencyRadPerSecond: frequencies[frequencies.length - 1],
      pointsPerDecade: Math.max(10, Math.min(100, Math.floor(511 / Math.log10(frequencies[frequencies.length - 1] / frequencies[0])))),
    }) as GeneralFeedbackResult["nyquistStability"]
    : undefined;
  let closedLoop: GeneralFeedbackResult["closedLoop"];
  if (plant.delaySeconds + controller.delaySeconds > 0) closedLoop = { status: "unavailable", reason: "Pure delay is exact in frequency response; finite closed-loop poles require an explicit delay approximation, which was not supplied." };
  else {
    const n = p.a.length; const m = c.a.length;
    const matrix = Array.from({ length: n + m }, (_, row) => Array.from({ length: n + m }, (_, column) => {
      if (row < n && column < n) return p.a[row][column] - p.b[row] * c.d * p.c[column] / feedthrough;
      if (row < n) return p.b[row] * c.c[column - n] / feedthrough;
      if (column < n) return -c.b[row - n] * p.c[column] / feedthrough;
      return c.a[row - n][column - n] - c.b[row - n] * p.d * c.c[column - n] / feedthrough;
    }));
    const result = matrix.length ? eigenvalues(matrix, tolerance) : { status: "qualified" as const, values: [] };
    if (result.status !== "qualified") return result;
    const poles = [...result.values].sort((a,b) => a.real - b.real || a.imaginary - b.imaginary);
    closedLoop = { status: "qualified", poles,
      stable: poles.every(pole => plant.kind === "discrete-state-space" ? Math.hypot(pole.real, pole.imaginary) < 1 - 1e-10 : pole.real < -1e-10),
      scope: "Exact poles of the declared selected-channel negative-feedback realization. Other inputs held fixed. This does not establish full MIMO feedback stability; undeclared nonlinear limits, filters and continuous-model sample/hold are not represented." };
  }
  return { status: "qualified", definition, openLoop, sensitivity, ...(nyquist ? { nyquistStability: nyquist } : {}), closedLoop, margins: estimateMargins(definition, frequencies),
    nyquistScope: nyquist ? "The shared Nyquist stability owner evaluated a bounded closed contour for the selected continuous transfer models with their declared delay; encirclement and pole counts are reported with its numerical limits." : "Finite sampled positive-frequency curve; conjugate negative-frequency points are shown for the real model. A full contour is unavailable for state-space selections without a compatible transfer conversion." };
}
function analyzeDeclaredFeedback(
  definition: GeneralFeedbackDefinition,
  frequencies: readonly number[],
  plantResponse: Extract<GeneralModelFrequencyResult, { status: "qualified" }>,
): GeneralFeedbackResult | { status: "rejected"; reason: string } {
  const plant = definition.plant;
  const law = definition.declaredController!;
  let transferPlant: Extract<GeneralModel, { kind: "transfer-function" }>;
  try { transferPlant = plant.kind === "continuous-state-space" ? linearTransferModel(plant) : plant.kind === "transfer-function" ? plant : undefined as never; }
  catch (error) { return { status: "rejected", reason: error instanceof Error ? error.message : "Declared rational feedback requires a supported SISO plant." }; }
  if (!transferPlant) return { status: "rejected", reason: "Declared rational feedback requires a supported SISO plant." };
  const controllerResponse = analyzeTransferFunction({ numerator: law.numerator, denominator: law.denominator, delaySeconds: 0 }, frequencies) as {
    status: string; reason?: string; bode?: readonly { magnitudeDb: number; phaseDegrees: number }[];
  };
  if (controllerResponse.status !== "qualified" || !controllerResponse.bode) return { status: "rejected", reason: controllerResponse.reason ?? "Declared controller frequency response could not be qualified." };
  const plantChannel = plantResponse.channels.find(channel => channel.input.id === definition.plantChannel.inputId && channel.output.id === definition.plantChannel.outputId);
  if (!plantChannel || plantChannel.points.length !== controllerResponse.bode.length) return { status: "rejected", reason: "Declared feedback channels do not share a frequency grid." };
  const controllerPoints = controllerResponse.bode.map((point, index) => {
    const magnitude = 10 ** (point.magnitudeDb / 20); const phase = point.phaseDegrees * Math.PI / 180;
    return { frequencyRadPerSecond: frequencies[index], real: magnitude * Math.cos(phase), imaginary: magnitude * Math.sin(phase), magnitude, phaseRadians: phase };
  });
  const openLoop = plantChannel.points.map((value, index) => {
    const controllerPoint = controllerPoints[index];
    const real = value.real * controllerPoint.real - value.imaginary * controllerPoint.imaginary;
    const imaginary = value.real * controllerPoint.imaginary + value.imaginary * controllerPoint.real;
    return { frequencyRadPerSecond: value.frequencyRadPerSecond, real, imaginary, magnitude: Math.hypot(real, imaginary), phaseRadians: Math.atan2(imaginary, real) };
  });
  if (!openLoop.every(point => Object.values(point).every(Number.isFinite))) return { status: "rejected", reason: "nonfinite-feedback-response" };
  const sensitivity = sensitivityPoints(openLoop, controllerPoints);
  const characteristic = polynomialAdd(multiply(transferPlant.denominator, law.denominator), multiply(transferPlant.numerator, law.numerator));
  let closedLoop: GeneralFeedbackResult["closedLoop"];
  if (transferPlant.delaySeconds > 0) closedLoop = { status: "unavailable", reason: "Pure delay is exact in frequency response; finite closed-loop poles require an explicit delay approximation, which was not supplied." };
  else {
    const poles = rootsOfRealPolynomial(characteristic);
    if (!poles) return { status: "rejected", reason: "Declared closed-loop characteristic polynomial could not be solved." };
    const sorted = [...poles].sort((a, b) => a.real - b.real || a.imaginary - b.imaginary);
    closedLoop = { status: "qualified", poles: sorted, stable: sorted.every(pole => pole.real < -1e-10),
      scope: `Exact roots of the selected plant denominator times the declared controller denominator plus their numerator product${plant.kind === "continuous-state-space" ? "; the full SISO state denominator is retained through the existing linear transfer conversion" : ""}. This is an analytic declared model result; nonlinear limits, sample/hold and runtime state are not represented.` };
  }
  const nyquist = nyquistStability({ numerator: multiply(transferPlant.numerator, law.numerator), denominator: multiply(transferPlant.denominator, law.denominator), delaySeconds: transferPlant.delaySeconds }, {
    minimumFrequencyRadPerSecond: frequencies[0], maximumFrequencyRadPerSecond: frequencies[frequencies.length - 1],
    pointsPerDecade: Math.max(10, Math.min(100, Math.floor(511 / Math.log10(frequencies[frequencies.length - 1] / frequencies[0])))),
  }) as Exclude<GeneralFeedbackResult["nyquistStability"], undefined>;
  return { status: "qualified", definition, openLoop, sensitivity, nyquistStability: nyquist, closedLoop,
    margins: estimateMargins(definition, frequencies),
    nyquistScope: nyquist.status === "qualified" ? "The shared Nyquist stability owner evaluated the declared controller loop with its exact plant delay; encirclement and pole counts are reported with its numerical limits." : `The shared Nyquist stability owner rejected the requested contour: ${nyquist.reason}` };
}
function sensitivityPoints(openLoop: readonly GeneralFrequencyPoint[], controller: readonly GeneralFrequencyPoint[]) {
  return openLoop.map((loop, index) => {
    const controllerPoint = controller[index]; const real = 1 + loop.real; const imaginary = loop.imaginary;
    const denominatorMagnitude = Math.hypot(real, imaginary); const s = 1 / denominatorMagnitude;
    return { frequencyRadPerSecond: loop.frequencyRadPerSecond,
      sensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, s)),
      complementarySensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, loop.magnitude * s)),
      controlSensitivityMagnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, controllerPoint.magnitude * s)) };
  });
}
function polynomialAdd(left: readonly number[], right: readonly number[]): number[] {
  const length = Math.max(left.length, right.length);
  const paddedLeft = [...Array(length - left.length).fill(0), ...left];
  const paddedRight = [...Array(length - right.length).fill(0), ...right];
  return paddedLeft.map((value, index) => value + paddedRight[index]);
}
function realization(model: GeneralModel, channel: { inputId: string; outputId: string }): Realization {
  if (model.kind !== "transfer-function") {
    const input = model.inputs.findIndex(c => c.id === channel.inputId); const output = model.outputs.findIndex(c => c.id === channel.outputId);
    return { a: model.a.map(row => [...row]), b: model.b.map(row => row[input]), c: [...model.c[output]], d: model.d[output][input] };
  }
  const denominator = model.denominator.map(v => v / model.denominator[0]);
  const numerator = [...Array<number>(denominator.length - model.numerator.length).fill(0), ...model.numerator].map(v => v / model.denominator[0]);
  const order = denominator.length - 1;
  return { a: Array.from({ length: order }, (_, row) => Array.from({ length: order }, (_, column) => row === 0 ? -denominator[column + 1] : column === row - 1 ? 1 : 0)),
    b: Array.from({ length: order }, (_, i) => i === 0 ? 1 : 0), c: Array.from({ length: order }, (_, i) => numerator[i + 1] - numerator[0] * denominator[i + 1]), d: numerator[0] };
}
function estimateMargins(definition: GeneralFeedbackDefinition, frequencies: readonly number[]): GeneralFeedbackResult["margins"] {
  const { plant, controller } = definition;
  if (plant.kind !== "transfer-function") return { status: "unavailable", reason: "Classical margin estimates currently require continuous transfer-function models." };
  const controllerTransfer = controller.kind === "transfer-function" ? controller : undefined;
  if (!definition.declaredController && !controllerTransfer) return { status: "unavailable", reason: "Classical margin estimates currently require continuous transfer-function models." };
  const numerator = definition.declaredController ? definition.declaredController.numerator : controllerTransfer!.numerator;
  const denominator = definition.declaredController ? definition.declaredController.denominator : controllerTransfer!.denominator;
  const delaySeconds = plant.delaySeconds + (definition.declaredController ? 0 : controllerTransfer!.delaySeconds);
  const decades = Math.log10(frequencies[frequencies.length-1]/frequencies[0]);
  const pointsPerDecade = Math.min(100, Math.floor(511 / decades));
  if (!Number.isFinite(decades) || decades <= 0 || pointsPerDecade < 10) return { status: "unavailable", reason: "The frequency range cannot support a bounded crossover estimate." };
  const largestInterval = frequencies[frequencies.length - 1] * (1 - 10 ** (-1 / pointsPerDecade));
  if (largestInterval * delaySeconds > Math.PI / 2) return { status: "unavailable", reason: "Pure-delay phase change exceeds the bounded grid resolution; narrow the frequency range before estimating margins." };
  const result = classicalStabilityMargins({ numerator: multiply(plant.numerator, numerator), denominator: multiply(plant.denominator, denominator), delaySeconds },
    { minimumFrequencyRadPerSecond: frequencies[0], maximumFrequencyRadPerSecond: frequencies[frequencies.length-1], pointsPerDecade }) as {
      status: string; gainCrossoverRadPerSecond: number; phaseMarginDegrees: number; gainMarginDb: number | "unbounded";
    };
  if (result.status !== "qualified" || !Number.isFinite(result.gainCrossoverRadPerSecond) || !Number.isFinite(result.phaseMarginDegrees)) return { status: "unavailable", reason: "No qualified gain crossover was found in the finite requested range." };
  return { status: "estimated", gainCrossoverRadPerSecond: result.gainCrossoverRadPerSecond, phaseMarginDegrees: result.phaseMarginDegrees,
    ...(typeof result.gainMarginDb === "number" && Number.isFinite(result.gainMarginDb) ? { gainMarginDb: result.gainMarginDb } : {}),
    scope: "Sampling-limited first-crossover estimate in the requested range (at most 512 samples). Other crossovers may exist. No phase crossover means gain margin is unavailable, not infinite. Margins alone do not qualify stability." };
}
function multiply(a: readonly number[], b: readonly number[]): number[] {
  const result = Array<number>(a.length+b.length-1).fill(0); a.forEach((x,i)=>b.forEach((y,j)=>{result[i+j]+=x*y;}));return result;
}
function decodeDeclaredController(value: unknown): DeclaredFeedbackController | undefined {
  if (!record(value) || value.kind !== "declared-rational" || !Array.isArray(value.numerator) || !Array.isArray(value.denominator)
      || value.numerator.length === 0 || value.denominator.length === 0 || !value.numerator.every(finite)
      || !value.denominator.every(finite) || value.denominator[0] === 0 || typeof value.parameter !== "string"
      || !finite(value.selectedValue) || typeof value.revision !== "string" || typeof value.inputUnit !== "string" || typeof value.outputUnit !== "string") return undefined;
  return value as unknown as DeclaredFeedbackController;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
