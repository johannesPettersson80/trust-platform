// The runtime's compact RecordedRun samples (specification 35, A7) and their expansion into the
// per-channel {value, quality} form this extension keeps internally. A sample's `values` holds one
// number or null per channel, in the run's `channels` order. Its `quality` holds one code per
// channel, an index into QUALITIES, and is absent when every channel is good.

/** The quality vocabulary of the wire format, in code order (the runtime's QUALITY_NAMES). */
const QUALITIES = ["good", "bad", "forced", "missing"] as const;
type Quality = (typeof QUALITIES)[number];

/**
 * Expands a runtime run's compact samples into the per-channel form. Every other field passes
 * through unchanged. A run whose `channels` or `samples` are not arrays is returned unchanged, so
 * the recorded-run validator reports it.
 */
export function expandRecordedRunWire(value: unknown): unknown {
  if (!isRecord(value) || !Array.isArray(value.channels) || !Array.isArray(value.samples)) return value;
  const ids: string[] = [];
  for (const channel of value.channels) {
    if (!isRecord(channel) || typeof channel.id !== "string") throw new Error("Invalid recorded-run channels.");
    ids.push(channel.id);
  }
  const samples: unknown[] = value.samples;
  return { ...value, samples: samples.map(sample => expandSample(sample, ids)) };
}

/** Expands the runs inside a search result's trials; a trial without a run passes through. */
export function expandFiniteSearchResultWire(value: unknown): unknown {
  if (!isRecord(value) || !Array.isArray(value.trials)) return value;
  const trials: unknown[] = value.trials;
  return { ...value, trials: trials.map(trial => isRecord(trial) && trial.run !== undefined
    ? { ...trial, run: expandRecordedRunWire(trial.run) } : trial) };
}

function expandSample(sample: unknown, ids: readonly string[]): Record<string, unknown> {
  if (!isRecord(sample) || !Array.isArray(sample.values)) throw invalidSample();
  const values: unknown[] = sample.values;
  const quality: unknown = sample.quality;
  if (values.length !== ids.length || (quality !== undefined && (!Array.isArray(quality) || quality.length !== ids.length))) {
    throw invalidSample();
  }
  const codes: unknown[] | undefined = Array.isArray(quality) ? quality : undefined;
  const expanded: Record<string, { value: number | null; quality: Quality }> = {};
  ids.forEach((id, index) => {
    const value = values[index];
    const code = codes === undefined ? 0 : codes[index];
    // A missing value has no measurement, so it cannot be good.
    if ((value !== null && typeof value !== "number") || !isQualityCode(code) || (value === null && code === 0)) throw invalidSample();
    expanded[id] = { value: value as number | null, quality: QUALITIES[code as number] };
  });
  const { values: _values, quality: _quality, ...rest } = sample;
  return { ...rest, values: expanded };
}

function isQualityCode(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value < QUALITIES.length;
}

function invalidSample(): Error {
  return new Error("Invalid recorded-run sample encoding.");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
