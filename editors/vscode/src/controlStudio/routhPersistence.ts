import type { DeclaredControlAnalysis } from "./declaredAnalysis";
import { decodeDeclaredAnalysis } from "./declaredAnalysis";
import type { calculateDeclaredRouth } from "./routhHurwitz";

export interface SavedRouthAnalysis { selectedValue: number; result: ReturnType<typeof calculateDeclaredRouth>; definition: DeclaredControlAnalysis }
export function isSavedRouthAnalysis(value: unknown): value is SavedRouthAnalysis {
  if (!record(value) || typeof value.selectedValue !== "number" || !Number.isFinite(value.selectedValue)
    || !record(value.result) || !record(value.definition) || !decodeDeclaredAnalysis(value.definition)) return false;
  const result = value.result;
  if (result.selectedValue !== value.selectedValue || typeof result.parameter !== "string" || typeof result.parameterUnit !== "string"
    || !record(result.allowedDomain) || !finite(result.allowedDomain.minimum) || !finite(result.allowedDomain.maximum)
    || result.allowedDomain.minimum > result.allowedDomain.maximum || !record(result.fixedParameters)
    || typeof result.provenance !== "string" || typeof result.scope !== "string" || !Array.isArray(result.table)
    || !polynomial(result.polynomial) || !interval(result.interval)) return false;
  if (!Object.values(result.fixedParameters).every(item => record(item) && finite(item.value) && typeof item.unit === "string")) return false;
  return result.table.every(row => record(row) && Number.isInteger(row.power) && Array.isArray(row.entries)
    && row.entries.every(finite) && Array.isArray(row.calculations) && row.calculations.every(calculation => record(calculation)
      && Number.isInteger(calculation.column) && [calculation.a0, calculation.aNext, calculation.b0, calculation.bNext, calculation.result].every(finite)));
}
function polynomial(value: unknown): boolean {
  if (!record(value) || !["qualified", "boundary", "uncertain", "rejected"].includes(String(value.status)) || !Array.isArray(value.coefficients)
    || !value.coefficients.every(finite) || !Array.isArray(value.rows) || !Array.isArray(value.firstColumn) || value.signChanges !== null && !Number.isInteger(value.signChanges)
    || value.stable !== null && typeof value.stable !== "boolean" || typeof value.interpretation !== "string" || !Array.isArray(value.issues)) return false;
  return value.firstColumn.every(entry => record(entry) && finite(entry.value) && ["positive", "negative", "zero"].includes(String(entry.sign)))
    && value.rows.every(row => record(row) && Number.isInteger(row.power) && Array.isArray(row.entries) && row.entries.every(finite)
      && Array.isArray(row.calculations) && row.calculations.every(calculation => record(calculation) && Number.isInteger(calculation.column)
        && [calculation.a0, calculation.aNext, calculation.b0, calculation.bNext, calculation.result].every(finite)))
    && (value.symbolicRows === undefined || Array.isArray(value.symbolicRows) && value.symbolicRows.every(row => record(row) && Number.isInteger(row.power) && Array.isArray(row.entries) && row.entries.every(item => typeof item === "string")))
    && value.issues.every(issue => record(issue) && ["invalid-polynomial", "origin-pole", "zero-pivot", "all-zero-row", "near-zero-pivot", "near-zero-entry", "numerical-overflow"].includes(String(issue.kind)) && typeof issue.explanation === "string");
}
function interval(value: unknown): boolean {
  if (!record(value) || !["qualified", "unsupported", "uncertain"].includes(String(value.status)) || !["open", "empty", "all-real", "unavailable", "union"].includes(String(value.kind))
    || !Array.isArray(value.inequalities) || !value.inequalities.every(item => typeof item === "string") || typeof value.derivation !== "string") return false;
  if (value.lower !== undefined && !finite(value.lower) || value.upper !== undefined && !finite(value.upper)) return false;
  if (value.lower !== undefined && value.upper !== undefined && value.lower >= value.upper) return false;
  if (value.intervals !== undefined && (!Array.isArray(value.intervals) || !value.intervals.every(item => record(item)
    && (item.lower === undefined || finite(item.lower)) && (item.upper === undefined || finite(item.upper))
    && (item.lowerBoundary === undefined || finite(item.lowerBoundary)) && (item.upperBoundary === undefined || finite(item.upperBoundary))))) return false;
  if (value.kind === "union" && (!Array.isArray(value.intervals) || !Array.isArray(value.boundaries))) return false;
  if (value.boundaries !== undefined && (!Array.isArray(value.boundaries) || !value.boundaries.every(item => record(item)
    && Array.isArray(item.polynomial) && item.polynomial.every(entry => typeof entry === "string") && typeof item.lower === "string" && typeof item.upper === "string"))) return false;
  return true;
}
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
