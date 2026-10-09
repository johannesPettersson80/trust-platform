import { compactStoredEvidence } from "./studyStorageCompaction";

/**
 * Recorded evidence is checked once and is immutable afterwards (Specification 35, A10.7). A verification result that a
 * decoder has checked is frozen and marked. Later snapshots, decoders and saves share it by reference instead of copying
 * it and checking it again, so their cost follows what changed, not everything recorded so far.
 */
const checked = new WeakSet<object>();
/** The checked result decoded from a source object, such as a result the verification client returned. */
const checkedOf = new WeakMap<object, object>();
const compactBytes = new WeakMap<object, number>();

/** Freezes a checked result deeply and marks it, so that it can be shared, and remembers it for its source. */
export function markChecked<T extends object>(result: T, source?: object): T {
  deepFreeze(result);
  checked.add(result);
  if (source) checkedOf.set(source, result);
  return result;
}

/** The checked result `value` is, or was decoded from; undefined when it has not been checked. */
export function checkedFor(value: unknown): object | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  return checked.has(value) ? value : checkedOf.get(value);
}

/** What `JSON.parse(JSON.stringify(value))` returns, except that checked evidence is kept by reference. */
export function shareCopy<T>(value: T): T {
  return copyJson(value) as T;
}

/** What `structuredClone(value)` returns for plain data, except that checked evidence is kept by reference. */
export function shareClone<T>(value: T): T {
  return cloneStructured(value) as T;
}

/**
 * The bytes of the compact stored form of `value` (studyStorageCompaction.ts), JSON-encoded. A checked result's share is
 * measured once and remembered.
 */
export function compactStoredBytes(value: unknown): number {
  let shared = 0;
  const rest = withoutChecked(value, result => {
    let bytes = compactBytes.get(result);
    if (bytes === undefined) {
      bytes = Buffer.byteLength(JSON.stringify(compactStoredEvidence(result)), "utf8");
      compactBytes.set(result, bytes);
    }
    shared += bytes;
  });
  return shared + Buffer.byteLength(JSON.stringify(compactStoredEvidence(rest)) ?? "", "utf8");
}

function deepFreeze(value: unknown): void {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) return;
  Object.freeze(value);
  for (const item of Object.values(value)) deepFreeze(item);
}

function copyJson(value: unknown): unknown {
  if (value === null) return null;
  switch (typeof value) {
    case "string":
    case "boolean":
      return value;
    case "number":
      return Number.isFinite(value) ? (Object.is(value, -0) ? 0 : value) : null;
    case "object":
      break;
    default:
      return undefined;
  }
  if (checked.has(value)) return value;
  const toJson = (value as { toJSON?: () => unknown }).toJSON;
  if (typeof toJson === "function") return copyJson(toJson.call(value));
  if (Array.isArray(value)) {
    return Array.from({ length: value.length }, (_, index) => copyJson(value[index]) ?? null);
  }
  const copy: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    const itemCopy = copyJson(item);
    if (itemCopy !== undefined) copy[key] = itemCopy;
  }
  return copy;
}

function cloneStructured(value: unknown): unknown {
  if (typeof value !== "object" || value === null) return value;
  if (checked.has(value)) return value;
  if (Array.isArray(value)) return value.map(cloneStructured);
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return structuredClone(value);
  const copy: Record<string, unknown> = {};
  for (const key of Object.keys(value)) copy[key] = cloneStructured((value as Record<string, unknown>)[key]);
  return copy;
}

/** A copy with each checked result, or source of one, replaced by null; `found` receives each checked result. */
function withoutChecked(value: unknown, found: (result: object) => void): unknown {
  if (typeof value !== "object" || value === null) return value;
  const result = checkedFor(value);
  if (result) {
    found(result);
    return null;
  }
  if (Array.isArray(value)) return value.map(item => withoutChecked(item, found));
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, withoutChecked(item, found)]));
}
