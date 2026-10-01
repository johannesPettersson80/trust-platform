import type { IdentificationSample, IdentifiedModel, ModelKind } from "./identification";

export interface ExcitationFit {
  recommended: IdentifiedModel;
  models: readonly IdentifiedModel[];
  input0: number;
  output0: number;
  points: readonly { timeSeconds: number; measured: number; predicted: number; residual: number }[];
}

type SimpleKind = Extract<ModelKind, "static_gain" | "integrating" | "first_order" | "first_order_dead_time">;
interface Candidate { kind: SimpleKind; gain: number; tau?: number; steps: number; error: number; basis: number[] }
const COMPLEXITY: Record<SimpleKind, number> = { static_gain: 1, integrating: 1, first_order: 2, first_order_dead_time: 3 };

/**
 * The simple candidates of the Find model flow (static, integrating, first-order and delayed first-order)
 * fitted to any recorded excitation, such as a relay, PRBS or multisine test. Each candidate is simulated
 * under the measured input, held between samples, from the steady baseline before the first input change;
 * its gain is the least-squares value for those dynamics. The simplest adequate candidate is recommended.
 */
export function identifyPlantFromExcitation(samples: readonly IdentificationSample[], maximumNormalizedError: number): ExcitationFit {
  if (samples.length < 20 || !samples.every((sample, index) => sample.quality === "good" && Number.isFinite(sample.timeSeconds)
      && Number.isFinite(sample.input) && Number.isFinite(sample.output) && (index === 0 || sample.timeSeconds > samples[index - 1].timeSeconds))) {
    throw new Error("Identification requires at least 20 good, finite, time-ordered samples.");
  }
  const dt = (samples[samples.length - 1].timeSeconds - samples[0].timeSeconds) / (samples.length - 1);
  if (samples.some((sample, index) => Math.abs(sample.timeSeconds - samples[0].timeSeconds - index * dt) > dt * 0.01)) {
    throw new Error("Identification from a recorded excitation requires uniformly sampled data.");
  }
  const tolerance = 1e-9 * Math.max(1, ...samples.map(sample => Math.abs(sample.input)));
  const first = samples.findIndex((sample, index) => index > 0 && Math.abs(sample.input - samples[index - 1].input) > tolerance);
  if (first < 3) throw new Error("A steady baseline of at least three samples before the first input change is required.");
  const input0 = mean(samples.slice(0, first).map(sample => sample.input));
  const output0 = mean(samples.slice(0, first).map(sample => sample.output));
  const input = samples.map(sample => sample.input - input0), output = samples.map(sample => sample.output - output0);
  const scale = Math.max(1e-12, Math.max(...samples.map(sample => sample.output)) - Math.min(...samples.map(sample => sample.output)));
  const horizon = samples[samples.length - 1].timeSeconds - samples[first].timeSeconds;
  const fit = (kind: SimpleKind, tau: number | undefined, steps: number): Candidate | undefined => {
    const basis = simulate(kind, input, dt, tau, steps);
    let numerator = 0, denominator = 0;
    basis.forEach((value, k) => { numerator += value * output[k]; denominator += value * value; });
    if (!(denominator > Number.EPSILON)) return undefined;
    const gain = numerator / denominator;
    const error = Math.sqrt(output.reduce((sum, value, k) => sum + (value - gain * basis[k]) ** 2, 0) / output.length) / scale;
    return { kind, gain, ...(tau === undefined ? {} : { tau }), steps, error, basis };
  };
  const better = (left: Candidate | undefined, right: Candidate | undefined) => !left || right && right.error < left.error ? right : left;
  const ratio = Math.pow(Math.max(dt, horizon) / dt, 1 / 44);
  const lag = (kind: "first_order" | "first_order_dead_time", delays: readonly number[]): Candidate | undefined => {
    let best: Candidate | undefined;
    for (const steps of delays) for (const tau of logarithmicGrid(dt, Math.max(dt, horizon), 45)) best = better(best, fit(kind, tau, steps));
    const coarse = best;
    if (coarse?.tau === undefined) return coarse;
    // Refine the time constant between its coarse neighbours, then the delay at that time constant.
    for (const tau of logarithmicGrid(coarse.tau / ratio, coarse.tau * ratio, 41)) best = better(best, fit(kind, tau, coarse.steps));
    const refined = best ?? coarse;
    for (const steps of delays.filter(steps => Math.abs(steps - refined.steps) <= 2)) best = better(best, fit(kind, refined.tau, steps));
    return best;
  };
  const maximumSteps = Math.max(1, Math.round(Math.min(horizon * 0.3, 5) / dt));
  const candidates = [fit("static_gain", undefined, 0), fit("integrating", undefined, 0), lag("first_order", [0]),
    lag("first_order_dead_time", Array.from({ length: maximumSteps }, (_, index) => index + 1))]
    .filter((candidate): candidate is Candidate => candidate !== undefined && Number.isFinite(candidate.gain));
  const models = candidates.map(candidate => identifiedModel(candidate, dt));
  const adequate = candidates.filter(candidate => candidate.error <= maximumNormalizedError)
    .sort((left, right) => COMPLEXITY[left.kind] - COMPLEXITY[right.kind] || left.error - right.error);
  if (!adequate.length) {
    const best = [...candidates].sort((left, right) => left.error - right.error)[0];
    throw new Error(`No simple model explains the recorded excitation${best ? ` (best ${best.kind.replace(/_/g, " ")}: normalized error ${best.error.toFixed(3)})` : ""}.`);
  }
  const selected = adequate[0];
  return { recommended: identifiedModel(selected, dt), models, input0, output0,
    points: samples.map((sample, k) => {
      const predicted = output0 + selected.gain * selected.basis[k];
      return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted, residual: sample.output - predicted };
    }) };
}

/** Unit-gain response to the held input deviations; the delay is a whole number of samples. */
function simulate(kind: SimpleKind, input: readonly number[], dt: number, tau: number | undefined, steps: number): number[] {
  const response = new Array<number>(input.length);
  const decay = tau === undefined ? 0 : Math.exp(-dt / tau);
  let state = 0;
  for (let k = 0; k < input.length; k++) {
    const held = input[k - steps] ?? 0;
    response[k] = kind === "static_gain" ? held : state;
    if (kind === "integrating") state += dt * held;
    else if (kind !== "static_gain") state = decay * state + (1 - decay) * held;
  }
  return response;
}

function identifiedModel(candidate: Candidate, dt: number): IdentifiedModel {
  return { kind: candidate.kind, gain: candidate.gain,
    ...(candidate.tau === undefined ? {} : { timeConstantSeconds: candidate.tau }),
    ...(candidate.kind === "first_order_dead_time" ? { deadTimeSeconds: candidate.steps * dt } : {}),
    normalizedError: candidate.error, normalizedEstimationError: candidate.error, stable: candidate.kind !== "integrating" };
}

function logarithmicGrid(minimum: number, maximum: number, count: number): number[] {
  if (maximum <= minimum) return [minimum];
  return Array.from({ length: count }, (_, index) => minimum * Math.pow(maximum / minimum, index / (count - 1)));
}

function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
