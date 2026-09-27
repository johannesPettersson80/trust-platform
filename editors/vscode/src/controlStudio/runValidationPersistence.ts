import { decodeRunRequirement, evaluateRecordedRun, type RecordedRun, type RunComparison, type RunEvaluation, type RunRequirement } from "./runValidation";
export interface SavedRunValidation {
  origin: "imported-record-contract";
  current: RecordedRun;
  candidate?: RecordedRun;
  requirements: readonly RunRequirement[];
  objective?: string;
  evaluation: RunEvaluation;
  comparison?: RunComparison;
}
export function isSavedRunValidation(value: unknown): value is SavedRunValidation {
  if (!record(value) || value.origin !== "imported-record-contract" || !Array.isArray(value.requirements)
    || value.requirements.length === 0 || value.requirements.length > 64
    || (value.objective !== undefined && typeof value.objective !== "string")) return false;
  try {
    const current = evaluateRecordedRun(value.current, []).run;
    const candidate = value.candidate === undefined ? undefined : evaluateRecordedRun(value.candidate, []).run;
    const requirements = value.requirements.map(requirement => decodeRunRequirement(requirement, current));
    if (new Set(requirements.map(requirement => requirement.id)).size !== requirements.length) return false;
    if (value.objective !== undefined && !requirements.some(requirement => requirement.id === value.objective)) return false;
    if (!evaluation(value.evaluation, current, value.requirements)) return false;
    if (candidate) {
      if (!record(value.comparison) || !evaluation(value.comparison.current, current, value.requirements)
        || !evaluation(value.comparison.candidate, candidate, value.requirements)
        || !status(value.comparison.status) || typeof value.comparison.reason !== "string"
        || (value.comparison.preferred !== undefined && !["current", "candidate", "tie"].includes(String(value.comparison.preferred)))) return false;
    } else if (value.comparison !== undefined) return false;
    return true;
  } catch { return false; }
}
function evaluation(value: unknown, run: RecordedRun, requirements: unknown[]): boolean {
  return record(value) && status(value.status) && typeof value.scope === "string"
    && JSON.stringify(value.run) === JSON.stringify(run) && JSON.stringify(value.specification) === JSON.stringify(requirements)
    && Array.isArray(value.requirements) && value.requirements.length === requirements.length
    && value.requirements.every((row, i) => record(row) && record(requirements[i]) && row.id === requirements[i].id
      && typeof row.kind === "string" && status(row.status) && typeof row.unit === "string" && typeof row.reason === "string"
      && (row.value === undefined || finite(row.value)) && (row.limit === undefined || finite(row.limit)));
}
function status(value: unknown): boolean { return ["Pass", "Fail", "Inconclusive"].includes(String(value)); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
