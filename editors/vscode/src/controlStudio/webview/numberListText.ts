import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";

/** Matrix rows in declared order, every entry with the precision of the smallest non-zero magnitude. */
export function formatMatrixRows(matrix: readonly (readonly number[])[]): string {
  const magnitudes = matrix.flat().map(Math.abs).filter(value => Number.isFinite(value) && value > 0);
  const digits = magnitudes.length ? automaticFractionDigits(Math.min(...magnitudes)) : undefined;
  return matrix.map(row => `[${row.map(value => formatEngineeringNumber(value, digits)).join(", ")}]`).join("; ");
}

/** A list of numbers, every entry with the precision of the smallest non-zero magnitude. */
export function formatNumberList(values: readonly number[]): string {
  const magnitudes = values.map(Math.abs).filter(value => Number.isFinite(value) && value > 0);
  const digits = magnitudes.length ? automaticFractionDigits(Math.min(...magnitudes)) : undefined;
  return values.map(value => formatEngineeringNumber(value, digits)).join(", ");
}
