/** Retained tuning inputs/results never carry runtime authority or a review token. */
export interface EngineerDraft {
  modelRevision: string;
  loopRevision: string;
  family: string;
  values: Readonly<Record<string, number>>;
  design: Readonly<Record<string, unknown>>;
}
export function decodeEngineerDraft(value: unknown): EngineerDraft | undefined {
  if (!record(value) || ![value.modelRevision, value.loopRevision, value.family].every(value => typeof value === "string" && value.length > 0)
    || !record(value.values) || Object.keys(value.values).length > 64
    || !Object.entries(value.values).every(([symbol, number]) => symbol.length > 0 && typeof number === "number" && Number.isFinite(number))
    || !record(value.design) || JSON.stringify(value.design).length > 256000) return undefined;
  return structuredClone(value) as unknown as EngineerDraft;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
