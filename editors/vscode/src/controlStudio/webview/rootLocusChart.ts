import type { DeclaredRootLocusResult } from "../declaredAnalysis";
import { formatEngineeringNumber } from "../chartContract";

export interface RootLocusColors { path: string; inspected: string; selected: string; foreground: string }
export type RootLocusSource = Pick<DeclaredRootLocusResult, "parameter" | "selectedValue" | "branches" | "path"> & { parameterUnit?: string };
type Gain = { gain: number; poles: readonly { real: number; imaginary: number }[] };

/** Joins each pole to the nearest pole at the next gain, so that every branch of the locus is one path of
 * [real, imaginary, gain] points. A change in the number of poles starts new branches. */
export function rootLocusBranches(points: readonly Gain[]): number[][][] {
  const branches: number[][][] = [];
  let open: { pole: { real: number; imaginary: number }; branch: number[][] }[] = [];
  for (const point of points) {
    const poles = point.poles.filter(pole => Number.isFinite(pole.real) && Number.isFinite(pole.imaginary));
    if (open.length !== poles.length) {
      open = poles.map(pole => { const branch = [[pole.real, pole.imaginary, point.gain]]; branches.push(branch); return { pole, branch }; });
      continue;
    }
    const remaining = [...poles];
    open = open.map(({ pole: last, branch }) => {
      const distance = (pole: { real: number; imaginary: number }) => Math.hypot(pole.real - last.real, pole.imaginary - last.imaginary);
      const nearest = remaining.reduce((best, pole, index) => distance(pole) < distance(remaining[best]) ? index : best, 0);
      const [pole] = remaining.splice(nearest, 1);
      branch.push([pole.real, pole.imaginary, point.gain]);
      return { pole, branch };
    });
  }
  return branches;
}

/** Root-locus series: the pole paths as the parameter varies, the inspected gains with their labels and the
 * selected candidate as its own legend entry. Without a separate path, the swept gains form the paths. */
export function rootLocusChartSeries(source: RootLocusSource, colors: RootLocusColors): Record<string, unknown>[] {
  const name = (gain: number) => `${source.parameter} = ${formatEngineeringNumber(gain)}`;
  const traced = source.path?.length ? source.path : source.branches;
  const range = traced.length ? ` ${formatEngineeringNumber(traced[0].gain)}–${formatEngineeringNumber(traced[traced.length - 1].gain)}${source.parameterUnit ? ` ${source.parameterUnit}` : ""}` : "";
  // A label sits above and right of a pole on the real axis, where a branch may also leave vertically, and beside
  // a pole off it: never on a path.
  const points = (gains: readonly Gain[]) => gains.flatMap(gain => gain.poles.map(pole => ({ value: [pole.real, pole.imaginary, gain.gain],
    label: { position: Math.abs(pole.imaginary) > 1e-9 ? "right" : [8, -18] } })));
  const label = { show: true, color: colors.foreground, formatter: (item: { value: number[] }) => name(item.value[2]) };
  const inspected = source.path?.length ? source.branches.filter(gain => gain.gain !== source.selectedValue) : [];
  return [
    { name: `Pole paths, ${source.parameter}${range}`, type: "line", showSymbol: false, connectNulls: false, silent: true,
      lineStyle: { width: 2, color: colors.path }, itemStyle: { color: colors.path },
      data: rootLocusBranches(traced).flatMap((branch, index) => index ? [[null, null, null], ...branch] : branch) },
    ...(inspected.length ? [{ name: "Inspected gains", type: "scatter", symbol: "circle", symbolSize: 10,
      itemStyle: { color: colors.inspected }, label, data: points(inspected) }] : []),
    { name: `Selected ${name(source.selectedValue)}`, type: "scatter", symbol: "diamond", symbolSize: 16,
      itemStyle: { color: colors.selected }, label,
      data: points(source.branches.filter(gain => gain.gain === source.selectedValue)) },
  ];
}

/** Item tooltip naming the gain and the pole it marks. */
export function rootLocusTooltip(source: RootLocusSource) {
  return { trigger: "item", confine: true, formatter: (item: { value?: number[] }) => {
    const [real, imaginary, gain] = item.value ?? [];
    return `${source.parameter} = ${formatEngineeringNumber(gain)}${source.parameterUnit ? ` ${source.parameterUnit}` : ""}<br/>`
      + `Pole ${formatEngineeringNumber(real)} ${imaginary < 0 ? "−" : "+"} ${formatEngineeringNumber(Math.abs(imaginary))}j`;
  } };
}
