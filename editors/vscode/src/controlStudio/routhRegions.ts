import type { RouthInterval } from "./routhHurwitz";
import { type RationalPolynomial, type RootInterval, makeRationalPolynomial as poly, polynomialMultiply as mul,
  polynomialPrimitive, polynomialSquareFree, polynomialDeterminant, polynomialZero, polynomialEvaluate,
  isolateRealRoots, rational, rationalAdd, rationalDivide, rationalCompare, rationalText, rationalNumber } from "./exactPolynomial";

/** Strict Hurwitz inequalities are constant in sign between certified determinant roots. */
export function hurwitzPositiveInterval(coefficients: readonly { constant: number; slope: number }[]): RouthInterval {
  const n = coefficients.length - 1;
  if (n < 1 || n > 16) return { status: "unsupported", kind: "unavailable", inequalities: [], derivation: "Characteristic degree must be between 1 and 16." };
  const inequalities = Array.from({ length: n }, (_, i) => `Δ${i + 1}(p) / a₀(p)^${i + 1} > 0`);
  try {
    const c = coefficients.map(value => poly([value.slope, value.constant])), leading = c[0];
    if (polynomialZero(leading)) throw new Error("The characteristic leading coefficient is identically zero.");
    const constraints: RationalPolynomial[] = [];
    for (let size = 1; size <= n; size++) {
      const h = Array.from({ length: size }, (_, row) => Array.from({ length: size }, (_, column) => c[2 * column - row + 1] ?? poly([0])));
      const determinant = polynomialDeterminant(h);
      // a₀^(2i) is positive off its roots. Only odd powers change the sign.
      constraints.push(polynomialPrimitive(size % 2 ? mul(determinant, leading) : determinant));
    }
    if (constraints.some(polynomialZero)) return { status: "qualified", kind: "empty", inequalities, derivation: "A strict Hurwitz determinant inequality is identically zero." };
    let boundaryPolynomial = poly([1]);
    for (const constraint of constraints) boundaryPolynomial = polynomialSquareFree(mul(boundaryPolynomial, polynomialSquareFree(constraint)));
    const isolation = isolateRealRoots(boundaryPolynomial);
    if (isolation.status !== "qualified") throw new Error(isolation.reason);
    const roots = isolation.intervals;
    const boundaries = roots.map(root => ({ polynomial: boundaryPolynomial.coefficients.map(rationalText), lower: rationalText(root.exactLower), upper: rationalText(root.exactUpper) }));
    const intervals: NonNullable<RouthInterval["intervals"]>[number][] = [];
    for (let i = 0; i <= roots.length; i++) {
      const left = roots[i - 1], right = roots[i];
      if (left && right && rationalCompare(left.exactUpper, right.exactLower) >= 0) throw new Error("Root enclosures need further separation.");
      const probe = left && right ? rationalDivide(rationalAdd(left.exactUpper, right.exactLower), rational(2n))
        : left ? rationalAdd(left.exactUpper, rational(1n)) : right ? rationalAdd(right.exactLower, rational(-1n)) : rational(0n);
      if (constraints.every(p => polynomialEvaluate(p, probe).numerator > 0n)) intervals.push({
        ...(left ? { lower: displayRoot(left), lowerBoundary: i - 1 } : {}),
        ...(right ? { upper: displayRoot(right), upperBoundary: i } : {}),
      });
    }
    const kind = !intervals.length ? "empty" : intervals.length > 1 ? "union" : intervals[0].lower === undefined && intervals[0].upper === undefined ? "all-real" : "open";
    return { status: "qualified", kind, ...(intervals.length === 1 ? intervals[0] : {}), intervals, boundaries, inequalities,
      derivation: "Exact rational Bareiss determinants and Sturm root isolation establish every open stable component. Decimal bounds are display approximations; the retained polynomial and rational enclosures identify the exact boundaries. Leading-coefficient zeros and determinant zeros are excluded." };
  } catch (error) { return { status: "uncertain", kind: "unavailable", inequalities,
    derivation: error instanceof Error ? error.message : String(error) }; }
}
function displayRoot(root: RootInterval): number { return rationalNumber(rationalDivide(rationalAdd(root.exactLower, root.exactUpper), rational(2n))); }
