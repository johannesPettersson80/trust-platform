import { evaluateRecordedRun, type RecordedRun } from "./runValidation";

export interface FiniteSearchTrial {
  index: number;
  parameters: Record<string, number>;
  status: "qualified" | "rejected" | "failed";
  score: number | null;
  score_unit?: string;
  reason?: string;
  violations?: readonly unknown[];
  run?: RecordedRun;
}
export interface FiniteSearchResult {
  job_id: string;
  project_revision: string;
  status: "completed" | "cancelled" | "failed";
  trial_count: number;
  trials: readonly FiniteSearchTrial[];
  ranking: readonly number[];
  unattempted_trials: readonly number[];
  source_basis: "loaded-runtime-sources";
  io_provenance: "isolated-simulation-no-physical-drivers";
  reason?: string | null;
}
export interface SavedFiniteSearch {
  origin: "runtime-isolated-simulation";
  request: Record<string, unknown>;
  result: FiniteSearchResult;
  selectedIndex?: number;
}

export function isSavedFiniteSearch(value: unknown): value is SavedFiniteSearch {
  if (!record(value) || value.origin !== "runtime-isolated-simulation" || !record(value.request) || !record(value.result)) return false;
  const result = value.result;
  if (JSON.stringify(value).length > 34 * 1024 * 1024 || typeof result.job_id !== "string" || !result.job_id ||
      result.project_revision !== value.request.expected_project_revision || value.request.execution !== "isolated-simulation" ||
      !["completed", "cancelled", "failed"].includes(String(result.status)) || !Number.isSafeInteger(result.trial_count) ||
      (result.trial_count as number) < 1 || (result.trial_count as number) > 64 || result.trial_count !== value.request.trial_count ||
      !Array.isArray(result.trials) || result.trials.length > (result.trial_count as number) || !Array.isArray(result.ranking) ||
      !Array.isArray(result.unattempted_trials) || result.source_basis !== "loaded-runtime-sources" ||
      result.io_provenance !== "isolated-simulation-no-physical-drivers") return false;
  try {
    let conditions: string | undefined;
    for (const [index, trial] of result.trials.entries()) {
      if (!record(trial) || trial.index !== index || !record(trial.parameters) ||
          !Object.values(trial.parameters).every(finite) || !["qualified", "rejected", "failed"].includes(String(trial.status))) return false;
      if (trial.status === "qualified" && (!finite(trial.score) || !trial.run)) return false;
      if (trial.run) {
        const run = evaluateRecordedRun(trial.run, []).run;
        if (run.evidence !== "ST-executed" || JSON.stringify(run.parameters) !== JSON.stringify(trial.parameters)) return false;
        const next = JSON.stringify(run.conditions);
        if (conditions !== undefined && conditions !== next) return false;
        conditions = next;
      }
    }
    const trials = result.trials;
    const remaining = Array.from({ length: (result.trial_count as number) - trials.length }, (_, index) => index + trials.length);
    if (JSON.stringify(result.unattempted_trials) !== JSON.stringify(remaining)) return false;
    if (result.status === "completed" && remaining.length !== 0) return false;
    const ranked = result.ranking;
    if (new Set(ranked).size !== ranked.length || (result.status !== "completed" && ranked.length !== 0) ||
        !ranked.every(index => Number.isSafeInteger(index) && trials[index]?.status === "qualified")) return false;
    if (value.selectedIndex !== undefined && !ranked.includes(value.selectedIndex)) return false;
    return true;
  } catch { return false; }
}

export function searchSummary(saved: SavedFiniteSearch): Record<string, unknown> {
  const { result } = saved;
  return { ...result, trials: result.trials.map(({ run: _run, ...trial }) => trial), selectedIndex: saved.selectedIndex };
}
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
