import type { IdentificationSample } from "./identification";
import { lowOrderStepResponse } from "./numerics/identification";
import { gainBounds, lattice, type StepResponseAnalysis } from "./stepResponseIdentification";

export interface SensorLagStepAnalysis extends Omit<StepResponseAnalysis, "best" | "consistency"> {
  zeroDelayLowerBound: boolean;
  best: StepResponseAnalysis["best"] & { secondTimeConstantSeconds: number };
  consistency: StepResponseAnalysis["consistency"] & { secondTimeConstantSeconds?: { minimum: number; maximum: number } };
}

const EPSILON = 1e-9;
const MAX_WORK = 500_000_000;
const tooLarge = () => new Error("Sensor-lag search is too large; narrow the justified bounds or increase the declared grid spacing.");

/** Adds the physical sensor balance to the existing qualified held-step calculation. */
export function fitSensorLagStep(samples: readonly IdentificationSample[], reference: StepResponseAnalysis): SensorLagStepAnalysis {
  const settings = reference.settings;
  const sensor = settings.sensorTimeConstant ?? { minimum: .1, maximum: 10, step: .1 };
  const hold = samples.filter(sample => sample.timeSeconds >= reference.stepTimeSeconds && sample.timeSeconds < reference.holdEndSeconds);
  const size = (grid: typeof sensor) => Math.round((grid.maximum - grid.minimum) / grid.step) + 1;
  const shapes = size(settings.delay) * size(settings.timeConstant) * size(sensor);
  const baselineOrigin = reference.baseline.mean - settings.measurementBound;
  const baselineCount = Math.floor((2 * settings.measurementBound + EPSILON) / settings.baselineStep) + 1;
  if (!Number.isFinite(shapes) || shapes > 1_000_000 || shapes * hold.length > MAX_WORK
    || baselineCount > 100_000 || shapes * baselineCount > 20_000_000) throw tooLarge();
  const gainRange = gainBounds(reference.baseline, reference.final, reference.inputChange, settings.measurementBound);
  const gains = lattice(gainRange.minimum, gainRange.maximum, settings.gainStep);
  const baselines = lattice(baselineOrigin, reference.baseline.mean + settings.measurementBound, settings.baselineStep);
  const delays = lattice(settings.delay.minimum, settings.delay.maximum, settings.delay.step);
  const taus = lattice(settings.timeConstant.minimum, settings.timeConstant.maximum, settings.timeConstant.step);
  const sensors = lattice(sensor.minimum, sensor.maximum, sensor.step);
  if (shapes * gains.length > MAX_WORK) throw tooLarge();
  const validBaseline = samples.filter(sample => sample.timeSeconds >= reference.baseline.startSeconds && sample.timeSeconds < reference.stepTimeSeconds);
  const baselineLower = Math.max(baselineOrigin, ...validBaseline.map(sample => sample.output - settings.measurementBound));
  const baselineUpper = Math.min(reference.baseline.mean + settings.measurementBound, ...validBaseline.map(sample => sample.output + settings.measurementBound));
  const response = new Float64Array(hold.length);
  const centered = hold.map(sample => sample.output - reference.baseline.mean);
  const sumY = centered.reduce((sum, value) => sum + value, 0);
  const sumY2 = centered.reduce((sum, value) => sum + value * value, 0);
  let best: SensorLagStepAnalysis["best"] | undefined;
  const consistency: SensorLagStepAnalysis["consistency"] = { count: 0 };
  let work = 0;

  for (const delaySeconds of delays) for (const timeConstantSeconds of taus) for (const secondTimeConstantSeconds of sensors) {
    let sumPhi = 0, sumPhi2 = 0, sumPhiY = 0;
    for (let i = 0; i < hold.length; i += 1) {
      const phi = reference.inputChange * lowOrderStepResponse("second_order_dead_time",
        hold[i].timeSeconds - reference.stepTimeSeconds + settings.timestampShiftSeconds,
        { tau1: timeConstantSeconds, tau2: secondTimeConstantSeconds, delay: delaySeconds });
      response[i] = phi;
      sumPhi += phi; sumPhi2 += phi * phi; sumPhiY += phi * centered[i];
    }
    work += hold.length;
    if (work > MAX_WORK) throw tooLarge();
    let shapeError = Number.POSITIVE_INFINITY;
    for (const baseline of baselines) {
      // For each baseline grid point, the gain objective is a convex quadratic.
      // Its closest two allowed grid points contain the exact discrete minimum.
      const endpoints = [(reference.final.mean - settings.measurementBound - baseline) / reference.inputChange,
        (reference.final.mean + settings.measurementBound - baseline) / reference.inputChange];
      const first = Math.max(0, Math.ceil((Math.min(...endpoints) - gains[0] - EPSILON) / settings.gainStep));
      const last = Math.min(gains.length - 1, Math.floor((Math.max(...endpoints) - gains[0] + EPSILON) / settings.gainStep));
      if (first > last) continue;
      const offset = baseline - reference.baseline.mean;
      const optimum = sumPhi2 > 0 ? (sumPhiY - offset * sumPhi) / sumPhi2 : gains[first];
      const nearest = (optimum - gains[0]) / settings.gainStep;
      for (const index of [Math.max(first, Math.min(last, Math.floor(nearest))), Math.max(first, Math.min(last, Math.ceil(nearest)))]) {
        const gain = gains[index];
        const squaredError = Math.max(0, sumY2 - 2 * offset * sumY - 2 * gain * sumPhiY
          + hold.length * offset * offset + 2 * offset * gain * sumPhi + gain * gain * sumPhi2);
        shapeError = Math.min(shapeError, squaredError);
        if (!best || squaredError < best.squaredError - 1e-12) {
          best = { baseline, gain, delaySeconds, timeConstantSeconds, secondTimeConstantSeconds, squaredError };
        }
      }
    }
    // No point can satisfy every residual bound when even the grid's least SSE
    // exceeds N*bound^2. This discards no measurement-consistent grid point.
    if (shapeError > hold.length * settings.measurementBound ** 2 + EPSILON) continue;
    for (const gain of gains) {
      let lower = Math.max(baselineLower, reference.final.mean - settings.measurementBound - gain * reference.inputChange);
      let upper = Math.min(baselineUpper, reference.final.mean + settings.measurementBound - gain * reference.inputChange);
      for (let i = 0; i < hold.length && lower <= upper + EPSILON; i += 1) {
        lower = Math.max(lower, hold[i].output - settings.measurementBound - gain * response[i]);
        upper = Math.min(upper, hold[i].output + settings.measurementBound - gain * response[i]);
        work += 1;
      }
      if (work > MAX_WORK) throw tooLarge();
      const first = Math.max(0, Math.ceil((lower - baselineOrigin - EPSILON) / settings.baselineStep));
      const last = Math.min(baselines.length - 1, Math.floor((upper - baselineOrigin + EPSILON) / settings.baselineStep));
      if (first > last) continue;
      consistency.count += last - first + 1;
      include(consistency, "gain", gain); include(consistency, "delaySeconds", delaySeconds);
      include(consistency, "timeConstantSeconds", timeConstantSeconds);
      include(consistency, "secondTimeConstantSeconds", secondTimeConstantSeconds);
      include(consistency, "baseline", baselines[first]); include(consistency, "baseline", baselines[last]);
    }
  }
  if (!best) throw new Error("The baseline/gain grid has no candidate within the qualified windows; refine its spacing.");
  const fitted = best;
  const points = hold.map(sample => {
    const predicted = fitted.baseline + reference.inputChange * fitted.gain * lowOrderStepResponse("second_order_dead_time",
      sample.timeSeconds - reference.stepTimeSeconds + settings.timestampShiftSeconds,
      { tau1: fitted.timeConstantSeconds, tau2: fitted.secondTimeConstantSeconds, delay: fitted.delaySeconds });
    return { timeSeconds: sample.timeSeconds, measured: sample.output, predicted, residual: sample.output - predicted };
  });
  fitted.squaredError = points.reduce((sum, point) => sum + point.residual ** 2, 0);
  const onBoundary = (value: number, grid: typeof sensor, zeroAllowed = false) =>
    !(zeroAllowed && value === 0) && (Math.abs(value - grid.maximum) < EPSILON || Math.abs(value - grid.minimum) < EPSILON);
  const amplitudeBoundary = Math.abs(fitted.baseline - baselines[0]) < EPSILON || Math.abs(fitted.baseline - baselines[baselines.length - 1]) < EPSILON
    || Math.abs(fitted.gain - gains[0]) < EPSILON || Math.abs(fitted.gain - gains[gains.length - 1]) < EPSILON;
  const boundaryLimited = amplitudeBoundary || onBoundary(fitted.delaySeconds, settings.delay, true)
    || onBoundary(fitted.timeConstantSeconds, settings.timeConstant) || onBoundary(fitted.secondTimeConstantSeconds, sensor);
  const warnings = ["Additional lag is a hypothesis; the recording alone cannot assign the two time constants to the process and the sensor."];
  if (boundaryLimited) warnings.push("The sensor-lag fit reaches a search boundary; inspect or extend the justified range.");
  if (amplitudeBoundary) warnings.push("The fitted baseline or gain reaches its grid boundary; review window uncertainty and grid resolution.");
  if (!consistency.count) warnings.push("No sensor-lag grid point satisfies the declared measurement bound; model adequacy does not replace measurement consistency.");
  if (settings.timestampShiftSeconds !== 0) warnings.push("The timestamp shift is a sensitivity study, not nominal model evidence.");
  return { ...structuredClone(reference), settings: { ...structuredClone(settings), sensorTimeConstant: { ...sensor } },
    best: fitted, consistency, boundaryLimited, zeroDelayLowerBound: fitted.delaySeconds === 0 && settings.delay.minimum === 0, warnings, points };
}

function include(set: SensorLagStepAnalysis["consistency"], key: Exclude<keyof SensorLagStepAnalysis["consistency"], "count">, value: number): void {
  const previous = set[key];
  set[key] = { minimum: Math.min(previous?.minimum ?? value, value), maximum: Math.max(previous?.maximum ?? value, value) };
}
