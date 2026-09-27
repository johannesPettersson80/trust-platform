import { decodeDeclaredAnalysis, type DeclaredControlAnalysis, type DeclaredRootLocusResult } from "./declaredAnalysis";

export interface SavedDeclaredAnalysis {
  definition: DeclaredControlAnalysis;
  result: DeclaredRootLocusResult;
}

/** Checks saved analysis identity and finite coordinates without recomputing historical evidence. */
export function isSavedDeclaredAnalysis(value: unknown): value is SavedDeclaredAnalysis {
  if (!record(value)) return false;
  const definition = decodeDeclaredAnalysis(value.definition);
  const result = value.result;
  if (!definition || !record(result) || result.parameter !== definition.rootLocus.parameter
      || result.selectedValue !== definition.rootLocus.selectedValue
      || result.parameterUnit !== definition.controller.parameters[definition.rootLocus.parameter].unit
      || typeof result.provenance !== "string" || typeof result.approximation !== "string"
      || !record(result.fixedParameters) || !Array.isArray(result.branches)
      || result.branches.length !== definition.rootLocus.values.length) return false;
  const fixedNames = Object.keys(definition.controller.parameters).filter(name => name !== result.parameter);
  if (Object.keys(result.fixedParameters).length !== fixedNames.length || !fixedNames.every(name => {
    const actual = (result.fixedParameters as Record<string, unknown>)[name];
    const expected = definition.controller.parameters[name];
    return record(actual) && actual.value === expected.value && actual.unit === expected.unit;
  })) return false;
  return result.branches.every((branch, index) => record(branch) && branch.gain === definition.rootLocus.values[index]
    && Array.isArray(branch.poles) && branch.poles.length > 0 && branch.poles.length <= 34
    && branch.poles.every(pole => record(pole) && finite(pole.real) && finite(pole.imaginary)));
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
