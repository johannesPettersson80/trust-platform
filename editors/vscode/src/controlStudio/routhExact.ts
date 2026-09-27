import { type RationalPolynomial as Polynomial, makeRationalPolynomial as poly, polynomialZero as zero,
  polynomialMultiply as mul, polynomialSubtract as sub, polynomialGcd, polynomialExactDivide,
  polynomial, rationalDivide, rationalMultiply, rationalOne, isolateRealRoots, rationalText } from "./exactPolynomial";

type Fraction = { numerator: Polynomial; denominator: Polynomial };
export interface ExactRouthResult {
  signs: readonly (-1 | 0 | 1)[];
  rows: readonly { power: number; entries: readonly string[] }[];
  zeroPivot: boolean; allZeroRow: boolean; boundary: boolean; uncertain: boolean;
}
const constant = (value: number): Fraction => ({ numerator: poly([value]), denominator: poly([1]) });
const epsilon: Fraction = { numerator: poly([1, 0]), denominator: poly([1]) };
/** A formal positive infinitesimal, with exact rational polynomial operations. */
export function exactRouthSigns(coefficients: readonly number[]): ExactRouthResult | undefined {
  if (coefficients.length < 2 || coefficients.length > 17 || coefficients.some(v => !Number.isFinite(v)) || coefficients[0] === 0) return undefined;
  let zeroPivot = false, allZeroRow = false;
  const signs: (-1 | 0 | 1)[] = [], rows: { power: number; entries: string[] }[] = [];
  try {
    const n = coefficients.length - 1, width = Math.ceil((n + 1) / 2);
    const table: Fraction[][] = [
      Array.from({ length: width }, (_, i) => constant(coefficients[2 * i] ?? 0)),
      Array.from({ length: width }, (_, i) => constant(coefficients[2 * i + 1] ?? 0)),
    ];
    for (let i = 0; i <= n; i++) {
      if (i && table[i].every(v => zero(v.numerator))) {
        allZeroRow = true;
        table[i] = table[i - 1].map((v, j) => n - i + 1 - 2 * j > 0 ? multiply(v, constant(n - i + 1 - 2 * j)) : constant(0));
      }
      if (zero(table[i][0].numerator)) { zeroPivot = true; table[i][0] = epsilon; }
      signs.push(limitSign(table[i][0]));
      rows.push({ power: n - i, entries: table[i].map(format) });
      if (i > 0 && i < n) {
        const prior = table[i - 1], pivot = table[i][0];
        table.push(Array.from({ length: width }, (_, j) => j === width - 1 ? constant(0)
          : divide(subtract(multiply(pivot, prior[j + 1]), multiply(prior[0], table[i][j + 1])), pivot)));
      }
    }
    return { signs, rows, zeroPivot, allZeroRow, boundary: imaginaryAxisBoundary(coefficients), uncertain: false };
  } catch { return { signs, rows, zeroPivot, allZeroRow, boundary: false, uncertain: true }; }
}
function reduce(numerator: Polynomial, denominator: Polynomial): Fraction {
  if (zero(denominator)) throw new Error("Zero formal denominator.");
  if (zero(numerator)) return constant(0);
  const common = polynomialGcd(numerator, denominator);
  const n = polynomialExactDivide(numerator, common)!, d = polynomialExactDivide(denominator, common)!;
  // Polynomial gcd removes factors but not scalar content. A monic denominator
  // keeps equivalent fractions from accumulating enormous common constants.
  const scale = rationalDivide(rationalOne, d.coefficients[0]);
  return { numerator: polynomial(n.coefficients.map(value => rationalMultiply(value, scale))),
    denominator: polynomial(d.coefficients.map(value => rationalMultiply(value, scale))) };
}
function multiply(a: Fraction, b: Fraction): Fraction { return reduce(mul(a.numerator, b.numerator), mul(a.denominator, b.denominator)); }
function subtract(a: Fraction, b: Fraction): Fraction { return reduce(sub(mul(a.numerator, b.denominator), mul(b.numerator, a.denominator)), mul(a.denominator, b.denominator)); }
function divide(a: Fraction, b: Fraction): Fraction { return reduce(mul(a.numerator, b.denominator), mul(a.denominator, b.numerator)); }
function limitSign(value: Fraction): -1 | 0 | 1 {
  const n = [...value.numerator.coefficients].reverse().find(v => v.numerator !== 0n)?.numerator ?? 0n;
  const d = [...value.denominator.coefficients].reverse().find(v => v.numerator !== 0n)?.numerator ?? 0n;
  return n * d > 0n ? 1 : n * d < 0n ? -1 : 0;
}
function format(value: Fraction): string {
  const text = (p: Polynomial) => p.coefficients.map((c, i) => c.numerator === 0n ? "" : `${rationalText(c)}${p.coefficients.length - i > 1 ? `ε^${p.coefficients.length - 1 - i}` : ""}`).filter(Boolean).join(" + ") || "0";
  return value.denominator.coefficients.length === 1 && value.denominator.coefficients[0].numerator === value.denominator.coefficients[0].denominator
    ? text(value.numerator) : `(${text(value.numerator)}) / (${text(value.denominator)})`;
}
/** P(s)=E(s²)+s O(s²); a common negative real root gives imaginary poles. */
function imaginaryAxisBoundary(coefficients: readonly number[]): boolean {
  if (coefficients[coefficients.length - 1] === 0) return true;
  const n = coefficients.length - 1;
  const even = poly(coefficients.filter((_, i) => (n - i) % 2 === 0));
  const odd = poly(coefficients.filter((_, i) => (n - i) % 2 === 1));
  const common = polynomialGcd(even, odd);
  if (common.coefficients.length === 1) return false;
  const roots = isolateRealRoots(common);
  if (roots.status !== "qualified") throw new Error("Auxiliary roots exceed the exact isolation budget.");
  return roots.intervals.some(root => root.exactUpper.numerator < 0n);
}
