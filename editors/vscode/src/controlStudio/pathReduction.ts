import { analyzeGeneralModelFrequency, type GeneralModel, type GeneralStateSpaceModel } from "./generalModel";
import { analyzeGeneralModelTime } from "./generalModelTime";
import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";
import { discretizeStateSpace } from "./numerics/adapter";

/** One path of a model reduced to first order with delay: steady-state gain, time constant and delay. */
export interface ReducedPath { gain: number; timeConstantSeconds: number; delaySeconds: number }

/** Ten of the model's slowest time constants and its delay: long enough for every output to settle. */
export function settlingHorizon(model: GeneralModel, subject: string): number {
  const overview = analyzeGeneralModelFrequency(model, [1]);
  if (overview.status !== "qualified") throw new Error(`Cannot calculate the ${subject}'s step responses: ${overview.reason}.`);
  const discrete = model.kind === "discrete-state-space";
  const rates = overview.poles.map(pole => discrete ? Math.abs(Math.log(Math.hypot(pole.real, pole.imaginary))) / model.sampleTimeSeconds! : Math.abs(pole.real));
  if (!rates.length || rates.some(rate => !(rate > 1e-9))) throw new Error(`A ${subject} path does not settle: the design needs a stable model.`);
  return 10 / Math.min(...rates) + model.delaySeconds;
}

/** The unit-step response of every output to one input, until every output has settled. */
export function openLoopStepResponse(model: GeneralModel, input: number, subject: string): { times: number[]; outputs: number[][]; final: number[] } {
  const horizon = settlingHorizon(model, subject);
  const discrete = model.kind === "discrete-state-space";
  const count = Math.floor(4096 / model.outputs.length) - 1;
  const stepSeconds = discrete ? model.sampleTimeSeconds! : horizon / count;
  const steps = discrete ? Math.min(count, Math.ceil(horizon / stepSeconds)) : count;
  const states = model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length;
  const response = analyzeGeneralModelTime(model, { stepSeconds, durationSeconds: steps * stepSeconds, initialState: Array(states).fill(0),
    input: model.inputs.map((_channel, index) => index === input ? 1 : 0), connection: "open-loop" });
  const times = response.points.map(point => point.timeSeconds), outputs = model.outputs.map((_channel, output) => response.points.map(point => point.zeroState[output]));
  if (times[times.length - 1] < horizon - 1e-9) throw new Error(`A ${subject} path is too slow for the model's sample period to reach steady state.`);
  return { times, outputs, final: outputs.map(values => values[values.length - 1]) };
}

/**
 * Two-point first-order-plus-delay reduction of a settled step response (Specification 35 §17.9):
 * τ = 1.5·(t₆₃ − t₂₈) and θ = t₆₃ − τ, or θ = 0 and τ = t₆₃ when θ would be negative.
 */
export function twoPointPath(times: readonly number[], values: readonly number[], subject: string): ReducedPath {
  const final = values[values.length - 1];
  if (!(Math.abs(final) > 1e-12)) throw new Error(`The ${subject} path has no steady-state gain.`);
  const crossing = (fraction: number) => {
    const target = fraction * final;
    const index = values.findIndex(value => Math.abs(value) >= Math.abs(target));
    if (index <= 0) throw new Error(`The ${subject} path has no step response to reduce.`);
    const [t0, t1, v0, v1] = [times[index - 1], times[index], values[index - 1], values[index]];
    return t0 + (t1 - t0) * (target - v0) / (v1 - v0);
  };
  const t28 = crossing(0.283), t63 = crossing(0.632), tau = 1.5 * (t63 - t28), theta = t63 - tau;
  return theta > 0 ? { gain: final, timeConstantSeconds: tau, delaySeconds: theta } : { gain: final, timeConstantSeconds: t63, delaySeconds: 0 };
}

/** A single-loop view of one controller of a structure: its measurement, the output it drives and its setpoint. */
export function singleLoopView(loop: ControlLoopDefinition, measurement: ControlSignalBinding, output: ControlSignalBinding, setpoint: ControlSignalBinding,
  outputMinimum: number, outputMaximum: number): ControlLoopDefinition {
  return { ...loop, channels: undefined, signals: { processVariable: measurement, manipulatedVariable: output, setpoint },
    limits: { outputMinimum, outputMaximum } };
}

/** A reduced path as a first-order-plus-delay transfer function between two signals, about their operating values. */
export function firstOrderPathModel(model: GeneralModel, path: ReducedPath, input: ControlSignalBinding, output: ControlSignalBinding,
  inputAt: number, outputAt: number): GeneralModel {
  return { kind: "transfer-function", numerator: [path.gain], denominator: [path.timeConstantSeconds, 1], delaySeconds: path.delaySeconds,
    revision: model.revision, source: model.source, validation: model.validation, evidence: model.evidence,
    inputs: [{ id: input.symbol, unit: input.unit, operatingRange: input.range }], outputs: [{ id: output.symbol, unit: output.unit, operatingRange: output.range }],
    operatingPoint: { inputs: [inputAt], outputs: [outputAt] } } as GeneralModel;
}

/** A state-space model sampled at the task period with the input held between samples (exact zero-order hold). */
export function sampledStateSpace(model: GeneralModel, period: number, subject: string): GeneralStateSpaceModel {
  if (model.kind === "transfer-function") throw new Error(`The ${subject} is designed on a state-space model.`);
  if (model.kind === "discrete-state-space") return model;
  const result = discretizeStateSpace({ a: model.a, b: model.b, c: model.c, d: model.d }, period, "exact_zoh");
  if (result.status !== "qualified") throw new Error(`The ${subject} model cannot be sampled at the task period (${result.reason}).`);
  const { a, b, c, d } = result.model;
  return { ...model, kind: "discrete-state-space", a, b, c, d, sampleTimeSeconds: period };
}
