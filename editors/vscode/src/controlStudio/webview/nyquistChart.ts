import { engineeringLine } from "./chartPrimitives";
import { equalUnitGeometry, type PlaneArea, type PlaneGeometry, type PlanePoint } from "./complexPlaneGeometry";

export type NyquistPoint = PlanePoint;
export type NyquistArea = PlaneArea;
export interface NyquistColors { positive: string; negative: string; critical: string; label: string }
export type NyquistGeometry = PlaneGeometry;

/** Equal units on both axes around the curve, its mirror image, −1 and the origin, so that distances to −1 read true. */
export function nyquistGeometry(points: readonly NyquistPoint[], area: NyquistArea): NyquistGeometry {
  return equalUnitGeometry([...points, { real: -1, imaginary: 0 }, { real: 0, imaginary: 0 }], area);
}

/** Both frequency branches, arrows in the direction of traversal (rising frequency on the positive branch, back
 * from high frequency to zero on its mirror image) and the critical point −1. */
export function nyquistSeries(points: readonly NyquistPoint[], colors: NyquistColors): Record<string, unknown>[] {
  const positive = points.map(point => [point.real, point.imaginary]);
  const negative = positive.map(([real, imaginary]) => [real, -imaginary]);
  const line = (name: string, data: number[][], color: string, stroke: "solid" | "dashed") => {
    const base = engineeringLine(name, 0, data, stroke);
    return { ...base, lineStyle: { ...base.lineStyle, color }, itemStyle: { color } };
  };
  const rotation = (from: number[], to: number[]) => Math.atan2(to[1] - from[1], to[0] - from[0]) * 180 / Math.PI - 90;
  const arrows = (name: string, color: string, data: { value: number[]; symbolRotate: number }[]) =>
    ({ type: "scatter", name, symbol: "arrow", symbolSize: 13, silent: true, z: 4, itemStyle: { color }, data });
  const indices = arrowIndices(points);
  return [
    line("Positive frequency", positive, colors.positive, "solid"),
    line("Negative frequency", negative, colors.negative, "dashed"),
    arrows("Positive frequency", colors.positive, indices.map(index => ({ value: positive[index], symbolRotate: rotation(positive[index], positive[index + 1]) }))),
    arrows("Negative frequency", colors.negative, indices.map(index => ({ value: negative[index], symbolRotate: rotation(negative[index + 1], negative[index]) }))),
    { type: "scatter", name: "−1", data: [[-1, 0]], symbol: "circle", symbolSize: 11, z: 5,
      itemStyle: { color: colors.critical, borderColor: colors.label, borderWidth: 1 },
      label: { show: true, formatter: "−1", position: "bottom", color: colors.label } },
  ];
}

/** Arrow positions along the outer part of the curve, before its magnitude first falls below a quarter of its
 * largest value: evenly spaced by length there, clear of the tail that winds into the origin. */
function arrowIndices(points: readonly NyquistPoint[]): number[] {
  const magnitude = points.map(point => Math.hypot(point.real, point.imaginary));
  const largest = Math.max(...magnitude.filter(Number.isFinite));
  const fall = magnitude.findIndex(value => value < 0.25 * largest);
  const end = Math.min(fall < 0 ? points.length - 1 : fall, points.length - 1);
  if (end < 2) return [];
  const length = [0];
  for (let index = 1; index <= end; index += 1)
    length.push(length[index - 1] + Math.hypot(points[index].real - points[index - 1].real, points[index].imaginary - points[index - 1].imaginary));
  const indices = [0.15, 0.4, 0.65, 0.9].map(fraction => Math.min(end - 1, Math.max(0, length.findIndex(value => value >= fraction * length[end]))));
  return indices.filter((index, position) => indices.indexOf(index) === position);
}
