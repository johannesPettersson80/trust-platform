import type { RuntimeVerificationResult } from "./verificationClient";
import { steadyRecordPeriod } from "./recordSequence";

type RecordedSample = RuntimeVerificationResult["run"]["samples"][number];
type RecordedValue = RecordedSample["values"][string];

/** A fit receives at most this many points; a longer recording is block-averaged down to it before fitting. */
export const MAXIMUM_FIT_SAMPLES = 5000;

/** The fields the identification analyses read from a sample on a time axis. */
export interface EvenSample {
  timeSeconds: number;
  input: number;
  output: number;
  quality: "good" | "uncertain" | "bad";
  clipped?: boolean;
  saturated?: boolean;
}

/** An interval longer than one and a half steps, which analyses that need continuous data refuse. */
export interface SampleGap { startSeconds: number; lengthSeconds: number }

export interface GridPoint { timeSeconds: number; index: number; fraction: number }

/** A recording on an even grid. A simulator's run already is one. A live plant's samples carry the times its task actually ran
 * (a 100 ms task in a 10 ms resource runs every 100 or 110 ms), so they are held onto a grid at their median spacing, or at
 * `step`, as a PLC holds a value until its next scan. Channels named in `linear` are interpolated between scans instead, which
 * suits a measured output. Missing scans and gaps longer than one and a half spacings are refused. */
export function evenlySampled(recorded: readonly RecordedSample[], step?: number, options: { linear?: readonly string[] } = {}):
  { samples: RecordedSample[]; step: number } {
  if (recorded.length < 30) throw new Error("Model validation requires at least 30 complete scans.");
  const times = recorded.map(sample => sample.timeSeconds);
  const spacing = step ?? medianSpacing(times);
  if (!(spacing > 0) || !steadyRecordPeriod(recorded.map(sample => sample.sequence), true)
    || times.some((time, i) => i > 0 && !(time > times[i - 1]))) throw new Error("The independent recording has missing or irregular scans.");
  const gap = firstGap(times, spacing);
  if (gap) throw new Error(`The independent recording has missing or irregular scans: ${gapPhrase(gap)}.`);
  if (recorded.every((sample, i) => Math.abs(sample.timeSeconds - recorded[0].timeSeconds - i * spacing) <= 1e-8)) {
    return { samples: [...recorded], step: spacing };
  }
  const linear = new Set(options.linear ?? []);
  const samples = evenGrid(times, spacing).map(({ timeSeconds, index, fraction }, k): RecordedSample => {
    const held = recorded[index];
    const next = fraction > 0 ? recorded[index + 1] : undefined;
    const values = Object.fromEntries(Object.entries(held.values).map(([channel, value]): [string, RecordedValue] => {
      const after = next?.values[channel];
      if (!after || !linear.has(channel) || value.value === null || after.value === null) return [channel, value];
      return [channel, { value: value.value + fraction * (after.value - value.value),
        quality: value.quality === "good" ? after.quality : value.quality }];
    }));
    return { ...held, timeSeconds, sequence: recorded[0].sequence + k, values };
  });
  if (samples.length < 30) throw new Error("Model validation requires at least 30 complete scans.");
  return { samples, step: spacing };
}

/** The median interval between consecutive times; zero when there are fewer than two. */
export function medianSpacing(times: readonly number[]): number {
  const intervals = times.slice(1).map((time, index) => time - times[index]).sort((left, right) => left - right);
  return intervals.length > 0 ? intervals[Math.floor(intervals.length / 2)] : 0;
}

/** The first interval longer than one and a half steps. Shorter intervals are normal task jitter. */
export function firstGap(times: readonly number[], step: number): SampleGap | undefined {
  for (let index = 1; index < times.length; index += 1) {
    const lengthSeconds = times[index] - times[index - 1];
    if (lengthSeconds > 1.5 * step + 1e-9) return { startSeconds: times[index - 1], lengthSeconds };
  }
  return undefined;
}

/** "a gap of 0.42 s at 12.3 s", for refusal messages. */
export function gapPhrase(gap: SampleGap): string {
  return `a gap of ${secondsText(gap.lengthSeconds)} s at ${secondsText(gap.startSeconds)} s`;
}

function secondsText(value: number): string {
  return String(Number(value.toFixed(3)));
}

/** Grid times anchored at the first sample and spaced `step` apart. Each holds the index of the last sample at or before it and
 * its fraction of the way to the next sample. */
export function evenGrid(times: readonly number[], step: number): GridPoint[] {
  const count = Math.floor((times[times.length - 1] - times[0]) / step + 1e-9) + 1;
  if (!(step > 0) || !Number.isFinite(count) || count > 4 * times.length + 16) {
    throw new Error("The recording cannot be placed on an even grid.");
  }
  const grid: GridPoint[] = [];
  let index = 0;
  for (let k = 0; k < count; k += 1) {
    const timeSeconds = times[0] + k * step;
    while (index + 1 < times.length && times[index + 1] <= timeSeconds + 1e-9) index += 1;
    const fraction = index + 1 < times.length
      ? Math.min(1, Math.max(0, (timeSeconds - times[index]) / (times[index + 1] - times[index]))) : 0;
    grid.push({ timeSeconds, index, fraction });
  }
  return grid;
}

/** Resamples a recording once onto its even grid, at `step` (default: its median spacing). Inputs are held from the last sample,
 * as a PLC holds a command between scans, and outputs are interpolated linearly between neighbouring samples. A grid point between
 * two samples is bad when either is bad, clipped or saturated. A recording already on its grid is copied unchanged. */
export function resampledEvenly<T extends EvenSample>(samples: readonly T[], step = medianSpacing(samples.map(sample => sample.timeSeconds))): T[] {
  const times = samples.map(sample => sample.timeSeconds);
  if (times.every((time, index) => index === 0 || Math.abs(time - times[index - 1] - step) <= 1e-6 * step)) {
    return samples.map(sample => ({ ...sample }));
  }
  return evenGrid(times, step).map(({ timeSeconds, index, fraction }) => {
    const held = samples[index];
    const next = fraction > 0 ? samples[index + 1] : undefined;
    return { ...held, timeSeconds, output: next ? held.output + fraction * (next.output - held.output) : held.output,
      ...combinedFlags(next ? [held, next] : [held]) };
  });
}

/** Block-averages a recording to at most `maximumPoints` points before a fit. A block takes the mean time, input and output of its
 * samples, and is bad when any member is bad, clipped or saturated. No block straddles a cut time, such as a step, so the measured
 * input change stays exact. */
export function blockAveraged<T extends EvenSample>(samples: readonly T[], maximumPoints = MAXIMUM_FIT_SAMPLES, cutsSeconds: readonly number[] = []): T[] {
  if (samples.length <= maximumPoints) return samples.map(sample => ({ ...sample }));
  const sideOf = (time: number) => cutsSeconds.filter(cut => cut <= time).length;
  const blockStarts = (size: number): number[] => {
    const starts: number[] = [];
    for (let start = 0; start < samples.length;) {
      starts.push(start);
      const side = sideOf(samples[start].timeSeconds);
      let end = start + 1;
      while (end < samples.length && end - start < size && sideOf(samples[end].timeSeconds) === side) end += 1;
      start = end;
    }
    return starts;
  };
  let size = Math.ceil(samples.length / maximumPoints);
  let starts = blockStarts(size);
  while (starts.length > maximumPoints && size < samples.length) {
    size += 1;
    starts = blockStarts(size);
  }
  return starts.map((start, block) => {
    const members = samples.slice(start, block + 1 < starts.length ? starts[block + 1] : samples.length);
    return { ...members[0], timeSeconds: mean(members.map(sample => sample.timeSeconds)),
      input: mean(members.map(sample => sample.input)), output: mean(members.map(sample => sample.output)), ...combinedFlags(members) };
  });
}

function combinedFlags<T extends EvenSample>(members: readonly T[]): { quality: EvenSample["quality"]; clipped: boolean; saturated: boolean } {
  const flagged = members.some(sample => sample.quality === "bad" || sample.clipped === true || sample.saturated === true);
  return { quality: flagged ? "bad" : members.some(sample => sample.quality === "uncertain") ? "uncertain" : "good",
    clipped: members.some(sample => sample.clipped === true), saturated: members.some(sample => sample.saturated === true) };
}

function mean(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}
