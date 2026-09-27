import { analyzeStateSpace, type DenseMatrix } from "./numerics";
import type { GeneralStateSpaceModel } from "./generalModel";
export type GeneralStateStructure = { status: "unavailable"; reason: string } | {
  status: "qualified";
  controllability: { rank: number; matrix: DenseMatrix };
  observability: { rank: number; matrix: DenseMatrix };
  scope: string;
};
export function calculateGeneralStateStructure(model: GeneralStateSpaceModel): GeneralStateStructure {
  try {
    const result = analyzeStateSpace(model, { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 }) as {
      status: string; controllability?: { rank: number; matrix: DenseMatrix }; observability?: { rank: number; matrix: DenseMatrix };
    };
    if (result.status !== "qualified" || !result.controllability || !result.observability) return { status: "unavailable", reason: "State-space structural analysis did not qualify." };
    const structure: GeneralStateStructure = { status: "qualified",
      controllability: { rank: result.controllability.rank, matrix: result.controllability.matrix },
      observability: { rank: result.observability.rank, matrix: result.observability.matrix },
      scope: "Ranks describe the supplied local unconstrained model in declared state/input/output order. They do not establish bounded physical reachability, arbitrary steady targets or useful estimation in noise." };
    return isGeneralStateStructure(structure, model) ? structure : { status: "unavailable", reason: "Structural matrices or ranks are nonfinite or inconsistent." };
  } catch { return { status: "unavailable", reason: "State-space structural calculation failed." }; }
}
export function isGeneralStateStructure(value: unknown, model: GeneralStateSpaceModel): value is GeneralStateStructure {
  if (!record(value)) return false;
  if (value.status === "unavailable") return typeof value.reason === "string";
  if (value.status !== "qualified" || typeof value.scope !== "string") return false;
  const states = model.states.length;
  return matrixRank(value.controllability, states, states * model.inputs.length, states)
    && matrixRank(value.observability, states * model.outputs.length, states, states);
}
function matrixRank(value: unknown, rows: number, columns: number, states: number): boolean {
  return record(value) && Number.isSafeInteger(value.rank) && (value.rank as number) >= 0 && (value.rank as number) <= states
    && Array.isArray(value.matrix) && value.matrix.length === rows
    && value.matrix.every(row => Array.isArray(row) && row.length === columns && row.every(v => typeof v === "number" && Number.isFinite(v)));
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
