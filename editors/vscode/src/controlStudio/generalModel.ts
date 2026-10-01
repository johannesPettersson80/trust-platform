import { calculateGeneralStateStructure, type GeneralStateStructure } from "./generalStateStructure";
import { analyzeTransferFunction, eigenvalues, relativeGainArray, solveLeastSquares } from "./numerics";
import type { ComplexValue, DenseMatrix } from "./numerics";

export interface ModelChannel {
  id: string;
  unit: string;
  operatingRange: { minimum: number; maximum: number };
}
interface ModelIdentity {
  revision: string;
  source: "provided" | "fitted" | "simulation";
  validation: "unvalidated" | "validated";
  evidence: readonly string[];
  inputs: readonly ModelChannel[];
  outputs: readonly ModelChannel[];
  delaySeconds: number;
  operatingPoint?: { inputs: readonly number[]; outputs: readonly number[] };
}
export interface GeneralTransferModel extends ModelIdentity {
  kind: "transfer-function";
  numerator: readonly number[];
  denominator: readonly number[];
}
export interface GeneralStateSpaceModel extends ModelIdentity {
  kind: "continuous-state-space" | "discrete-state-space";
  states: readonly ModelChannel[];
  a: DenseMatrix; b: DenseMatrix; c: DenseMatrix; d: DenseMatrix;
  sampleTimeSeconds?: number;
}
export type GeneralModel = GeneralTransferModel | GeneralStateSpaceModel;
export interface GeneralFrequencyPoint extends ComplexValue {
  frequencyRadPerSecond: number;
  magnitude: number;
  phaseRadians: number;
}
export type GeneralModelFrequencyResult = { status: "rejected"; reason: string } | {
  status: "qualified";
  model: GeneralModel;
  channels: readonly { input: ModelChannel; output: ModelChannel; points: readonly GeneralFrequencyPoint[] }[];
  poles: readonly ComplexValue[];
  stable: boolean;
  poleScope: string;
  relativeGainMatrix?: readonly (readonly number[])[];
  stateStructure?: GeneralStateStructure;
  capabilities: {
    relativeGain: { available: boolean; reason?: string };
    stepResponse: { available: false; reason: string };
    transferConversion: { available: boolean; reason?: string };
  };
};
const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

/** Validate imported model data before it reaches numerical or plotting code. */
export function decodeGeneralModel(value: unknown): GeneralModel | undefined {
  if (!record(value) || !text(value.revision) || !["provided", "fitted", "simulation"].includes(String(value.source))
      || !["unvalidated", "validated"].includes(String(value.validation)) || !Array.isArray(value.evidence)
      || !value.evidence.every(text) || (value.validation === "validated" && value.evidence.length === 0)
      || !channels(value.inputs) || !channels(value.outputs) || !finite(value.delaySeconds) || value.delaySeconds < 0) return undefined;
  if (value.operatingPoint !== undefined && (!record(value.operatingPoint)
      || !pointCoordinates(value.operatingPoint.inputs, value.inputs)
      || !pointCoordinates(value.operatingPoint.outputs, value.outputs))) return undefined;
  if (value.kind === "transfer-function") {
    if (value.inputs.length !== 1 || value.outputs.length !== 1 || !polynomial(value.numerator)
        || !polynomial(value.denominator) || value.denominator[0] === 0
        || value.numerator.length > value.denominator.length) return undefined;
  } else if (value.kind === "continuous-state-space" || value.kind === "discrete-state-space") {
    if (!channels(value.states)) return undefined;
    const n = value.states.length;
    if (!matrix(value.a, n, n) || !matrix(value.b, n, value.inputs.length)
        || !matrix(value.c, value.outputs.length, n) || !matrix(value.d, value.outputs.length, value.inputs.length)
        || (value.kind === "discrete-state-space" && (!finite(value.sampleTimeSeconds) || value.sampleTimeSeconds <= 0))
        || (value.kind === "continuous-state-space" && value.sampleTimeSeconds !== undefined)) return undefined;
  } else return undefined;
  // Own a snapshot so callers cannot mutate the model after validation.
  return JSON.parse(JSON.stringify(value)) as GeneralModel;
}

/** Evaluate every ordered channel using the shared numerical engine, independently of runtime PID state. */
export function analyzeGeneralModelFrequency(value: unknown, frequencies: readonly number[]): GeneralModelFrequencyResult {
  const model = decodeGeneralModel(value);
  if (!model) return { status: "rejected", reason: "invalid-model" };
  if (frequencies.length === 0 || frequencies.length > 4096 || !frequencies.every((f, i) => finite(f) && f > 0 && (i === 0 || f > frequencies[i - 1]))) {
    return { status: "rejected", reason: "invalid-frequency-grid" };
  }
  if (model.kind === "discrete-state-space" && frequencies.some(f => f >= Math.PI / model.sampleTimeSeconds!)) {
    return { status: "rejected", reason: "alias-risk" };
  }
  const channelsResult: { input: ModelChannel; output: ModelChannel; points: GeneralFrequencyPoint[] }[] = [];
  let poles: readonly ComplexValue[];
  if (model.kind === "transfer-function") {
    const result = analyzeTransferFunction(model, frequencies) as {
      status: string; reason?: string; poles: ComplexValue[];
      bode: { magnitudeDb: number; phaseDegrees: number }[];
    };
    if (result.status !== "qualified") return { status: "rejected", reason: result.reason ?? "solver-failed" };
    poles = result.poles;
    const points = result.bode.map((point, index) => {
      const magnitude = 10 ** (point.magnitudeDb / 20);
      const phase = point.phaseDegrees * Math.PI / 180;
      return frequencyPoint(frequencies[index], magnitude * Math.cos(phase), magnitude * Math.sin(phase));
    });
    channelsResult.push({ input: model.inputs[0], output: model.outputs[0], points });
  } else {
    const eigen = eigenvalues(model.a, tolerance);
    if (eigen.status !== "qualified") return eigen;
    poles = eigen.values;
    for (let output = 0; output < model.outputs.length; output++) {
      for (let input = 0; input < model.inputs.length; input++) {
        const points: GeneralFrequencyPoint[] = [];
        for (const frequency of frequencies) {
          const response = stateSpaceChannel(model, frequency, input, output);
          if (!response) return { status: "rejected", reason: "singular-or-ill-conditioned-frequency" };
          points.push(response);
        }
        channelsResult.push({ input: model.inputs[input], output: model.outputs[output], points });
      }
    }
  }
  if (channelsResult.some(channel => channel.points.some(point => !Object.values(point).every(Number.isFinite)))) {
    return { status: "rejected", reason: "nonfinite-response" };
  }
  // A Bode phase is continuous in frequency: each point takes the branch nearest the previous one, so a delay keeps
  // lowering the phase instead of wrapping it back past ±180°.
  for (const channel of channelsResult) {
    for (let index = 1; index < channel.points.length; index++) {
      const previous = channel.points[index - 1].phaseRadians;
      const phase = channel.points[index].phaseRadians;
      channel.points[index] = { ...channel.points[index], phaseRadians: phase - 2 * Math.PI * Math.round((phase - previous) / (2 * Math.PI)) };
    }
  }
  // Square shape is necessary, not sufficient. RGA additionally needs a qualified DC solve.
  let relativeGain: { available: boolean; reason?: string } = { available: false, reason: model.inputs.length !== model.outputs.length
    ? "Relative gain requires equal input and output counts." : "Relative gain requires a qualified nonsingular DC gain matrix." };
  let relativeGainMatrix: readonly (readonly number[])[] | undefined;
  if (model.inputs.length === model.outputs.length) {
    const dc = model.kind === "transfer-function"
      ? [[model.numerator[model.numerator.length - 1] / model.denominator[model.denominator.length - 1]]]
      : model.outputs.map((_, output) => model.inputs.map((_, input) => stateSpaceChannel(model, 0, input, output)?.real ?? NaN));
    if (dc.flat().every(Number.isFinite)) {
      const result = relativeGainArray(dc, tolerance) as { status: string; matrix?: number[][] };
      if (result.status === "qualified" && result.matrix?.flat().every(Number.isFinite)) {
        relativeGain = { available: true };
        relativeGainMatrix = result.matrix;
      }
    }
  }
  return { status: "qualified", model, channels: channelsResult, poles, relativeGainMatrix,
    ...(model.kind !== "transfer-function" ? { stateStructure: calculateGeneralStateStructure(model) } : {}),
    stable: poles.every(pole => model.kind === "discrete-state-space" ? Math.hypot(pole.real, pole.imaginary) < 1 : pole.real < 0),
    poleScope: "Model poles only; pure delay is not represented in the pole list. This is not closed-loop stability.",
    capabilities: { relativeGain,
      stepResponse: { available: false, reason: "Select a qualified time-response execution adapter and initial conditions." },
      transferConversion: model.kind === "transfer-function" ? { available: true }
        : { available: false, reason: "State-space channel polynomial conversion is not provided by this adapter." } } };
}

function stateSpaceChannel(model: GeneralStateSpaceModel, frequency: number, input: number, output: number): GeneralFrequencyPoint | undefined {
  const n = model.a.length;
  const angle = frequency * (model.sampleTimeSeconds ?? 0);
  const real = model.kind === "discrete-state-space" ? Math.cos(angle) : 0;
  const imaginary = model.kind === "discrete-state-space" ? Math.sin(angle) : frequency;
  // (q I - A) x = B, represented as one real block system for the existing solver.
  const block = Array.from({ length: 2 * n }, (_, row) => Array.from({ length: 2 * n }, (_, column) => {
    const r = row % n; const c = column % n;
    if ((row < n) === (column < n)) return (r === c ? real : 0) - model.a[r][c];
    return r === c ? (row < n ? -imaginary : imaginary) : 0;
  }));
  const rhs = [...model.b.map(row => row[input]), ...Array<number>(n).fill(0)];
  const solved = solveLeastSquares(block, rhs, tolerance);
  if (solved.status !== "qualified") return undefined;
  const re = model.d[output][input] + model.c[output].reduce((sum, value, i) => sum + value * solved.solution[i], 0);
  const im = model.c[output].reduce((sum, value, i) => sum + value * solved.solution[i + n], 0);
  const delay = -frequency * model.delaySeconds;
  return frequencyPoint(frequency, re * Math.cos(delay) - im * Math.sin(delay), re * Math.sin(delay) + im * Math.cos(delay));
}
function frequencyPoint(frequencyRadPerSecond: number, real: number, imaginary: number): GeneralFrequencyPoint {
  return { frequencyRadPerSecond, real, imaginary, magnitude: Math.hypot(real, imaginary), phaseRadians: Math.atan2(imaginary, real) };
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function channels(value: unknown): value is ModelChannel[] {
  return Array.isArray(value) && value.length > 0 && value.length <= 32 && new Set(value.map(c => record(c) ? c.id : undefined)).size === value.length
    && value.every(c => record(c) && text(c.id) && text(c.unit) && record(c.operatingRange)
      && finite(c.operatingRange.minimum) && finite(c.operatingRange.maximum) && c.operatingRange.minimum < c.operatingRange.maximum);
}
function matrix(value: unknown, rows: number, columns: number): value is number[][] {
  return Array.isArray(value) && value.length === rows && value.every(row => Array.isArray(row) && row.length === columns && row.every(finite));
}
function polynomial(value: unknown): value is number[] { return Array.isArray(value) && value.length > 0 && value.length <= 17 && value.every(finite); }

function pointCoordinates(value: unknown, orderedChannels: readonly ModelChannel[]): boolean {
  return Array.isArray(value) && value.length === orderedChannels.length && value.every((coordinate, index) =>
    finite(coordinate) && coordinate >= orderedChannels[index].operatingRange.minimum
      && coordinate <= orderedChannels[index].operatingRange.maximum);
}
