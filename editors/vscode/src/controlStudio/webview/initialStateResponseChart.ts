import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";
import type { ModelTimeResponse } from "../generalModelTime";
import { engineeringLine } from "./chartPrimitives";

export interface InitialStateResponseColors { total: string; zeroState: string; initialCondition: string }

/** One output's initial-state response. The total is the wide primary line; its two parts are drawn
 * over it in auxiliary strokes, so a part that equals the total, such as the initial-condition
 * response under a zero input, stays visible on top of it. */
export function initialStateResponseSeries(points: ModelTimeResponse["points"], outputIndex: number, colors: InitialStateResponseColors): Record<string, unknown>[] {
  const line = (name: string, color: string, stroke: "solid" | "dashed" | "dotted", width: number, z: number, value: (point: ModelTimeResponse["points"][number]) => number) => {
    const base = engineeringLine(name, 0, points.map(point => [point.timeSeconds, value(point)]), stroke);
    return { ...base, z, lineStyle: { ...base.lineStyle, width, color }, itemStyle: { color } };
  };
  return [
    line("Zero-state input", colors.zeroState, "dashed", 2, 3, point => point.zeroState[outputIndex]),
    line("Initial condition", colors.initialCondition, "dotted", 2, 3, point => point.initialCondition[outputIndex]),
    line("Total", colors.total, "solid", 3.5, 2, point => point.total[outputIndex]),
  ];
}

/** The response table rows: time, zero-state part, initial-condition part and total. Each column has one precision, set by
 * its smallest non-zero magnitude, so a column reads in one notation. */
export function initialStateResponseRows(points: ModelTimeResponse["points"], outputIndex: number): string[][] {
  const rows = points.map(point => [point.timeSeconds, point.zeroState[outputIndex], point.initialCondition[outputIndex], point.total[outputIndex]]);
  const digits = [0, 1, 2, 3].map(column => {
    const magnitudes = rows.map(row => Math.abs(row[column])).filter(value => Number.isFinite(value) && value > 0);
    return magnitudes.length ? automaticFractionDigits(Math.min(...magnitudes)) : undefined;
  });
  return rows.map(row => row.map((value, column) => formatEngineeringNumber(value, digits[column])));
}
