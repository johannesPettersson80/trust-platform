// Exists because the Validate uncertainty band was a near-invisible default-grey fill
// with no upper-bound legend entry and no stated source (finding V-17), and an
// unevaluated uncertainty simply omitted the band instead of stating "Not evaluated" as
// Specification 35 S15 requires. This module builds the labelled, outlined series and the
// accessible description; chart.ts only wires the result into the ECharts option.

export interface UncertaintyPoint {
  timeSeconds: number;
  lower: number;
  upper: number;
}

export interface UncertaintyBandSeries {
  name: string;
  type: "line";
  stack?: string;
  showSymbol: boolean;
  lineStyle: { width: number; type?: "dashed" | "solid" };
  areaStyle?: { opacity: number; color: string; decal: Record<string, unknown> };
  data: ReadonlyArray<readonly [number, number]>;
  z: number;
}

/** One sentence stating whether an uncertainty range exists and, when it does, its
 * source, instead of leaving the band's meaning to colour and an internal method slug. */
export function uncertaintyBandDescription(method: string, pointCount: number): string {
  return pointCount > 0
    ? `Uncertainty band: ${method}, shaded lower-to-upper range across ${pointCount} predicted points.`
    : "Uncertainty: Not evaluated.";
}

/** Builds a labelled lower bound, upper bound, and filled+hatched range so the band reads
 * without relying on colour alone. Returns an empty array when there is nothing to plot;
 * callers must show the "Not evaluated" text explicitly rather than a zero-width band. */
export function uncertaintyBandSeries(
  method: string,
  points: readonly UncertaintyPoint[],
  color: string
): UncertaintyBandSeries[] {
  if (points.length === 0) return [];
  const lower = points.map((point) => [point.timeSeconds, point.lower] as const);
  const upper = points.map((point) => [point.timeSeconds, point.upper] as const);
  const span = points.map((point) => [point.timeSeconds, point.upper - point.lower] as const);
  return [
    { name: "Uncertainty lower bound", type: "line", stack: "uncertainty-band", showSymbol: false,
      lineStyle: { width: 1, type: "dashed" }, data: lower, z: 2 },
    { name: `Uncertainty range · ${method}`, type: "line", stack: "uncertainty-band", showSymbol: false,
      lineStyle: { width: 0 }, data: span, z: 1,
      areaStyle: { opacity: 0.22, color,
        decal: { symbol: "line", dashArrayX: [1, 0], dashArrayY: [4, 4], rotation: Math.PI / 4, color, symbolSize: 1 } } },
    { name: "Uncertainty upper bound", type: "line", showSymbol: false,
      lineStyle: { width: 1, type: "dashed" }, data: upper, z: 2 },
  ];
}
