import { decodeDeclaredAnalysis, type DeclaredCoefficient, type DeclaredControlAnalysis } from "./declaredAnalysis";
import { auxiliaryDerivativeRow } from "./routhSpecialCases";
import { hurwitzPositiveInterval } from "./routhRegions";
import { exactRouthSigns } from "./routhExact";

export type RouthStatus = "qualified" | "boundary" | "uncertain" | "rejected";
export type RouthSign = "positive" | "negative" | "zero";

export interface RouthCalculation {
  column: number;
  a0: number;
  aNext: number;
  b0: number;
  bNext: number;
  result: number;
}
export interface RouthRow {
  power: number;
  entries: readonly number[];
  calculations: readonly RouthCalculation[];
}
export interface RouthIssue {
  kind: "invalid-polynomial" | "origin-pole" | "zero-pivot" | "all-zero-row" | "near-zero-pivot" | "near-zero-entry" | "numerical-overflow";
  explanation: string;
}
export interface RouthPolynomialAnalysis {
  status: RouthStatus;
  coefficients: readonly number[];
  rows: readonly RouthRow[];
  firstColumn: readonly { value: number; sign: RouthSign }[];
  signChanges: number | null;
  stable: boolean | null;
  interpretation: string;
  issues: readonly RouthIssue[];
  symbolicRows?: readonly { power: number; entries: readonly string[] }[];
}
export interface RouthInterval {
  status: "qualified" | "unsupported" | "uncertain";
  kind: "open" | "empty" | "all-real" | "unavailable" | "union";
  lower?: number;
  upper?: number;
  intervals?: readonly { lower?: number; upper?: number; lowerBoundary?: number; upperBoundary?: number }[];
  boundaries?: readonly { polynomial: readonly string[]; lower: string; upper: string }[];
  inequalities: readonly string[];
  derivation: string;
}
export interface DeclaredRouthResult {
  parameter: string;
  parameterUnit: string;
  selectedValue: number;
  allowedDomain: { minimum: number; maximum: number };
  fixedParameters: Readonly<Record<string, { value: number; unit: string }>>;
  polynomial: RouthPolynomialAnalysis;
  table: readonly RouthRow[];
  interval: RouthInterval;
  provenance: string;
  scope: string;
}

const RELATIVE_PIVOT_TOLERANCE = 1e-12;

/** Constructs an auditable Routh table without epsilon substitutions for singular cases. */
export function analyzeRouthPolynomial(coefficients: readonly number[]): RouthPolynomialAnalysis {
  const original = [...coefficients];
  if (coefficients.length < 2 || coefficients.length > 17 || !coefficients.every(Number.isFinite)
      || coefficients[0] === 0 || !coefficients.some(value => value !== 0)) {
    return rejected(original, "The polynomial must have real finite descending coefficients, degree 1–16, and a nonzero leading coefficient.");
  }
  const degree = coefficients.length - 1;
  const columns = Math.ceil((degree + 1) / 2);
  const table = Array.from({ length: degree + 1 }, () => Array<number>(columns).fill(0));
  for (let column = 0; column < columns; column += 1) {
    table[0][column] = coefficients[column * 2] ?? 0;
    table[1][column] = coefficients[column * 2 + 1] ?? 0;
  }
  const calculations: RouthCalculation[][] = Array.from({ length: degree + 1 }, () => []);
  const issues: RouthIssue[] = [];
  if (coefficients[coefficients.length - 1] === 0) {
    issues.push({ kind: "origin-pole", explanation: "The zero constant coefficient establishes an origin pole, so asymptotic stability is excluded." });
  }
  const scale = Math.max(...coefficients.map(value => Math.abs(value)));
  let computedRows = 2;
  for (let row = 2; row <= degree; row += 1) {
    const prior = table[row - 2];
    let pivotRow = table[row - 1];
    if (pivotRow.every(value => value === 0)) {
      issues.push({ kind: "all-zero-row", explanation: `The s^${degree - row + 1} row is all zero; its auxiliary-polynomial case requires separate analysis.` });
      table[row - 1] = auxiliaryDerivativeRow(prior, degree - row + 2, columns);
      pivotRow = table[row - 1];
    }
    const pivot = table[row - 1][0];
    if (pivot === 0) {
      issues.push({ kind: "zero-pivot", explanation: `The s^${degree - row + 1} row has a zero first-column pivot; no epsilon substitution was made.` });
      break;
    }
    if (Math.abs(pivot) <= scale * RELATIVE_PIVOT_TOLERANCE) {
      issues.push({ kind: "near-zero-pivot", explanation: `The s^${degree - row + 1} first-column pivot is too small relative to the polynomial scale for a confident count.` });
      break;
    }
    for (let column = 0; column < columns - 1; column += 1) {
      const result = (pivot * prior[column + 1] - prior[0] * pivotRow[column + 1]) / pivot;
      calculations[row].push({ column, a0: prior[0], aNext: prior[column + 1], b0: pivot, bNext: pivotRow[column + 1], result });
      if (!Number.isFinite(result)) {
        issues.push({ kind: "numerical-overflow", explanation: `The s^${degree - row} row overflowed or lost finite numerical representation.` });
        break;
      }
      table[row][column] = result;
    }
    if (issues.some(issue => issue.kind === "numerical-overflow")) break;
    computedRows = row + 1;
    if (table[row].every(value => value === 0)) {
      issues.push({ kind: "all-zero-row", explanation: `The s^${degree - row} row is all zero; its auxiliary-polynomial case requires separate analysis.` });
      break;
    }
  }
  const rows = table.slice(0, computedRows).map((entries, index) => ({ power: degree - index, entries: [...entries], calculations: [...calculations[index]] }));
  let firstColumn = rows.map(row => ({ value: row.entries[0], sign: signOf(row.entries[0]) }));
  const special = issues.some(issue => issue.kind === "zero-pivot" || issue.kind === "all-zero-row" || issue.kind === "near-zero-pivot") ? exactRouthSigns(coefficients) : undefined;
  if (special && !special.uncertain && (special.zeroPivot || special.allZeroRow) && special.signs.every(value => value !== 0)) {
    if (special.allZeroRow && !issues.some(issue => issue.kind === "all-zero-row")) {
      issues.push({ kind: "all-zero-row", explanation: "Exact arithmetic resolves the numerically small pivot as an auxiliary-polynomial all-zero row." });
    } else if (special.zeroPivot && !issues.some(issue => issue.kind === "zero-pivot")) {
      issues.push({ kind: "zero-pivot", explanation: "Exact arithmetic resolves the numerically small pivot as a zero pivot; the formal positive infinitesimal determines its limiting sign." });
    }
    firstColumn = special.signs.map(value => ({ value, sign: signOf(value) }));
    const signChanges = firstColumn.slice(1).reduce((count, entry, index) => count + (entry.sign !== firstColumn[index].sign ? 1 : 0), 0);
    const status: RouthStatus = special.boundary ? "boundary" : "qualified";
    return { status, coefficients: original, rows, symbolicRows: special.rows, firstColumn, signChanges, stable: status === "qualified" && signChanges === 0,
      interpretation: `${issues.some(issue => issue.kind === "origin-pole") ? "An origin pole excludes strict stability. " : special.boundary ? "The auxiliary polynomial establishes imaginary-axis poles and excludes strict stability. " : ""}${signChanges} right-half-plane poles established by the exact formal-ε Routh recurrence.`, issues };
  }
  if (!issues.some(issue => issue.kind === "near-zero-pivot" || issue.kind === "numerical-overflow")
      && firstColumn.some(entry => entry.value !== 0 && Math.abs(entry.value) <= scale * RELATIVE_PIVOT_TOLERANCE)) {
    issues.push({ kind: "near-zero-entry", explanation: "A first-column entry is near zero relative to the polynomial scale, so the sign-change count is numerically uncertain." });
  }
  const status = issues.some(issue => issue.kind === "near-zero-pivot" || issue.kind === "near-zero-entry" || issue.kind === "numerical-overflow") ? "uncertain"
    : issues.some(issue => issue.kind === "origin-pole") ? "boundary"
    : issues.length > 0 ? "uncertain" : "qualified";
  if (status !== "qualified") {
    return { status, coefficients: original, rows, firstColumn, signChanges: null,
      stable: issues.some(issue => issue.kind === "origin-pole") ? false : null,
      interpretation: issues.map(issue => issue.explanation).join(" "), issues };
  }
  let signChanges = 0;
  for (let index = 1; index < firstColumn.length; index += 1) {
    if (firstColumn[index].sign !== firstColumn[index - 1].sign) signChanges += 1;
  }
  return { status, coefficients: original, rows, firstColumn, signChanges, stable: signChanges === 0,
    interpretation: signChanges === 0 ? "No right-half-plane poles were established by the Routh first column; the declared polynomial is asymptotically stable."
      : `${signChanges} right-half-plane pole${signChanges === 1 ? "" : "s"} established by first-column sign changes.`, issues };
}

/** Calculates the declared delay-free characteristic polynomial Dp Dc + Np Nc and its limited affine interval. */
export function calculateDeclaredRouth(input: DeclaredControlAnalysis, selectedValue?: number): DeclaredRouthResult {
  const analysis = decodeDeclaredAnalysis(input);
  if (!analysis) throw new Error("The declared model or parameter sweep is invalid.");
  const parameter = analysis.rootLocus.parameter;
  const selected = selectedValue ?? analysis.rootLocus.selectedValue;
  if (!Number.isFinite(selected)) throw new Error("The selected parameter value must be finite.");
  const configured = analysis.controller.parameters[parameter];
  const fixedParameters = Object.fromEntries(Object.entries(analysis.controller.parameters)
    .filter(([name]) => name !== parameter).map(([name, value]) => [name, { value: value.value, unit: value.unit }]));
  const provenance = `${analysis.plant.source} model ${analysis.plant.revision} · ${analysis.plant.validation}`;
  const scope = "Declared continuous-time, delay-free scalar negative-feedback model; nonlinear limits, sample/hold, and the running controller are not changed.";
  if (analysis.plant.delaySeconds > 0) {
    const polynomial = rejected([], "Routh-Hurwitz analysis does not approximate declared delay models.");
    return { parameter, parameterUnit: configured.unit, selectedValue: selected,
      allowedDomain: { minimum: configured.minimum, maximum: configured.maximum }, fixedParameters,
      polynomial, table: polynomial.rows, interval: unavailable("Declared delay is unsupported for this criterion."), provenance, scope };
  }
  const selectedPolynomial = characteristic(analysis, parameter, selected);
  const polynomial = analyzeRouthPolynomial(selectedPolynomial);
  const interval = affineInterval(analysis, parameter);
  return { parameter, parameterUnit: configured.unit, selectedValue: selected,
    allowedDomain: { minimum: configured.minimum, maximum: configured.maximum }, fixedParameters,
    polynomial, table: polynomial.rows, interval, provenance, scope };
}

function characteristic(analysis: DeclaredControlAnalysis, parameter: string, selected: number): number[] {
  const value = (coefficient: DeclaredCoefficient): number => typeof coefficient === "number" ? coefficient
    : (coefficient.scale ?? 1) * (coefficient.parameter === parameter ? selected : analysis.controller.parameters[coefficient.parameter].value);
  return add(multiply(analysis.plant.denominator, analysis.controller.denominator.map(value)),
    multiply(analysis.plant.numerator, analysis.controller.numerator.map(value)));
}

function affineInterval(analysis: DeclaredControlAnalysis, parameter: string): RouthInterval {
  const coefficients = addAffine(multiplyAffine(analysis.plant.denominator, analysis.controller.denominator, analysis, parameter),
    multiplyAffine(analysis.plant.numerator, analysis.controller.numerator, analysis, parameter));
  if (!coefficients.every(coefficient => Number.isFinite(coefficient.constant) && Number.isFinite(coefficient.slope))) {
    return { status: "uncertain", kind: "unavailable", inequalities: [], derivation: "The affine coefficient calculation is not finite." };
  }
  const degree = coefficients.length - 1;
  if (degree > 2) return hurwitzPositiveInterval(coefficients);
  const leading = coefficients[0];
  if (leading.slope !== 0) return hurwitzPositiveInterval(coefficients);
  if (leading.constant === 0) return unavailable("The characteristic leading coefficient is zero.");
  const factor = leading.constant < 0 ? -1 : 1;
  const constraints = coefficients.slice(1).map((coefficient, index) => ({
    constant: coefficient.constant * factor, slope: coefficient.slope * factor,
    label: `a${degree - index - 1}`,
  }));
  const solved = constraints.map(solvePositive);
  if (solved.some(result => result.kind === "uncertain")) return { status: "uncertain", kind: "unavailable",
    inequalities: solved.map(result => result.inequality), derivation: "A coefficient inequality is too ill-conditioned to classify confidently." };
  if (solved.some(result => result.kind === "empty")) return { status: "qualified", kind: "empty",
    inequalities: solved.map(result => result.inequality), derivation: "At least one strict Routh coefficient inequality is impossible." };
  const lower = Math.max(...solved.map(result => result.lower ?? -Infinity));
  const upper = Math.min(...solved.map(result => result.upper ?? Infinity));
  if (lower >= upper) return { status: "qualified", kind: "empty", inequalities: solved.map(result => result.inequality),
    derivation: "The strict coefficient half-lines do not intersect." };
  if (lower === -Infinity && upper === Infinity) return { status: "qualified", kind: "all-real", inequalities: solved.map(result => result.inequality),
    derivation: "Every strict coefficient inequality holds over the real parameter axis." };
  return { status: "qualified", kind: "open", ...(lower === -Infinity ? {} : { lower }), ...(upper === Infinity ? {} : { upper }),
    inequalities: solved.map(result => result.inequality), derivation: "Intersected strict coefficient-sign half-lines after normalizing the leading coefficient positive." };
}

function multiply(a: readonly number[], b: readonly number[]): number[] {
  const result = Array<number>(a.length + b.length - 1).fill(0);
  a.forEach((left, i) => b.forEach((right, j) => { result[i + j] += left * right; }));
  return result;
}
function add(a: readonly number[], b: readonly number[]): number[] {
  const size = Math.max(a.length, b.length);
  return Array.from({ length: size }, (_, index) => (a[index - (size - a.length)] ?? 0) + (b[index - (size - b.length)] ?? 0));
}
type Affine = { constant: number; slope: number };
function multiplyAffine(plant: readonly number[], controller: readonly DeclaredCoefficient[], analysis: DeclaredControlAnalysis, parameter: string): Affine[] {
  const result = Array.from({ length: plant.length + controller.length - 1 }, (): Affine => ({ constant: 0, slope: 0 }));
  plant.forEach((left, i) => controller.forEach((right, j) => {
    const term = affineCoefficient(right, analysis, parameter);
    result[i + j].constant += left * term.constant;
    result[i + j].slope += left * term.slope;
  }));
  return result;
}
function addAffine(a: readonly Affine[], b: readonly Affine[]): Affine[] {
  const size = Math.max(a.length, b.length);
  return Array.from({ length: size }, (_, index) => {
    const left = a[index - (size - a.length)] ?? { constant: 0, slope: 0 };
    const right = b[index - (size - b.length)] ?? { constant: 0, slope: 0 };
    return { constant: left.constant + right.constant, slope: left.slope + right.slope };
  });
}
function affineCoefficient(coefficient: DeclaredCoefficient, analysis: DeclaredControlAnalysis, parameter: string): Affine {
  if (typeof coefficient === "number") return { constant: coefficient, slope: 0 };
  const scale = coefficient.scale ?? 1;
  return coefficient.parameter === parameter ? { constant: 0, slope: scale }
    : { constant: scale * analysis.controller.parameters[coefficient.parameter].value, slope: 0 };
}
function solvePositive(value: { constant: number; slope: number; label: string }): { kind: "bound" | "all" | "empty" | "uncertain"; lower?: number; upper?: number; inequality: string } {
  const tolerance = Math.max(1, Math.abs(value.constant), Math.abs(value.slope)) * RELATIVE_PIVOT_TOLERANCE;
  if (!Number.isFinite(value.constant) || !Number.isFinite(value.slope)) {
    return { kind: "uncertain", inequality: `${value.label}(p) > 0 is not finite` };
  }
  if (value.slope === 0) {
    return value.constant > 0 ? { kind: "all", inequality: `${value.label}(p) = ${value.constant} > 0` }
      : { kind: "empty", inequality: `${value.label}(p) = ${value.constant} > 0 is impossible` };
  }
  if (Math.abs(value.slope) <= tolerance) {
    return { kind: "uncertain", inequality: `${value.label}(p) > 0 has an ill-conditioned nonzero parameter slope` };
  }
  const bound = -value.constant / value.slope;
  if (!Number.isFinite(bound)) return { kind: "uncertain", inequality: `${value.label}(p) > 0 has a non-finite bound` };
  return value.slope > 0 ? { kind: "bound", lower: bound, inequality: `${value.label}(p) > 0 requires p > ${bound}` }
    : { kind: "bound", upper: bound, inequality: `${value.label}(p) > 0 requires p < ${bound}` };
}
function unavailable(derivation: string): RouthInterval {
  return { status: "unsupported", kind: "unavailable", inequalities: [], derivation };
}
function signOf(value: number): RouthSign { return value > 0 ? "positive" : value < 0 ? "negative" : "zero"; }
function rejected(coefficients: readonly number[], explanation: string): RouthPolynomialAnalysis {
  return { status: "rejected", coefficients: [...coefficients], rows: [], firstColumn: [], signChanges: null, stable: null,
    interpretation: explanation, issues: [{ kind: "invalid-polynomial", explanation }] };
}
