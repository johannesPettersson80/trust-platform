import type { IdentificationSample, IdentifiedModel, ModelKind } from "./identification";
import { blockAveraged, firstGap, gapPhrase, MAXIMUM_FIT_SAMPLES, medianSpacing } from "./evenSampling";
import { baselineNoiseRms, heldUnitResponse, leastSquaresGain, noiseRefusal, type HeldShape } from "./numerics/identification";

export interface ExcitationFit {
  recommended: IdentifiedModel;
  models: readonly IdentifiedModel[];
  input0: number;
  output0: number;
  points: readonly { timeSeconds: number; measured: number; predicted: number; residual: number }[];
}

type SimpleKind = Extract<ModelKind, "static_gain" | "integrating" | "first_order" | "first_order_dead_time">;
interface Candidate { kind: SimpleKind; gain: number; tau?: number; delay: number; error: number; basis: number[] }
const COMPLEXITY: Record<SimpleKind, number> = { static_gain: 1, integrating: 1, first_order: 2, first_order_dead_time: 3 };

/**
 * The simple candidates of the Find model flow (static, integrating, first-order and delayed first-order)
 * fitted to any recorded excitation, such as a relay, PRBS or multisine test. Each candidate is simulated
 * under the recorded input on the recorded sample times, the input held between samples and the dynamics
 * discretized exactly for each interval (A4.3, N2.2). Its gain is the least-squares value for those dynamics,
 * from the steady baseline before the first input change. The simplest adequate candidate is recommended,
 * and its predicted response must be larger than the measured noise.
 *
 * A recording that names the end of its steady baseline window (`baselineEndSeconds`, B3.4) takes the operating point over that
 * window (N1.3, N2.2): a loop in Auto moves its plant input with the measured noise before a setpoint step, so the first change
 * of the input is not where the experiment begins.
 */
export function identifyPlantFromExcitation(samples: readonly IdentificationSample[], maximumNormalizedError: number,
  baselineEndSeconds?: number): ExcitationFit {
  if (samples.length < 20 || !samples.every((sample, index) => sample.quality === "good" && Number.isFinite(sample.timeSeconds)
      && Number.isFinite(sample.input) && Number.isFinite(sample.output) && (index === 0 || sample.timeSeconds > samples[index - 1].timeSeconds))) {
    throw new Error("Identification requires at least 20 good, finite, time-ordered samples.");
  }
  // Task jitter is normal; an interval longer than one and a half median spacings is a gap, refused with its time.
  const times = samples.map(sample => sample.timeSeconds);
  const step = medianSpacing(times);
  const gap = firstGap(times, step);
  if (gap) throw new Error(`The recording has ${gapPhrase(gap)}.`);
  const unit = samples[0].outputUnit ?? "";
  const afterBaseline = (series: readonly IdentificationSample[]) => baselineEndSeconds === undefined ? firstInputChange(series)
    : series.findIndex(sample => sample.timeSeconds > baselineEndSeconds + 1e-9);
  const rawChange = afterBaseline(samples);
  const noise = baselineNoiseRms(samples.slice(0, rawChange > 0 ? rawChange : samples.length).map(sample => sample.output));
  // The fit keeps the recorded instants. Blocks never straddle the first input change, so the steady baseline stays exact.
  const fitted = blockAveraged(samples, MAXIMUM_FIT_SAMPLES, rawChange > 0 ? [samples[rawChange].timeSeconds] : []);
  const first = afterBaseline(fitted);
  if (first < 3) throw new Error("A steady baseline of at least three samples before the first input change is required.");
  const fittedTimes = fitted.map(sample => sample.timeSeconds);
  const dt = medianSpacing(fittedTimes);
  const input0 = mean(fitted.slice(0, first).map(sample => sample.input));
  const output0 = mean(fitted.slice(0, first).map(sample => sample.output));
  const deviation = fitted.map(sample => sample.input - input0), output = fitted.map(sample => sample.output - output0);
  const scale = Math.max(1e-12, Math.max(...fitted.map(sample => sample.output)) - Math.min(...fitted.map(sample => sample.output)));
  const horizon = fittedTimes[fittedTimes.length - 1] - fittedTimes[first];
  const fit = (kind: SimpleKind, tau: number | undefined, delay: number): Candidate | undefined => {
    const basis = heldUnitResponse(fittedTimes, deviation, shapeOf(kind, tau, delay));
    const { gain } = leastSquaresGain(output, 0, basis);
    if (!Number.isFinite(gain)) return undefined;
    const error = Math.sqrt(output.reduce((sum, value, k) => sum + (value - gain * basis[k]) ** 2, 0) / output.length) / scale;
    return { kind, gain, ...(tau === undefined ? {} : { tau }), delay, error, basis };
  };
  const better = (left: Candidate | undefined, right: Candidate | undefined) => !left || right && right.error < left.error ? right : left;
  const ratio = Math.pow(Math.max(dt, horizon) / dt, 1 / 44);
  const lag = (kind: "first_order" | "first_order_dead_time", delays: readonly number[]): Candidate | undefined => {
    let best: Candidate | undefined;
    for (const delay of delays) for (const tau of logarithmicGrid(dt, Math.max(dt, horizon), 45)) best = better(best, fit(kind, tau, delay));
    const coarse = best;
    if (coarse?.tau === undefined) return coarse;
    // Refine the time constant between its coarse neighbours, then the delay at that time constant.
    for (const tau of logarithmicGrid(coarse.tau / ratio, coarse.tau * ratio, 41)) best = better(best, fit(kind, tau, coarse.delay));
    const refined = best ?? coarse;
    for (const delay of delays.filter(candidate => Math.abs(candidate - refined.delay) <= 2 * step)) best = better(best, fit(kind, refined.tau, delay));
    return best;
  };
  const maximumSteps = Math.max(1, Math.round(Math.min(horizon * 0.3, 5) / step));
  const candidates = [fit("static_gain", undefined, 0), fit("integrating", undefined, 0), lag("first_order", [0]),
    lag("first_order_dead_time", Array.from({ length: maximumSteps }, (_, index) => (index + 1) * step))]
    .filter((candidate): candidate is Candidate => candidate !== undefined && Number.isFinite(candidate.gain));
  const models = candidates.map(identifiedModel);
  const adequate = candidates.filter(candidate => candidate.error <= maximumNormalizedError)
    .sort((left, right) => COMPLEXITY[left.kind] - COMPLEXITY[right.kind] || left.error - right.error);
  if (!adequate.length) {
    const best = [...candidates].sort((left, right) => left.error - right.error)[0];
    throw new Error(`No simple model explains the recorded excitation${best ? ` (best ${best.kind.replace(/_/g, " ")}: normalized error ${best.error.toFixed(3)})` : ""}.`);
  }
  const selected = adequate[0];
  const predicted = selected.basis.map(value => output0 + selected.gain * value);
  const refusal = noiseRefusal(Math.max(...predicted) - Math.min(...predicted), noise, unit, "excitation");
  if (refusal) throw new Error(refusal);
  return { recommended: identifiedModel(selected), models, input0, output0,
    points: fitted.map((sample, k) => ({ timeSeconds: sample.timeSeconds, measured: sample.output, predicted: predicted[k],
      residual: sample.output - predicted[k] })) };
}

function shapeOf(kind: SimpleKind, tau: number | undefined, delay: number): HeldShape {
  if (kind === "static_gain") return { kind };
  if (kind === "integrating") return { kind, delaySeconds: delay };
  return { kind: "first_order", timeConstantSeconds: tau ?? 1, delaySeconds: delay };
}

/** Index of the first sample whose input differs from the one before it, or -1 when the input never changes. */
function firstInputChange(samples: readonly IdentificationSample[]): number {
  const tolerance = 1e-9 * Math.max(1, ...samples.map(sample => Math.abs(sample.input)));
  return samples.findIndex((sample, index) => index > 0 && Math.abs(sample.input - samples[index - 1].input) > tolerance);
}

function identifiedModel(candidate: Candidate): IdentifiedModel {
  return { kind: candidate.kind, gain: candidate.gain,
    ...(candidate.tau === undefined ? {} : { timeConstantSeconds: candidate.tau }),
    ...(candidate.kind === "first_order_dead_time" ? { deadTimeSeconds: candidate.delay } : {}),
    normalizedError: candidate.error, normalizedEstimationError: candidate.error, stable: candidate.kind !== "integrating" };
}

function logarithmicGrid(minimum: number, maximum: number, count: number): number[] {
  if (maximum <= minimum) return [minimum];
  return Array.from({ length: count }, (_, index) => minimum * Math.pow(maximum / minimum, index / (count - 1)));
}

function mean(values: readonly number[]): number { return values.reduce((sum, value) => sum + value, 0) / values.length; }
