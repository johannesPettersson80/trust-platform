/**
 * The steadiness and settling rules of Specification 35 §B3.4, measured on process time (§A3.2). Pure functions: the campaign
 * passes process-time samples and acts on the decision, so no rule reads a clock.
 */

/** One measured value at a process time, in seconds. */
export interface TimedValue {
  readonly seconds: number;
  readonly value: number;
}

/** A window, and a settle window, hold at least this many samples (B3.4). */
export const MINIMUM_WINDOW_SAMPLES = 20;
/** The steady baseline window is never longer than 30 minutes (B3.4). */
export const BASELINE_WINDOW_CAP_SECONDS = 1_800;
/** The response has left the noise band beyond this many noise sigmas (B3.4). */
const NOISE_BAND_SIGMAS = 3;

export function mean(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

/** Noise σ: the root mean square of the first differences, sqrt(mean(diff²)/2) (B3.4). */
export function noiseSigma(values: readonly number[]): number {
  if (values.length < 2) return 0;
  let squares = 0;
  for (let index = 1; index < values.length; index += 1) {
    const step = values[index] - values[index - 1];
    squares += step * step;
  }
  return Math.sqrt(squares / (values.length - 1) / 2);
}

/** The least-squares slope of the values against their process times, in value per second. */
export function trendSlope(points: readonly TimedValue[]): number {
  if (points.length < 2) return 0;
  const meanSeconds = mean(points.map(point => point.seconds));
  const meanValue = mean(points.map(point => point.value));
  let covariance = 0;
  let variance = 0;
  for (const point of points) {
    covariance += (point.seconds - meanSeconds) * (point.value - meanValue);
    variance += (point.seconds - meanSeconds) ** 2;
  }
  return variance > 0 ? covariance / variance : 0;
}

/** How far the least-squares trend moves the value across the time span of the points. */
export function trendMovement(points: readonly TimedValue[]): number {
  if (points.length < 2) return 0;
  return Math.abs(trendSlope(points)) * (points[points.length - 1].seconds - points[0].seconds);
}

/** A baseline is steady when its trend moves it by no more than four times the noise, or 0.001 % of its range when that is larger
 * (B3.4). A tolerance tied to the range would let a short window call an output that still moves fast steady. */
export function isSteady(points: readonly TimedValue[], noise: number, range: number): boolean {
  return trendMovement(points) <= Math.max(4 * noise, 1e-5 * range);
}

/** The baseline window: the longest of 20 samples, 5 % of the maximum duration and, when declared, 10 % of the expected
 * settling time, capped at 30 minutes (B3.4). */
export function baselineWindowSeconds(maximumSeconds: number, spacingSeconds: number, settlingSeconds?: number): number {
  const declared = settlingSeconds === undefined ? 0 : 0.1 * settlingSeconds;
  return Math.min(BASELINE_WINDOW_CAP_SECONDS,
    Math.max(MINIMUM_WINDOW_SAMPLES * spacingSeconds, 0.05 * maximumSeconds, declared));
}

/** The settle window: the longer of 20 samples and 10 % of the hold so far (B3.4). */
export function settleWindowSeconds(holdSeconds: number, spacingSeconds: number): number {
  return Math.max(MINIMUM_WINDOW_SAMPLES * spacingSeconds, 0.1 * holdSeconds);
}

/** The points that lie within `seconds` of the last point, in time order. */
export function trailingWindow(points: readonly TimedValue[], seconds: number): TimedValue[] {
  if (points.length === 0) return [];
  const start = points[points.length - 1].seconds - seconds;
  let first = points.length - 1;
  while (first > 0 && points[first - 1].seconds >= start) first -= 1;
  return points.slice(first);
}

/** The median spacing of successive points; the nominal spacing when fewer than two points exist. */
export function medianSpacing(points: readonly TimedValue[], nominalSeconds: number): number {
  const gaps: number[] = [];
  for (let index = 1; index < points.length; index += 1) gaps.push(points[index].seconds - points[index - 1].seconds);
  if (gaps.length === 0) return nominalSeconds;
  gaps.sort((first, second) => first - second);
  const middle = Math.floor(gaps.length / 2);
  return gaps.length % 2 === 1 ? gaps[middle] : (gaps[middle - 1] + gaps[middle]) / 2;
}

/** A single value has left the noise band when it is more than three noise sigmas from the baseline mean (B3.4). */
export function leftNoiseBand(value: number, baselineMean: number, noise: number): boolean {
  return Math.abs(value - baselineMean) > NOISE_BAND_SIGMAS * noise;
}

/** The response has settled when, over the settle window, it changes by less than the larger of twice the noise and 2 % of
 * its change so far (B3.4). The caller has already seen it leave the noise band. */
export function hasSettled(points: readonly TimedValue[],
  baseline: { readonly mean: number; readonly noise: number; readonly spacingSeconds: number }, stepSeconds: number): boolean {
  if (points.length === 0) return false;
  const now = points[points.length - 1].seconds;
  const recent = trailingWindow(points, settleWindowSeconds(now - stepSeconds, baseline.spacingSeconds));
  if (recent.length < MINIMUM_WINDOW_SAMPLES) return false;
  const change = trendMovement(recent);
  const soFar = Math.abs(mean(recent.map(point => point.value)) - baseline.mean);
  return change < Math.max(2 * baseline.noise, 0.02 * soFar);
}

/** The ratio of process time to wall time over one observation, or undefined until a second of wall time has passed. */
export function observedTimeScale(processSeconds: number, wallMilliseconds: number): number | undefined {
  return wallMilliseconds >= 1_000 && processSeconds > 0 ? processSeconds / (wallMilliseconds / 1_000) : undefined;
}
