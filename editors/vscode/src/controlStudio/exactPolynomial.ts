/** Bounded exact arithmetic for one real parameter. Coefficients are descending. */
export interface Rational { readonly numerator: bigint; readonly denominator: bigint }
export interface RationalPolynomial { readonly coefficients: readonly Rational[] }
export interface RootInterval { readonly lower: number; readonly upper: number; readonly exactLower: Rational; readonly exactUpper: Rational }
export interface RootIsolation { readonly status: "qualified" | "uncertain"; readonly intervals: readonly RootInterval[]; readonly reason?: string }
const MAX_BITS = 32768;
export const rationalZero = rational(0n);
export const rationalOne = rational(1n);
export function rational(numerator: bigint, denominator = 1n): Rational {
  if (!denominator) throw new Error("Exact division by zero.");
  if (denominator < 0n) { numerator = -numerator; denominator = -denominator; }
  const g = gcd(abs(numerator), denominator);
  const value = { numerator: numerator / g, denominator: denominator / g };
  if (abs(value.numerator).toString(2).length > MAX_BITS || value.denominator.toString(2).length > MAX_BITS) throw new Error("Exact arithmetic exceeded its coefficient budget.");
  return value;
}
export function decimalRational(value: number): Rational {
  if (!Number.isFinite(value)) throw new Error("Polynomial coefficients must be finite.");
  const [mantissa, exponent = "0"] = String(value).toLowerCase().split("e");
  const [whole, fraction = ""] = mantissa.split("."); const power = Number(exponent) - fraction.length;
  return power >= 0 ? rational(BigInt(whole + fraction) * 10n ** BigInt(power)) : rational(BigInt(whole + fraction), 10n ** BigInt(-power));
}
export function rationalAdd(a: Rational, b: Rational): Rational { return rational(a.numerator * b.denominator + b.numerator * a.denominator, a.denominator * b.denominator); }
export function rationalNegate(a: Rational): Rational { return { numerator: -a.numerator, denominator: a.denominator }; }
export function rationalMultiply(a: Rational, b: Rational): Rational { return rational(a.numerator * b.numerator, a.denominator * b.denominator); }
export function rationalDivide(a: Rational, b: Rational): Rational { return rational(a.numerator * b.denominator, a.denominator * b.numerator); }
export function rationalCompare(a: Rational, b: Rational): number { return sign(a.numerator * b.denominator - b.numerator * a.denominator); }
export function rationalNumber(a: Rational): number { return Number(a.numerator) / Number(a.denominator); }
export function rationalText(a: Rational): string { return a.denominator === 1n ? String(a.numerator) : `${a.numerator}/${a.denominator}`; }
export function makeRationalPolynomial(coefficients: readonly number[]): RationalPolynomial { return polynomial(coefficients.map(decimalRational)); }
export function polynomial(coefficients: readonly Rational[]): RationalPolynomial {
  let first = 0; while (first < coefficients.length - 1 && coefficients[first].numerator === 0n) first++;
  if (coefficients.length > 257) throw new Error("Exact polynomial degree exceeds 256.");
  return { coefficients: coefficients.length ? coefficients.slice(first) : [rationalZero] };
}
export function polynomialZero(p: RationalPolynomial): boolean { return p.coefficients.every(c => c.numerator === 0n); }
export function polynomialNegate(p: RationalPolynomial): RationalPolynomial { return polynomial(p.coefficients.map(rationalNegate)); }
export function polynomialAdd(a: RationalPolynomial, b: RationalPolynomial): RationalPolynomial {
  const n = Math.max(a.coefficients.length, b.coefficients.length);
  return polynomial(Array.from({ length: n }, (_, i) => rationalAdd(a.coefficients[i - n + a.coefficients.length] ?? rationalZero, b.coefficients[i - n + b.coefficients.length] ?? rationalZero)));
}
export function polynomialSubtract(a: RationalPolynomial, b: RationalPolynomial): RationalPolynomial { return polynomialAdd(a, polynomialNegate(b)); }
export function polynomialMultiply(a: RationalPolynomial, b: RationalPolynomial): RationalPolynomial {
  const c = Array<Rational>(a.coefficients.length + b.coefficients.length - 1).fill(rationalZero);
  a.coefficients.forEach((v, i) => b.coefficients.forEach((w, j) => { c[i + j] = rationalAdd(c[i + j], rationalMultiply(v, w)); })); return polynomial(c);
}
export function polynomialDivide(a: RationalPolynomial, b: RationalPolynomial): { quotient: RationalPolynomial; remainder: RationalPolynomial } {
  if (polynomialZero(b)) throw new Error("Exact polynomial division by zero.");
  let remainder = a;
  const result = Array<Rational>(Math.max(1, a.coefficients.length - b.coefficients.length + 1)).fill(rationalZero);
  while (!polynomialZero(remainder) && remainder.coefficients.length >= b.coefficients.length) {
    const power = remainder.coefficients.length - b.coefficients.length, factor = rationalDivide(remainder.coefficients[0], b.coefficients[0]);
    result[result.length - 1 - power] = factor;
    remainder = polynomial(remainder.coefficients.map((v, i) => rationalAdd(v, rationalNegate(rationalMultiply(factor, b.coefficients[i] ?? rationalZero)))).slice(1));
  }
  return { quotient: polynomial(result), remainder };
}
export function polynomialExactDivide(a: RationalPolynomial, b: RationalPolynomial): RationalPolynomial | undefined { const result = polynomialDivide(a, b); return polynomialZero(result.remainder) ? result.quotient : undefined; }
export function polynomialDerivative(p: RationalPolynomial): RationalPolynomial { return polynomial(p.coefficients.slice(0, -1).map((c, i) => rationalMultiply(c, rational(BigInt(p.coefficients.length - 1 - i))))); }
export function polynomialEvaluate(p: RationalPolynomial, x: Rational): Rational { return p.coefficients.reduce((v, c) => rationalAdd(rationalMultiply(v, x), c), rationalZero); }
/** Positive scaling preserves Sturm signs. */
export function polynomialPrimitive(p: RationalPolynomial): RationalPolynomial {
  let denominator = 1n;
  for (const c of p.coefficients) denominator = denominator / gcd(denominator, c.denominator) * c.denominator;
  const values = p.coefficients.map(c => c.numerator * (denominator / c.denominator));
  const common = values.reduce((v, c) => gcd(v, abs(c)), 0n) || 1n;
  return polynomial(values.map(v => rational(v / common)));
}
export function polynomialGcd(a: RationalPolynomial, b: RationalPolynomial): RationalPolynomial {
  while (!polynomialZero(b)) { const next = polynomialPrimitive(polynomialDivide(a, b).remainder); a = b; b = next; } return polynomialPrimitive(a);
}
export function polynomialSquareFree(p: RationalPolynomial): RationalPolynomial {
  if (p.coefficients.length <= 1) return p;
  return polynomialPrimitive(polynomialExactDivide(p, polynomialGcd(p, polynomialDerivative(p)))!);
}
export function sturmSequence(input: RationalPolynomial): readonly RationalPolynomial[] {
  const p = polynomialSquareFree(polynomialPrimitive(input));
  if (p.coefficients.length <= 1) return [p];
  const sequence = [p, polynomialPrimitive(polynomialDerivative(p))];
  while (sequence[sequence.length - 1].coefficients.length > 1) {
    const next = polynomialPrimitive(polynomialNegate(polynomialDivide(sequence[sequence.length - 2], sequence[sequence.length - 1]).remainder));
    if (polynomialZero(next)) break; sequence.push(next);
  } return sequence;
}
export function sturmVariations(sequence: readonly RationalPolynomial[], point: Rational): number {
  const signs = sequence.map(p => sign(polynomialEvaluate(p, point).numerator)).filter(s => s !== 0);
  return signs.slice(1).reduce((n, s, i) => n + Number(s !== signs[i]), 0);
}
/** Each disjoint rational enclosure contains exactly one distinct real root. */
export function isolateRealRoots(input: RationalPolynomial): RootIsolation {
  const intervals: RootInterval[] = [];
  try {
    if (polynomialZero(input)) throw new Error("The zero polynomial has infinitely many roots.");
    if (input.coefficients.length === 1) return { status: "qualified", intervals };
    const sequence = sturmSequence(input), p = sequence[0], lead = p.coefficients[0];
    let bound = 2n;
    for (const c of p.coefficients.slice(1)) { const ratio = rationalDivide(c, lead); const ceil = (abs(ratio.numerator) + ratio.denominator - 1n) / ratio.denominator + 2n; if (ceil > bound) bound = ceil; }
    let nodes = 0;
    const emit = (a: Rational, b: Rational) => {
      const lower = rationalNumber(a), upper = rationalNumber(b);
      if (!Number.isFinite(lower) || !Number.isFinite(upper)) throw new Error("A root lies outside finite display coordinates.");
      intervals.push({ lower, upper, exactLower: a, exactUpper: b });
    };
    const isolate = (a: Rational, b: Rational, va: number, vb: number, depth: number): void => {
      if (++nodes > 50000 || depth > 256) throw new Error("Root isolation exceeded its subdivision budget.");
      const count = va - vb - Number(polynomialEvaluate(p, b).numerator === 0n);
      if (!count) return; if (count < 0) throw new Error("Inconsistent Sturm certificate.");
      const width = rationalAdd(b, rationalNegate(a));
      if (count === 1 && rationalCompare(width, rational(1n, 1n << 48n)) <= 0 && polynomialEvaluate(p, a).numerator !== 0n && polynomialEvaluate(p, b).numerator !== 0n) { emit(a, b); return; }
      const middle = rationalDivide(rationalAdd(a, b), rational(2n)), vm = sturmVariations(sequence, middle);
      isolate(a, middle, va, vm, depth + 1);
      if (polynomialEvaluate(p, middle).numerator === 0n) emit(middle, middle);
      isolate(middle, b, vm, vb, depth + 1);
    };
    const a = rational(-bound), b = rational(bound); isolate(a, b, sturmVariations(sequence, a), sturmVariations(sequence, b), 0);
    return { status: "qualified", intervals };
  } catch (error) { return { status: "uncertain", intervals: [], reason: error instanceof Error ? error.message : String(error) }; }
}
/** Fraction-free elimination with checked exact division at each Bareiss step. */
export function polynomialDeterminant(input: readonly (readonly RationalPolynomial[])[]): RationalPolynomial {
  const n = input.length;
  if (!n || n > 16 || input.some(row => row.length !== n)) throw new Error("Expected a square matrix of order 1–16.");
  const m = input.map(row => [...row]); let previous = makeRationalPolynomial([1]), parity = 1;
  for (let k = 0; k < n - 1; k++) {
    const pivotRow = m.findIndex((row, i) => i >= k && !polynomialZero(row[k]));
    if (pivotRow < 0) return makeRationalPolynomial([0]);
    if (pivotRow !== k) { [m[k], m[pivotRow]] = [m[pivotRow], m[k]]; parity *= -1; }
    const pivot = m[k][k];
    for (let i = k + 1; i < n; i++) for (let j = k + 1; j < n; j++) {
      const numerator = polynomialSubtract(polynomialMultiply(pivot, m[i][j]), polynomialMultiply(m[i][k], m[k][j]));
      const value = polynomialExactDivide(numerator, previous); if (!value) throw new Error("Fraction-free determinant lost exact divisibility."); m[i][j] = value;
    }
    for (let i = k + 1; i < n; i++) m[i][k] = makeRationalPolynomial([0]); previous = pivot;
  } return parity === 1 ? m[n - 1][n - 1] : polynomialNegate(m[n - 1][n - 1]);
}
function sign(value: bigint): -1 | 0 | 1 { return value < 0n ? -1 : value > 0n ? 1 : 0; }
function abs(value: bigint): bigint { return value < 0n ? -value : value; }
function gcd(a: bigint, b: bigint): bigint { while (b) { const next = a % b; a = b; b = next; } return a; }
