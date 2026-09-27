export interface StateDesignPreferences {
  finiteHorizon?: { steps: number; terminalWeights: readonly (readonly number[])[] };
  desiredPoles?: readonly number[];
  stateWeights?: readonly (readonly number[])[];
  inputWeights?: readonly (readonly number[])[];
  processNoise?: readonly (readonly number[])[];
  measurementNoise?: readonly (readonly number[])[];
}

export const STATE_DESIGN_FIELDS = ["desiredPoles", "stateWeights", "inputWeights", "processNoise", "measurementNoise"] as const;
export type StateDesignDraft = Partial<Record<typeof STATE_DESIGN_FIELDS[number] | "solverMode" | "horizonSteps" | "terminalWeights", string>>;
export const STATE_DESIGN_LABELS: Record<keyof StateDesignDraft, string> = {
  desiredPoles: "Desired poles", stateWeights: "State weights (Q)", inputWeights: "Input weights (R)",
  processNoise: "Process noise covariance", measurementNoise: "Measurement noise covariance",
  solverMode: "Solver", horizonSteps: "Horizon steps", terminalWeights: "Terminal weights",
};
export function decodeStateDesignDraft(value: unknown): StateDesignDraft | undefined {
  if (!record(value) || Object.entries(value).some(([key, text]) => !Object.prototype.hasOwnProperty.call(STATE_DESIGN_LABELS, key) || typeof text !== "string" || text.length > 32768)) return undefined;
  return { ...value } as StateDesignDraft;
}
/** Raw text is saved editing state. Only this validated projection reaches a solver. */
export function parseStateDesignDraft(draft: StateDesignDraft, family: string): StateDesignPreferences {
  const result: Record<string, unknown> = {};
  const parse = (key: keyof StateDesignDraft) => {
    try { return JSON.parse(draft[key]!); } catch { throw new Error(`${STATE_DESIGN_LABELS[key]} must be valid JSON.`); }
  };
  for (const key of STATE_DESIGN_FIELDS) {
    const applies = family === "state-feedback" ? key === "desiredPoles" : key === "stateWeights" || key === "inputWeights" || family === "LQG" && (key === "processNoise" || key === "measurementNoise");
    if (applies && draft[key]?.trim()) result[key] = parse(key);
  }
  if (family !== "state-feedback" && draft.solverMode === "finite") {
    const steps = draft.horizonSteps?.trim() ? Number(draft.horizonSteps) : NaN;
    if (!Number.isInteger(steps) || steps < 1 || steps > 512) throw new Error("Horizon steps must be an integer from 1 to 512.");
    if (!draft.terminalWeights?.trim()) throw new Error("Enter the terminal weights matrix.");
    result.finiteHorizon = { steps, terminalWeights: parse("terminalWeights") };
  } else if (draft.solverMode && !["finite", "infinite"].includes(draft.solverMode)) throw new Error("Choose finite or infinite horizon.");
  const decoded = decodeStateDesignPreferences(result);
  if (!decoded) throw new Error("Use finite symmetric weight matrices and valid desired poles.");
  return decoded;
}

/** Draft-only contract; dimensions and definiteness remain design-owner checks. */
export function decodeStateDesignPreferences(value: unknown, dimensions?: { states: number; inputs: number; outputs: number }): StateDesignPreferences | undefined {
  if (!record(value) || Object.keys(value).some(key => !["finiteHorizon", "desiredPoles", "stateWeights", "inputWeights", "processNoise", "measurementNoise"].includes(key))) return undefined;
  if (value.finiteHorizon !== undefined && (!record(value.finiteHorizon) || Object.keys(value.finiteHorizon).some(key => key !== "steps" && key !== "terminalWeights")
    || !Number.isInteger(value.finiteHorizon.steps) || (value.finiteHorizon.steps as number) < 1 || (value.finiteHorizon.steps as number) > 512
    || !matrix(value.finiteHorizon.terminalWeights) || value.finiteHorizon.terminalWeights.some((row, i) => row[i] < 0)
    || dimensions && value.finiteHorizon.terminalWeights.length !== dimensions.states)) return undefined;
  if (value.desiredPoles !== undefined && (!Array.isArray(value.desiredPoles) || !value.desiredPoles.length || value.desiredPoles.length > 32 || value.desiredPoles.some(item => !finite(item)))) return undefined;
  for (const key of ["stateWeights", "inputWeights", "processNoise", "measurementNoise"] as const) if (value[key] !== undefined && !matrix(value[key])) return undefined;
  if (dimensions && value.desiredPoles !== undefined && (value.desiredPoles as number[]).length !== dimensions.states) return undefined;
  for (const key of ["stateWeights", "inputWeights", "processNoise", "measurementNoise"] as const) {
    const weights = value[key] as number[][] | undefined;
    if (!weights) continue;
    const size = dimensions && (key === "inputWeights" ? dimensions.inputs : key === "measurementNoise" ? dimensions.outputs : dimensions.states);
    if (size !== undefined && weights.length !== size) return undefined;
    if (weights.some((row, i) => key === "inputWeights" || key === "measurementNoise" ? row[i] <= 0 : row[i] < 0)) return undefined;
  }
  return structuredClone(value) as StateDesignPreferences;
}
function matrix(value: unknown): value is number[][] {
  if (!Array.isArray(value) || value.length === 0 || value.length > 32
    || !value.every(row => Array.isArray(row) && row.length === value.length && row.every(finite))) return false;
  return (value as number[][]).every((row, i) => row.every((item, j) => Math.abs(item - value[j][i]) <= 1e-10));
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
