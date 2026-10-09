// Persisted draft records (specification 43, §6 and "Repairs — 7 October 2026"): the stored
// shape, its compact receipts, and which finished records may leave storage. Open drafts are
// never removed here: only applied or discarded records go, oldest first.

import { createHash } from "crypto";
import type { Draft } from "./drafts";

/** Storage bounds of one workspace. The product uses DRAFT_LIMITS; tests may lower them. */
export interface DraftLimits {
  /** Open drafts: neither applied nor discarded. They are never removed automatically. */
  maxOpen: number;
  /** Finished (applied or discarded) records kept as retry receipts; the oldest leave first. */
  maxFinished: number;
  /** Bytes of all records together. Finished records leave first, oldest first, to make room. */
  maxBytes: number;
  /** Edit receipts kept per draft; the oldest leave first. */
  maxReceipts: number;
  /** How long a finished record is kept as a retry receipt, in milliseconds. */
  retentionMs: number;
  now: () => number;
}

export const DRAFT_LIMITS: DraftLimits = {
  maxOpen: 5,
  maxFinished: 64,
  maxBytes: 50 * 1024 * 1024,
  maxReceipts: 64,
  retentionMs: 7 * 24 * 60 * 60 * 1000,
  now: () => Date.now(),
};

/** The receipt of one edit: its committed revision and content hashes, never the page bodies. */
export interface EditReceipt {
  fingerprint: string;
  revision: string;
  contentHashes: Record<string, string>;
  data?: unknown;
  /** Order of commitment, for oldest-first removal. */
  sequence: number;
}
/** `contentHashes` is present while the original creation outcome can still be returned. */
export interface CreationReceipt {
  key: string;
  fingerprint: string;
  contentHashes?: Record<string, string>;
  data?: unknown;
}
export interface ApplicationReceipt {
  fingerprint: string;
  status: "prepared" | "committed";
  outcome?: unknown;
}
export interface StoredDraft {
  format: 2;
  draft: Draft;
  discarded?: true;
  /** When the draft was applied or discarded, in milliseconds since the epoch. */
  finishedAt?: number;
  creation?: CreationReceipt;
  requests: Record<string, EditReceipt>;
  applications?: Record<string, ApplicationReceipt>;
}

/** One record file as found in storage; `stored` is absent for an unreadable record, which is
 * retained for inspection and never removed automatically. */
export interface RecordEntry {
  id: string;
  file: string;
  size: number;
  modified: number;
  stored?: StoredDraft;
}

const plain = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const text = (value: unknown): string | undefined => (typeof value === "string" ? value : undefined);
/** A map of strings (file texts or content hashes), or undefined. */
const stringMap = (value: unknown): Record<string, string> | undefined =>
  plain(value) && Object.values(value).every((item) => typeof item === "string")
    ? (value as Record<string, string>)
    : undefined;

export function contentHashes(files: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(files).map(([file, body]) => [file, createHash("sha256").update(body).digest("hex")]),
  );
}

/** Sets an own enumerable property for any client key, including `__proto__`. */
export function define<T>(target: Record<string, T>, key: string, value: T): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

/** Format 1 kept a full draft copy (pages and base snapshots) in each edit receipt. */
function compactReceipt(entry: unknown, order: number): EditReceipt | undefined {
  if (!plain(entry)) return undefined;
  const {
    fingerprint: storedFingerprint,
    revision: storedRevision,
    contentHashes: storedHashes,
    data,
    sequence: storedSequence,
    outcome,
  } = entry;
  const legacy: Record<string, unknown> = plain(outcome) ? outcome : {};
  const fingerprint = text(storedFingerprint);
  const revision = text(storedRevision) ?? text(legacy.revision);
  if (fingerprint === undefined || revision === undefined) return undefined;
  const legacyFiles = stringMap(legacy.files);
  const hashes =
    stringMap(storedHashes) ?? stringMap(legacy.contentHashes) ?? (legacyFiles ? contentHashes(legacyFiles) : {});
  const sequence = typeof storedSequence === "number" ? storedSequence : order;
  return { fingerprint, revision, contentHashes: hashes, ...(data === undefined ? {} : { data }), sequence };
}

/** Format 1 kept the complete created draft in the creation receipt. */
function compactCreation(entry: unknown): CreationReceipt | undefined {
  if (!plain(entry)) return undefined;
  const {
    key: storedKey,
    fingerprint: storedFingerprint,
    contentHashes: storedHashes,
    data: storedData,
    outcome: storedOutcome,
  } = entry;
  const key = text(storedKey),
    fingerprint = text(storedFingerprint);
  if (key === undefined || fingerprint === undefined) return undefined;
  const outcome: Record<string, unknown> | undefined = plain(storedOutcome) ? storedOutcome : undefined;
  const createdDraft = outcome?.draft;
  const created: Record<string, unknown> = plain(createdDraft) ? createdDraft : {};
  const createdFiles = stringMap(created.files);
  const hashes =
    stringMap(storedHashes) ??
    stringMap(created.contentHashes) ??
    (createdFiles ? contentHashes(createdFiles) : undefined);
  const data = outcome ? outcome.data : storedData;
  return {
    key,
    fingerprint,
    ...(hashes ? { contentHashes: hashes } : {}),
    ...(hashes && data !== undefined ? { data } : {}),
  };
}

/** A parsed record in the current shape, or undefined for an unsupported one. Format 1 records
 * are read with compact receipts and written back compact on their next change. */
export function normalizedRecord(value: unknown): StoredDraft | undefined {
  if (!plain(value)) return undefined;
  const {
    format,
    draft,
    requests: storedRequests,
    creation: storedCreation,
    applications,
    discarded,
    finishedAt,
  } = value;
  if (
    (format !== 1 && format !== 2) ||
    !plain(draft) ||
    !plain(storedRequests) ||
    (applications !== undefined && !plain(applications))
  )
    return undefined;
  const requests: Record<string, EditReceipt> = {};
  let order = 0;
  for (const [key, entry] of Object.entries(storedRequests)) {
    const receipt = compactReceipt(entry, order);
    if (!receipt) return undefined;
    define(requests, key, receipt);
    order += 1;
  }
  const creation = storedCreation === undefined ? undefined : compactCreation(storedCreation);
  if (storedCreation !== undefined && !creation) return undefined;
  return {
    format: 2,
    draft: draft as unknown as Draft,
    requests,
    ...(discarded === true ? { discarded: true as const } : {}),
    ...(typeof finishedAt === "number" ? { finishedAt } : {}),
    ...(creation ? { creation } : {}),
    ...(applications !== undefined ? { applications: applications as Record<string, ApplicationReceipt> } : {}),
  };
}

export const isFinished = (stored: StoredDraft): boolean =>
  stored.discarded === true || stored.draft.state === "applied";

/** A finished record's age; a format 1 record without its finishing time uses the file's. */
const finishedTime = (entry: RecordEntry): number => entry.stored?.finishedAt ?? entry.modified;

/** The record that remains of a finished draft: its identity and creation key, so an old retry
 * cannot revive or rebuild it, and for an applied draft its application receipts, so an apply
 * retry still returns the original outcome. No page bodies, base snapshots or edit receipts. */
export function finishedRecord(stored: StoredDraft, now: number, outcome: "applied" | "discarded"): StoredDraft {
  const { id, workspaceId, revision, state } = stored.draft;
  const applied = outcome === "applied";
  const creation = stored.creation ? { key: stored.creation.key, fingerprint: stored.creation.fingerprint } : undefined;
  return {
    format: 2,
    finishedAt: now,
    ...(applied ? {} : { discarded: true as const }),
    draft: {
      id,
      workspaceId,
      revision,
      state: applied ? "applied" : state,
      files: {},
      contentHashes: {},
      base: [],
      findings: [],
    },
    requests: {},
    ...(applied && stored.applications ? { applications: stored.applications } : {}),
    ...(creation ? { creation } : {}),
  };
}

/** The receipts with one more, keeping only the newest `max`. */
export function withEditReceipt(
  requests: Record<string, EditReceipt>,
  key: string,
  receipt: Omit<EditReceipt, "sequence">,
  max: number,
): Record<string, EditReceipt> {
  const entries = Object.entries(requests).sort(([, a], [, b]) => a.sequence - b.sequence);
  const next: Record<string, EditReceipt> = {};
  for (const [name, entry] of entries.slice(Math.max(0, entries.length - Math.max(0, max - 1))))
    define(next, name, entry);
  define(next, key, { ...receipt, sequence: entries.length ? entries[entries.length - 1][1].sequence + 1 : 0 });
  return next;
}

/** The finished records to remove, oldest first: those past retention, those beyond the count
 * bound, then as many more as `needBytes` requires. Open and unreadable records never appear. */
export function evictionPlan(entries: RecordEntry[], limits: DraftLimits, needBytes = 0, keep?: string): RecordEntry[] {
  const now = limits.now();
  const finished = entries
    .filter((entry) => entry.stored !== undefined && isFinished(entry.stored) && entry.id !== keep)
    .sort((a, b) => finishedTime(a) - finishedTime(b) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const plan: RecordEntry[] = [];
  let freed = 0;
  finished.forEach((entry, index) => {
    const expired = now - finishedTime(entry) >= limits.retentionMs;
    const surplus = finished.length - index > limits.maxFinished;
    if (expired || surplus || freed < needBytes) {
      plan.push(entry);
      freed += entry.size;
    }
  });
  return plan;
}

/** Open drafts as a person can act on them: id, pages, state and revision. */
export function describeOpen(records: StoredDraft[]): string {
  return records
    .map(({ draft }) => {
      const pages = Object.keys(draft.files).sort().join(", ") || "no pages";
      return `${draft.id} (${pages}; ${draft.state}; revision ${draft.revision})`;
    })
    .join("; ");
}
