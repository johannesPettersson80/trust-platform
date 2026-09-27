/** Origin labels carry no model-validation or runtime-write authority. */
export interface AutomaticTestCriteria {
  modelRevision: string;
  fields: readonly string[];
}
export const automaticRequirementKeys = ["referenceChange", "maximumError", "settlingBand", "settlingTimeSeconds", "durationSeconds", "tuningObjective"] as const;
export function decodeAutomaticTestCriteria(value: unknown): AutomaticTestCriteria | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const input = value as Record<string, unknown>;
  if (Object.keys(input).some(key => !["modelRevision", "fields"].includes(key)) || typeof input.modelRevision !== "string" || !input.modelRevision
    || input.modelRevision.length > 256 || !Array.isArray(input.fields) || input.fields.length > 70
    || input.fields.some(field => typeof field !== "string" || field.length > 256
      || !automaticRequirementKeys.some(key => field === key || field.startsWith(`${key}:`)))
    || new Set(input.fields).size !== input.fields.length) return undefined;
  return { modelRevision: input.modelRevision, fields: [...input.fields] as string[] };
}
