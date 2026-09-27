/** Derivative of the auxiliary polynomial in the preceding Routh row. */
export function auxiliaryDerivativeRow(rowAbove: readonly number[], abovePower: number, width: number): number[] {
  return Array.from({ length: width }, (_, index) => abovePower - 2 * index > 0 ? (rowAbove[index] ?? 0) * (abovePower - 2 * index) : 0);
}
