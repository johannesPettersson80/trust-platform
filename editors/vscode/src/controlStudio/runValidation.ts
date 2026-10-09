import { canonicalJson } from "./canonicalJson";
import { ISOLATED_SIMULATION_SAMPLE_LIMIT } from "./controlStudioLimits";
import { firstGap, gapPhrase, medianSpacing } from "./evenSampling";
import { steadyRecordPeriod } from "./recordSequence";
import { largest, rootSumSquare, smallest } from "./sampleStatistics";

/** Generic observed-run acceptance, independent of controller structure and transport. */
export type RunEvidence = "model-predicted" | "ST-executed" | "physical-recorded";
export type RequirementStatus = "Pass" | "Fail" | "Inconclusive";
export interface RunConditions {
  initialState: Readonly<Record<string, number>>;
  events: readonly { atSeconds: number; channel: string; value: number }[];
  durationSeconds: number;
  sampleTimesSeconds: readonly number[];
  seed: number;
  modelRevision: string;
  programRevision: string;
  parameterSchemaRevision: string;
}
export interface RecordedRun {
  id: string;
  evidence: RunEvidence;
  parameters: Readonly<Record<string, number>>;
  conditions: RunConditions;
  channels: readonly { id: string; unit: string }[];
  samples: readonly { timeSeconds: number; sequence: number;
    values: Readonly<Record<string, { value: number | null; quality: "good" | "bad" | "forced" | "missing" }>> }[];
}
interface RequirementBase { id: string; channel: string; window: { start: number; end: number } }
export type RunRequirement = RequirementBase & (
  | { kind: "error"; reference: number | { channel: string }; statistic: "maximum" | "mean" | "rms"; maximum: number }
  | { kind: "overshoot"; initial: number; target: number; maximum: number }
  | { kind: "settling"; target: number; band: number; holdSeconds: number; deadlineSeconds: number }
  | { kind: "bounds" | "state-bounds"; minimum: number; maximum: number }
  | { kind: "levels"; allowed: readonly number[]; tolerance: number }
  | { kind: "dwell"; threshold: number; state: "high" | "low"; minimumSeconds: number }
  | { kind: "rate"; maximum: number; direction?: "up" | "down" }
  | { kind: "saturation"; minimum: number; maximum: number; maximumSeconds: number }
  | { kind: "switching"; threshold: number; maximum: number }
  | { kind: "activity"; maximum?: number });
export interface RunRequirementResult {
  id: string; kind: RunRequirement["kind"]; status: RequirementStatus;
  value?: number; limit?: number; unit: string; reason: string;
}
export interface RunEvaluation {
  status: RequirementStatus;
  run: RecordedRun;
  specification: readonly RunRequirement[];
  requirements: readonly RunRequirementResult[];
  scope: string;
}
export interface RunComparison {
  status: RequirementStatus;
  current: RunEvaluation;
  candidate: RunEvaluation;
  preferred?: "current" | "candidate" | "tie";
  reason: string;
}

export function evaluateRecordedRun(input: unknown, requirements: readonly unknown[]): RunEvaluation {
  const run = decodeRun(input);
  const specification = requirements.map(value => decodeRunRequirement(value, run));
  if (new Set(specification.map(r => r.id)).size !== specification.length) throw new Error("Requirement IDs must be unique.");
  const results = specification.map(requirement => evaluateObservedRequirement(run, requirement));
  return { status: aggregate(results.map(r => r.status)), run, specification, requirements: results,
    scope: "Only the declared observed conditions were evaluated; Pass does not establish general stability." };
}

export function compareRecordedRuns(currentInput: unknown, candidateInput: unknown, requirements: readonly unknown[], objective?: string): RunComparison {
  const current = evaluateRecordedRun(currentInput, requirements);
  const candidate = evaluateRecordedRun(candidateInput, requirements);
  if (current.run.evidence !== candidate.run.evidence || canonicalJson(current.run.conditions) !== canonicalJson(candidate.run.conditions)
      || canonicalJson(current.run.channels) !== canonicalJson(candidate.run.channels)
      || canonicalJson(Object.keys(current.run.parameters).sort()) !== canonicalJson(Object.keys(candidate.run.parameters).sort())) {
    return { status: "Inconclusive", current, candidate, reason: "Evidence class, channel/parameter identities or immutable experiment conditions differ." };
  }
  const result: RunComparison = { status: aggregate([current.status, candidate.status]), current, candidate,
    reason: "No ranking objective was declared." };
  if (objective === undefined) return result;
  const a = current.requirements.find(r => r.id === objective);
  const b = candidate.requirements.find(r => r.id === objective);
  if (!a || !b) throw new Error("Ranking objective must name a declared requirement.");
  if (current.status !== "Pass" && candidate.status !== "Pass") return { ...result, reason: "Neither run satisfies all requirements; no ranking is available." };
  if (current.status !== "Pass") return { ...result, preferred: "candidate", reason: "Only candidate satisfies all requirements." };
  if (candidate.status !== "Pass") return { ...result, preferred: "current", reason: "Only current satisfies all requirements." };
  if (a.value === undefined || b.value === undefined) return { ...result, status: "Inconclusive", reason: "Ranking measurement is unavailable." };
  return { ...result, preferred: a.value === b.value ? "tie" : a.value < b.value ? "current" : "candidate",
    reason: `Both runs qualify; minimizing ${objective}.` };
}

export function evaluateObservedRequirement(run: Pick<RecordedRun, "channels" | "samples"> & { conditions: Pick<RunConditions, "sampleTimesSeconds"> }, requirement: RunRequirement): RunRequirementResult {
  const channel = run.channels.find(c => c.id === requirement.channel)!;
  let unit = channel.unit;
  if (["settling", "saturation", "dwell"].includes(requirement.kind)) unit = "s";
  if (requirement.kind === "switching") unit = "count";
  // Movement of a percentage command is a percentage-point change.
  if (requirement.kind === "activity" && unit === "%") unit = "pp";
  if (requirement.kind === "rate") unit += "/s";
  const declaredLimit = requirement.kind === "settling" ? requirement.deadlineSeconds
    : requirement.kind === "saturation" ? requirement.maximumSeconds
    : requirement.kind === "levels" ? requirement.tolerance : requirement.kind === "dwell" ? requirement.minimumSeconds
    : requirement.kind === "bounds" || requirement.kind === "state-bounds" ? 0 : requirement.maximum;
  const base = { id: requirement.id, kind: requirement.kind, unit, ...(declaredLimit === undefined ? {} : { limit: declaredLimit }) };
  const expected = run.conditions.sampleTimesSeconds.filter(t => t >= requirement.window.start && t <= requirement.window.end);
  const referenceChannel = requirement.kind === "error" && typeof requirement.reference !== "number" ? requirement.reference.channel : undefined;
  const needed = referenceChannel ? [requirement.channel, referenceChannel] : [requirement.channel];
  const coverage = heldOnGrid(run.samples, expected, needed, requirement.window,
    ((run.conditions as { recordEvery?: number }).recordEvery ?? 1) > 1);
  if ("reason" in coverage) return { ...base, status: "Inconclusive", reason: coverage.reason };
  const samples = coverage.samples;
  const values = samples.map(s => s.values[requirement.channel].value as number);
  let value: number; let limit: number;
  switch (requirement.kind) {
    case "error": {
      const reference = requirement.reference;
      const errors = values.map((v, i) => Math.abs(v - (typeof reference === "number" ? reference : samples[i].values[reference.channel].value as number)));
      value = requirement.statistic === "maximum" ? largest(errors) : requirement.statistic === "rms"
        ? rootSumSquare(errors) / Math.sqrt(errors.length) : errors.reduce((a, b) => a + b, 0) / errors.length;
      limit = requirement.maximum; break;
    }
    case "overshoot":
      value = largest(values.map(v => Math.sign(requirement.target - requirement.initial) * (v - requirement.target)), 0);
      limit = requirement.maximum; break;
    case "bounds": case "state-bounds":
      value = largest(values.map(v => Math.max(requirement.minimum - v, v - requirement.maximum)), 0);
      limit = 0; break;
    case "levels":
      value = largest(values.map(value => Math.min(...requirement.allowed.map(level => Math.abs(value - level)))), 0);
      limit = requirement.tolerance; break;
    case "dwell": {
      const intervals: number[] = [];
      let transitionStart: number | undefined;
      let high = values[0] >= requirement.threshold;
      for (let index = 1; index < values.length; index++) {
        const nextHigh = values[index] >= requirement.threshold;
        if (nextHigh === high) continue;
        if (transitionStart !== undefined && high === (requirement.state === "high")) intervals.push(samples[index].timeSeconds - transitionStart);
        transitionStart = samples[index].timeSeconds; high = nextHigh;
      }
      if (!intervals.length) return { ...base, status: "Inconclusive", reason: `No complete ${requirement.state} dwell interval was observed; extend the switching scenario.` };
      const shortest = smallest(intervals);
      const meetsMinimum = shortest + 1e-9 * Math.max(1, requirement.minimumSeconds) >= requirement.minimumSeconds;
      return { ...base, status: meetsMinimum ? "Pass" : "Fail", value: shortest, limit: requirement.minimumSeconds,
        reason: meetsMinimum ? "Every completed observed dwell meets the declared minimum." : "An observed switch occurred before the declared minimum dwell." };
    }
    case "rate":
      value = largest(values.slice(1).map((v, i) => (requirement.direction === "up" ? v - values[i] : requirement.direction === "down" ? values[i] - v : Math.abs(v - values[i])) / (samples[i + 1].timeSeconds - samples[i].timeSeconds)), 0);
      limit = requirement.maximum; break;
    case "switching":
      value = values.slice(1).filter((v, i) => Math.abs(v - values[i]) > requirement.threshold).length;
      limit = requirement.maximum; break;
    case "activity": {
      const movement = values.slice(1).reduce((sum, v, i) => sum + Math.abs(v - values[i]), 0);
      if (requirement.maximum === undefined) return { ...base, status: finite(movement) ? "Pass" : "Inconclusive", value: movement,
        reason: finite(movement) ? "Measured total command movement." : "The calculated measurement is nonfinite." };
      value = movement; limit = requirement.maximum; break;
    }
    case "saturation":
      value = values.slice(0, -1).reduce((sum, v, i) => sum + (v <= requirement.minimum || v >= requirement.maximum
        ? samples[i + 1].timeSeconds - samples[i].timeSeconds : 0), 0);
      limit = requirement.maximumSeconds; break;
    case "settling": {
      const arrival = samples.findIndex((s, i) => requirement.window.end - s.timeSeconds >= requirement.holdSeconds
        && values.slice(i).every(v => Math.abs(v - requirement.target) <= requirement.band));
      if (arrival < 0) return { ...base, status: "Fail", limit: requirement.deadlineSeconds,
        reason: "No observed arrival remained in band for the required hold through the end of the window." };
      value = samples[arrival].timeSeconds - requirement.window.start;
      limit = requirement.deadlineSeconds; break;
    }
  }
  if (!finite(value)) return { ...base, status: "Inconclusive", reason: "The calculated measurement is nonfinite." };
  return { ...base, status: value <= limit ? "Pass" : "Fail", value, limit,
    reason: value <= limit ? "Observed measurement satisfies the declared limit." : "Observed measurement exceeds the declared limit." };
}

type RunScan = RecordedRun["samples"][number];
const INCOMPLETE_COVERAGE = "The declared window lacks complete, ordered, good-quality sample coverage.";

/** The declared sample times of a window, each holding the last observed scan at or before it. Normal task jitter is accepted: the
 * scans must reach beyond both window ends, be ordered, contiguous and good, and leave no interval longer than one and a half steps. */
function heldOnGrid(scans: readonly RunScan[], grid: readonly number[], needed: readonly string[], range: { start: number; end: number },
  averaged: boolean):
  { samples: RunScan[] } | { reason: string } {
  let lower = -1;
  for (let index = 0; index < scans.length && scans[index].timeSeconds <= range.start + 1e-9; index += 1) lower = index;
  let upper = scans.findIndex(scan => scan.timeSeconds >= range.end - 1e-9);
  // A sample recorded at a record period sits at the mean time of its executions (E4.4): the first one covers the run's
  // beginning and the last one its end, so a window that starts before the first or ends after the last is still covered.
  if (averaged && lower < 0 && scans.length > 0) lower = 0;
  if (averaged && upper < 0 && scans.length > 0) upper = scans.length - 1;
  if (grid.length === 0 || lower < 0 || upper < 0) return { reason: INCOMPLETE_COVERAGE };
  const span = scans.slice(lower, upper + 1);
  if (span.some((scan, index) => !Number.isSafeInteger(scan.sequence) || index > 0
      && !(scan.timeSeconds > span[index - 1].timeSeconds))
    || !steadyRecordPeriod(span.map(scan => scan.sequence), upper === scans.length - 1)
    || span.some(scan => !needed.every(id => scan.values[id]?.quality === "good" && finite(scan.values[id]?.value)))) {
    return { reason: INCOMPLETE_COVERAGE };
  }
  const gap = firstGap(span.map(scan => scan.timeSeconds), medianSpacing(grid));
  if (gap) return { reason: `The recording has ${gapPhrase(gap)}.` };
  let held = lower;
  return { samples: grid.map(timeSeconds => {
    while (held < upper && scans[held + 1].timeSeconds <= timeSeconds + 1e-9) held += 1;
    return { timeSeconds, sequence: scans[held].sequence, values: scans[held].values };
  }) };
}

function decodeRun(value: unknown): RecordedRun {
  if (!record(value) || !text(value.id) || !["model-predicted", "ST-executed", "physical-recorded"].includes(String(value.evidence))
      || !numbers(value.parameters) || !record(value.conditions) || !Array.isArray(value.channels) || !Array.isArray(value.samples)) throw new Error("Invalid recorded-run contract.");
  const c = value.conditions;
  if (!numbers(c.initialState) || !finite(c.durationSeconds) || c.durationSeconds <= 0 || !Number.isSafeInteger(c.seed)
      || !text(c.modelRevision) || !text(c.programRevision) || !text(c.parameterSchemaRevision)
      || !Array.isArray(c.sampleTimesSeconds) || c.sampleTimesSeconds.length < 2 || c.sampleTimesSeconds.length > ISOLATED_SIMULATION_SAMPLE_LIMIT
      // A run recorded at a record period k (E4.4) takes each sample at the mean time of its k executions, so only an
      // unaveraged run starts at 0.
      || c.recordEvery !== undefined && (!Number.isSafeInteger(c.recordEvery) || (c.recordEvery as number) < 2 || (c.recordEvery as number) > 100_000)
      || (c.recordEvery === undefined ? c.sampleTimesSeconds[0] !== 0 : !finite(c.sampleTimesSeconds[0]) || c.sampleTimesSeconds[0] < 0)
      || (c.recordEvery === undefined ? c.sampleTimesSeconds[c.sampleTimesSeconds.length - 1] !== c.durationSeconds
        : !(c.sampleTimesSeconds[c.sampleTimesSeconds.length - 1] <= (c.durationSeconds as number) + 1e-9))
      || !c.sampleTimesSeconds.every((t, i, all) => finite(t) && t >= 0 && (i === 0 || t > all[i - 1]))
      || !Array.isArray(c.events) || !c.events.every(e => record(e) && finite(e.atSeconds) && e.atSeconds >= 0 && e.atSeconds <= (c.durationSeconds as number)
        && text(e.channel) && finite(e.value))) throw new Error("Invalid immutable experiment conditions.");
  // The isolated run's channel maximum (runtime protocol annex, E8; specification 35, A7) and its sample maximum (A7; E8.3).
  if (value.channels.length === 0 || value.channels.length > 64 || !value.channels.every(c => record(c) && text(c.id) && text(c.unit))
      || new Set(value.channels.map(c => c.id)).size !== value.channels.length
      || value.samples.length > ISOLATED_SIMULATION_SAMPLE_LIMIT || !value.samples.every(s => record(s) && finite(s.timeSeconds) && Number.isSafeInteger(s.sequence) && record(s.values))) {
    throw new Error("Invalid run channels or sample structure.");
  }
  // Preserve NaN/undefined quality evidence; JSON cloning would silently coerce it.
  return clone(value) as unknown as RecordedRun;
}
export function decodeRunRequirement(value: unknown, run: Pick<RecordedRun, "channels">
  & { conditions: Pick<RunConditions, "sampleTimesSeconds"> & { durationSeconds?: number } }): RunRequirement {
  // A window lies within the run. A run recorded at a record period has its samples at the mean times of their executions (E4.4),
  // so a window's ends need not be sample times.
  const times = run.conditions.sampleTimesSeconds;
  const end = run.conditions.durationSeconds ?? times[times.length - 1];
  if (!record(value) || !text(value.id) || !text(value.channel) || !run.channels.some(c => c.id === value.channel)
      || !record(value.window) || !finite(value.window.start) || !finite(value.window.end) || value.window.start >= value.window.end
      || value.window.start < 0 || !(value.window.end <= end + 1e-9)) throw new Error("Invalid requirement identity, channel or sample window.");
  const nonnegative = (v: unknown) => finite(v) && v >= 0;
  let valid = false;
  switch (value.kind) {
    case "error": valid = nonnegative(value.maximum) && ["maximum", "mean", "rms"].includes(String(value.statistic))
      && (finite(value.reference) || (record(value.reference) && run.channels.some(c => c.id === (value.reference as Record<string, unknown>).channel
        && c.unit === run.channels.find(c => c.id === value.channel)?.unit))); break;
    case "overshoot": valid = finite(value.initial) && finite(value.target) && value.initial !== value.target && nonnegative(value.maximum); break;
    case "bounds": case "state-bounds": valid = finite(value.minimum) && finite(value.maximum) && value.minimum <= value.maximum; break;
    case "levels": valid = Array.isArray(value.allowed) && value.allowed.length > 0 && value.allowed.length <= 16 && value.allowed.every(finite) && nonnegative(value.tolerance); break;
    case "dwell": valid = finite(value.threshold) && ["high", "low"].includes(String(value.state)) && nonnegative(value.minimumSeconds); break;
    case "rate": valid = nonnegative(value.maximum) && (value.direction === undefined || value.direction === "up" || value.direction === "down"); break;
    case "switching": valid = nonnegative(value.threshold) && nonnegative(value.maximum) && Number.isSafeInteger(value.maximum); break;
    case "activity": valid = value.maximum === undefined || nonnegative(value.maximum); break;
    case "saturation": valid = finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum && nonnegative(value.maximumSeconds); break;
    case "settling": valid = finite(value.target) && nonnegative(value.band) && finite(value.holdSeconds) && value.holdSeconds > 0
      && value.holdSeconds <= value.window.end - value.window.start && nonnegative(value.deadlineSeconds)
      && (value.deadlineSeconds as number) + (value.holdSeconds as number) <= value.window.end - value.window.start; break;
  }
  if (!valid) throw new Error("Invalid requirement calculation or limits.");
  return clone(value) as unknown as RunRequirement;
}
function aggregate(statuses: readonly RequirementStatus[]): RequirementStatus {
  return statuses.includes("Fail") ? "Fail" : statuses.length === 0 || statuses.includes("Inconclusive") ? "Inconclusive" : "Pass";
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function numbers(value: unknown): value is Record<string, number> { return record(value) && Object.entries(value).every(([k, v]) => text(k) && finite(v)); }
function clone(value: unknown): unknown { return Array.isArray(value) ? Object.freeze(value.map(clone)) : record(value) ? Object.freeze(Object.fromEntries(Object.entries(value).map(([k, v]) => [k, clone(v)]))) : value; }
