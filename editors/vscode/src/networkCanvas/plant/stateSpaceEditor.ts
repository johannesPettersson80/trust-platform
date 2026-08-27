import type { PlantParameters } from "./editorModel";

export interface PlantStateSpaceDimensions {
  states: number;
  inputs: number;
  outputs: number;
}

export interface PlantStateSpaceCellIssue {
  matrix: "a" | "b" | "c" | "d" | "initial_state";
  row: number;
  column: number;
  message: string;
}

export function stateSpaceDimensions(parameters: PlantParameters): PlantStateSpaceDimensions {
  const a = matrix(parameters.a);
  const b = matrix(parameters.b);
  const c = matrix(parameters.c);
  return {
    states: Math.max(1, a.length),
    inputs: Math.max(1, b[0]?.length ?? 0),
    outputs: Math.max(1, c.length),
  };
}

export function parseStateSpacePaste(text: string): number[][] | undefined {
  const rows = text.trim().split(/\r?\n/).map((row) => row.trim().split(/[\t, ]+/).map(Number));
  const width = rows[0]?.length ?? 0;
  if (rows.length === 0 || width === 0 || rows.some((row) => row.length !== width || row.some((value) => !Number.isFinite(value)))) {
    return undefined;
  }
  return rows;
}

export function validateStateSpace(input: {
  parameters: PlantParameters;
  initialState: number[];
  dimensions: PlantStateSpaceDimensions;
}): PlantStateSpaceCellIssue[] {
  const issues: PlantStateSpaceCellIssue[] = [];
  const shapes: Array<["a" | "b" | "c" | "d", number, number]> = [
    ["a", input.dimensions.states, input.dimensions.states],
    ["b", input.dimensions.states, input.dimensions.inputs],
    ["c", input.dimensions.outputs, input.dimensions.states],
    ["d", input.dimensions.outputs, input.dimensions.inputs],
  ];
  for (const [name, rows, columns] of shapes) {
    const value = matrix(input.parameters[name]);
    if (value.length !== rows || value.some((row) => row.length !== columns)) {
      issues.push({ matrix: name, row: 0, column: 0, message: `${name.toUpperCase()} must be ${rows} × ${columns}.` });
      continue;
    }
    value.forEach((row, rowIndex) => row.forEach((cell, columnIndex) => {
      if (!Number.isFinite(cell)) issues.push({
        matrix: name,
        row: rowIndex,
        column: columnIndex,
        message: `${name.toUpperCase()}[${rowIndex + 1},${columnIndex + 1}] must be a finite number.`,
      });
    }));
  }
  if (input.initialState.length !== input.dimensions.states) {
    issues.push({ matrix: "initial_state", row: 0, column: 0, message: `Initial state must contain ${input.dimensions.states} values.` });
  } else {
    input.initialState.forEach((cell, column) => {
      if (!Number.isFinite(cell)) issues.push({ matrix: "initial_state", row: 0, column, message: `Initial state[${column + 1}] must be a finite number.` });
    });
  }
  return issues;
}

export function resizeStateSpace(input: {
  parameters: PlantParameters;
  initialState: number[];
  requested: PlantStateSpaceDimensions;
}): { parameters: PlantParameters; initialState: number[] } {
  const { states, inputs, outputs } = input.requested;
  return {
    parameters: {
      ...input.parameters,
      a: resizeMatrix(matrix(input.parameters.a), states, states),
      b: resizeMatrix(matrix(input.parameters.b), states, inputs),
      c: resizeMatrix(matrix(input.parameters.c), outputs, states),
      d: resizeMatrix(matrix(input.parameters.d), outputs, inputs),
    },
    initialState: Array.from({ length: states }, (_, index) => input.initialState[index] ?? 0),
  };
}

export function planStateSpaceResize(input: {
  parameters: PlantParameters;
  initialState: number[];
  dimensions: PlantStateSpaceDimensions;
  requested: PlantStateSpaceDimensions;
}): ({ kind: "apply" } | { kind: "confirm"; message: string }) & {
  parameters: PlantParameters;
  initialState: number[];
} {
  const resized = resizeStateSpace(input);
  const removesValues = input.requested.states < input.dimensions.states ||
    input.requested.inputs < input.dimensions.inputs ||
    input.requested.outputs < input.dimensions.outputs;
  return removesValues
    ? { kind: "confirm", message: "Reducing dimensions removes matrix or initial-state values.", ...resized }
    : { kind: "apply", ...resized };
}

function matrix(value: unknown): number[][] {
  return Array.isArray(value)
    ? value.map((row) => Array.isArray(row) ? row.map((cell) => typeof cell === "number" ? cell : Number.NaN) : [])
    : [];
}

function resizeMatrix(source: readonly number[][], rows: number, columns: number): number[][] {
  return Array.from({ length: rows }, (_, row) =>
    Array.from({ length: columns }, (_, column) => source[row]?.[column] ?? 0)
  );
}
