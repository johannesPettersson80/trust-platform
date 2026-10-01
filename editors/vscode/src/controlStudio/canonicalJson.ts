/** Deterministic JSON text for evidence equality: object keys sorted, arrays in order. Built in one pass,
 * it matches the earlier stringify-parse-restringify form exactly: integer-like keys come first in numeric
 * order, as JavaScript orders an object's own keys, and a member that is not a JSON value is rejected. */
export function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(member).join(",")}]`;
  if (record(value)) return `{${orderedKeys(value).map(key => `${JSON.stringify(key)}:${member(value[key])}`).join(",")}}`;
  return JSON.stringify(value);
}

function member(value: unknown): string {
  const text = canonicalJson(value);
  if (text === undefined) throw new SyntaxError("A canonical evidence member must be a JSON value.");
  return text;
}

const ARRAY_INDEX = /^(?:0|[1-9]\d*)$/;
function orderedKeys(value: Record<string, unknown>): string[] {
  const sorted = Object.keys(value).sort();
  const isIndex = (key: string) => ARRAY_INDEX.test(key) && Number(key) < 4294967295;
  const indices = sorted.filter(isIndex);
  return indices.length === 0 ? sorted : [...indices.sort((left, right) => Number(left) - Number(right)), ...sorted.filter(key => !isIndex(key))];
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
