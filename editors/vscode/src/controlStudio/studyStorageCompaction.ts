/**
 * Workspace storage keeps each recorded run once and stores its samples in columns.
 * Expansion restores exactly the in-memory evidence that was compacted; anything that
 * does not match the uniform recorded-run shape is stored unchanged.
 */
const COLUMNS = "trust.controlStudio.columns.v1";
const SHARED_RUN = "trust.controlStudio.sharedRun.v1";

type Sample = { timeSeconds: number; sequence: number; values: Record<string, { value: unknown; quality: unknown }> };

export function compactStoredEvidence(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(compactStoredEvidence);
  if (!record(value)) return value;
  const compact = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, compactStoredEvidence(item)]));
  const channels = channelIds(value.channels);
  if (channels && uniformSamples(value.samples, channels)) compact.samples = columns(value.samples, channels);
  // A verification result's evaluation repeats its run; keep one copy.
  if (record(value.evaluation) && record(value.run) && sameEvidence(value.evaluation.run, value.run)) {
    compact.evaluation = { ...(compact.evaluation as Record<string, unknown>), run: SHARED_RUN };
  }
  return compact;
}

export function expandStoredEvidence(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(expandStoredEvidence);
  if (!record(value)) return value;
  const expanded = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandStoredEvidence(item)]));
  const channels = channelIds(value.channels);
  if (channels && record(value.samples) && value.samples.format === COLUMNS) expanded.samples = rows(value.samples, channels);
  if (record(expanded.evaluation) && expanded.evaluation.run === SHARED_RUN) {
    expanded.evaluation = { ...expanded.evaluation, run: expanded.run };
  }
  return expanded;
}

function columns(samples: Sample[], channels: string[]): Record<string, unknown> {
  const qualities = Object.fromEntries(channels.flatMap(id => {
    const column = samples.map(sample => sample.values[id].quality);
    return column.every(quality => quality === "good") ? [] : [[id, column]];
  }));
  return { format: COLUMNS, timeSeconds: samples.map(sample => sample.timeSeconds), sequence: samples.map(sample => sample.sequence),
    values: Object.fromEntries(channels.map(id => [id, samples.map(sample => sample.values[id].value)])), qualities };
}

function rows(stored: Record<string, unknown>, channels: string[]): Sample[] | Record<string, unknown> {
  const { timeSeconds, sequence, values, qualities } = stored;
  if (!Array.isArray(timeSeconds) || !Array.isArray(sequence) || !record(values) || !record(qualities)) return stored;
  return timeSeconds.map((time, index) => ({ timeSeconds: time as number, sequence: sequence[index] as number,
    values: Object.fromEntries(channels.map(id => [id, { value: (values[id] as unknown[])[index],
      quality: Array.isArray(qualities[id]) ? (qualities[id] as unknown[])[index] : "good" }])) }));
}

function channelIds(value: unknown): string[] | undefined {
  if (!Array.isArray(value) || value.length === 0) return undefined;
  const ids = value.map(channel => record(channel) ? channel.id : undefined);
  return ids.every((id): id is string => typeof id === "string") && new Set(ids).size === ids.length ? ids : undefined;
}

function uniformSamples(value: unknown, channels: string[]): value is Sample[] {
  return Array.isArray(value) && value.length > 0 && value.every(sample => record(sample) && Object.keys(sample).length === 3
    && typeof sample.timeSeconds === "number" && typeof sample.sequence === "number" && record(sample.values)
    && Object.keys(sample.values).length === channels.length && channels.every(id => {
      const entry = (sample.values as Record<string, unknown>)[id];
      return record(entry) && Object.keys(entry).length === 2 && "value" in entry && "quality" in entry;
    }));
}

function sameEvidence(left: unknown, right: unknown): boolean {
  return left === right || JSON.stringify(left) === JSON.stringify(right);
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
