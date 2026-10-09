import { createHash } from "crypto";
import { canonicalJson } from "./canonicalJson";
import { expandRecordedRunWire } from "./recordedRunWire";

/**
 * Recordings of a Control Studio study (Specification 35, A8.3 and B10.1). A recorded run is written once, in the
 * runtime's compact wire form: its channels once, then per sample its time and its values in channel order, with
 * quality codes only where a value is not good. Its file name is derived from its content, so the same recording
 * always maps to the same file and a recording file is never rewritten.
 */

/** The quality vocabulary of the wire format, in code order (recordedRunWire.ts). */
const QUALITIES = ["good", "bad", "forced", "missing"] as const;

/** A recorded run in the wire form that is written to a recording file. */
export type CompactRecordedRun = Record<string, unknown>;

/** Stands in for an inline recorded run inside a study document. */
export interface RecordingReference { $recording: string }

type InlineSample = Record<string, unknown> & { values?: Record<string, { value: unknown; quality: unknown }> };

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

/** A recorded run in the extension's own form: per-channel values and qualities, as the verification client keeps it. */
export function isInlineRecordedRun(value: unknown): value is Record<string, unknown> & { channels: unknown[]; samples: unknown[] } {
  return isRecord(value) && Array.isArray(value.channels) && Array.isArray(value.samples) && isRecord(value.conditions)
    && typeof value.evidence === "string";
}

export function isRecordingReference(value: unknown): value is RecordingReference {
  return isRecord(value) && Object.keys(value).length === 1 && typeof value.$recording === "string";
}

/** Converts a recorded run to the compact wire form, and proves the conversion exact before it is returned. */
export function compactRecordedRun(run: Record<string, unknown>): CompactRecordedRun {
  const normalized = jsonCopy(run);
  const ids = (normalized.channels as Array<{ id?: unknown }>).map(channel => String(channel.id));
  const samples = (normalized.samples as InlineSample[]).map(sample => {
    const { values, ...rest } = sample;
    const cells = ids.map(id => values?.[id]);
    if (!values || cells.some(cell => cell === undefined)) throw new Error("A recorded run sample does not hold a value for every channel.");
    const codes = cells.map(cell => QUALITIES.indexOf(cell!.quality as typeof QUALITIES[number]));
    if (codes.some(code => code < 0)) throw new Error("A recorded run sample holds an unknown quality.");
    return { ...rest, values: cells.map(cell => cell!.value), ...(codes.some(code => code !== 0) ? { quality: codes } : {}) };
  });
  const compact = { ...normalized, samples };
  if (canonicalJson(expandRecordedRunWire(compact)) !== canonicalJson(normalized)) {
    throw new Error("A recorded run cannot be stored in its compact form without changing it.");
  }
  return compact;
}

/** The identity of a recording: a prefix of the SHA-256 of its canonical compact form. */
export function recordingIdFor(run: CompactRecordedRun): string {
  return `rec-${createHash("sha256").update(canonicalJson(run), "utf8").digest("hex").slice(0, 32)}`;
}

/** The compact form and identity of each frozen run, which can no longer change (recordedEvidence.ts). */
const compactedRuns = new WeakMap<object, { id: string; run: CompactRecordedRun }>();

/** Replaces each inline recorded run with a reference and hands its compact form to `store`. */
export function externalizeRecordings(value: unknown, store: (id: string, run: CompactRecordedRun) => void): unknown {
  if (Array.isArray(value)) return value.map(item => externalizeRecordings(item, store));
  if (!isRecord(value)) return value;
  const known = compactedRuns.get(value);
  if (known) {
    store(known.id, known.run);
    const reference: RecordingReference = { $recording: known.id };
    return reference;
  }
  if (isInlineRecordedRun(value)) {
    // A run that cannot be proven exact in compact form stays inline: its data is kept unchanged, only less compactly.
    let run: CompactRecordedRun | undefined;
    try {
      run = compactRecordedRun(value);
    } catch {
      run = undefined;
    }
    if (run) {
      const id = recordingIdFor(run);
      if (Object.isFrozen(value)) compactedRuns.set(value, { id, run });
      store(id, run);
      const reference: RecordingReference = { $recording: id };
      return reference;
    }
  }
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, externalizeRecordings(item, store)]));
}

/** Lists the recording identities a study value refers to, in order of appearance. */
export function recordingReferences(value: unknown, found: string[] = []): string[] {
  if (Array.isArray(value)) value.forEach(item => recordingReferences(item, found));
  else if (isRecordingReference(value)) found.push(value.$recording);
  else if (isRecord(value)) Object.values(value).forEach(item => recordingReferences(item, found));
  return found;
}

/** Expands each reference back into its recorded run, in the extension's own form. */
export function inlineRecordings(value: unknown, load: (id: string) => CompactRecordedRun): unknown {
  if (Array.isArray(value)) return value.map(item => inlineRecordings(item, load));
  if (isRecordingReference(value)) return expandRecordedRunWire(load(value.$recording));
  if (!isRecord(value)) return value;
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, inlineRecordings(item, load)]));
}

/** A JSON copy, which is also the form a stored study keeps: undefined is dropped and numbers are finite. */
export function jsonCopy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
