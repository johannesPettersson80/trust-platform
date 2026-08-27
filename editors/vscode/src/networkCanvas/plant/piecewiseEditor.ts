import type { PlantParameters } from "./editorModel";

export interface PlantPiecewisePoint {
  input: number;
  output: number;
}

export interface PlantPiecewiseIssue {
  row: number;
  field: "input" | "output";
  message: string;
}

export function piecewisePointsFromParameters(parameters: PlantParameters): PlantPiecewisePoint[] {
  const inputs = Array.isArray(parameters.input_points) ? parameters.input_points : [];
  const outputs = Array.isArray(parameters.output_points) ? parameters.output_points : [];
  return inputs.map((input, index) => ({
    input: typeof input === "number" ? input : Number.NaN,
    output: typeof outputs[index] === "number" ? outputs[index] as number : Number.NaN,
  }));
}

export function piecewiseParametersFromPoints(points: readonly PlantPiecewisePoint[]): PlantParameters {
  return {
    input_points: points.map((point) => point.input),
    output_points: points.map((point) => point.output),
  };
}

export function appendPiecewisePoint(points: readonly PlantPiecewisePoint[]): PlantPiecewisePoint[] {
  const previous = points[points.length - 1];
  return [
    ...points,
    {
      input: previous ? previous.input + 1 : 0,
      output: previous?.output ?? 0,
    },
  ];
}

export function validatePiecewisePoints(points: readonly PlantPiecewisePoint[]): PlantPiecewiseIssue[] {
  const issues: PlantPiecewiseIssue[] = [];
  if (points.length < 2) {
    issues.push({ row: Math.max(0, points.length - 1), field: "input", message: "Add at least two input/output points." });
  }
  points.forEach((point, row) => {
    if (!Number.isFinite(point.input)) issues.push({ row, field: "input", message: `Input point ${row + 1} must be a finite number.` });
    if (!Number.isFinite(point.output)) issues.push({ row, field: "output", message: `Output point ${row + 1} must be a finite number.` });
    if (row > 0 && Number.isFinite(point.input) && Number.isFinite(points[row - 1].input) && point.input <= points[row - 1].input) {
      issues.push({ row, field: "input", message: `Input point ${row + 1} must be greater than input point ${row}.` });
    }
  });
  return issues;
}

export function movePiecewisePoint(
  points: readonly PlantPiecewisePoint[],
  index: number,
  delta: -1 | 1
): { moved: boolean; points: PlantPiecewisePoint[] } {
  const target = index + delta;
  if (index < 0 || index >= points.length || target < 0 || target >= points.length) {
    return { moved: false, points: [...points] };
  }
  const next = [...points];
  [next[index], next[target]] = [next[target], next[index]];
  if (validatePiecewisePoints(next).length > 0) return { moved: false, points: [...points] };
  return { moved: true, points: next };
}
