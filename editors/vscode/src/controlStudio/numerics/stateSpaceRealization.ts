/** Unreduced controllable-companion realization, including a static gain. */
export function transferFunctionStateSpace(rawNumerator: readonly number[], rawDenominator: readonly number[]) {
  if (!rawNumerator.length || !rawDenominator.length || rawNumerator.length > rawDenominator.length || rawDenominator[0] === 0) {
    throw new Error("A proper transfer function with a nonzero leading denominator is required.");
  }
  const order = rawDenominator.length - 1;
  const denominator = rawDenominator.map(value => value / rawDenominator[0]);
  const numerator = [...Array<number>(order + 1 - rawNumerator.length).fill(0),
    ...rawNumerator.map(value => value / rawDenominator[0])];
  if (!denominator.every(Number.isFinite) || !numerator.every(Number.isFinite)) {
    throw new Error("Transfer-function coefficients must remain finite after normalization.");
  }
  const a = Array.from({ length: order }, (_, row) => Array.from({ length: order }, (_, column) =>
    row === 0 ? -denominator[column + 1] : Number(column === row - 1)));
  const b = Array.from({ length: order }, (_, row) => [Number(row === 0)]);
  const c = [numerator.slice(1).map((value, index) => value - denominator[index + 1] * numerator[0])];
  return { order, numerator, denominator, a, b, c, d: [[numerator[0]]] };
}
