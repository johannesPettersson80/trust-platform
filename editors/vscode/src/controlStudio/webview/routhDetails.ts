import type { RouthPolynomialAnalysis } from "../routhHurwitz";
import { formatEngineeringNumber } from "../chartContract";

/** The Routh result's detail line: the first-column signs of the table they were read from, then every issue that
 * shaped the count, so a replaced row or an excluded boundary is never left unexplained. */
export function routhDetailText(polynomial: Pick<RouthPolynomialAnalysis, "firstColumn" | "symbolicRows" | "issues">): string {
  const table = polynomial.symbolicRows?.length ? "the exact table" : "the table";
  return [`First-column signs of ${table}: ${polynomial.firstColumn.map(entry => entry.sign).join(", ")}.`,
    ...polynomial.issues.map(issue => issue.explanation)].join(" ");
}

/** A table cell whose exact value stays available for inspection. */
export interface InspectableCell { text: string; title: string }

/** One row per stability boundary: its polynomial, then the boundary as a decimal (a range when the exact
 * enclosure is wider than the display precision). The exact rational enclosure stays on the cell. */
export function routhBoundaryRows(boundaries: readonly { polynomial: readonly string[]; lower: string; upper: string }[]): [string, InspectableCell][] {
  const decimal = (value: string): string => {
    const [numerator, denominator = "1"] = value.split("/");
    return formatEngineeringNumber(Number(numerator) / Number(denominator));
  };
  return boundaries.map(boundary => {
    const lower = decimal(boundary.lower), upper = decimal(boundary.upper);
    return [boundary.polynomial.join(", "), { text: lower === upper ? lower : `${lower} – ${upper}`, title: `Exact enclosure [${boundary.lower}, ${boundary.upper}]` }];
  });
}
