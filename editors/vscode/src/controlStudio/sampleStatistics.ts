/**
 * Statistics over recorded samples. They loop instead of spreading the samples into Math.max, Math.min or Math.hypot: a run or a
 * recording keeps up to 100,000 samples (Specification 35, A7), and a spread of that size can exceed the engine's argument limit.
 */

/** The largest value, from `floor` upward; `floor` when there are no values. */
export function largest(values: readonly number[], floor = Number.NEGATIVE_INFINITY): number {
  let result = floor;
  for (const value of values) if (value > result) result = value;
  return result;
}

/** The smallest value; positive infinity when there are no values. */
export function smallest(values: readonly number[]): number {
  let result = Number.POSITIVE_INFINITY;
  for (const value of values) if (value < result) result = value;
  return result;
}

/** The square root of the sum of squares, as Math.hypot computes it. */
export function rootSumSquare(values: readonly number[]): number {
  let sum = 0;
  for (const value of values) sum += value * value;
  return Math.sqrt(sum);
}
