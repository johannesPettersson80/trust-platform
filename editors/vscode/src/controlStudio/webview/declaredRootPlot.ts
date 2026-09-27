import type { DeclaredRootLocusResult } from "../declaredAnalysis";
import { formatEngineeringNumber } from "../chartContract";

export function declaredRootPlotOptions(
  result: DeclaredRootLocusResult,
  theme: { foreground: string },
  xAxis: unknown,
  yAxis: unknown
): Record<string, unknown> {
  return { animation: false,
      aria: { enabled: true, decal: { show: true } },
      tooltip: { trigger: "item", renderMode: "richText", formatter: (item: any) =>
        `${result.parameter} = ${formatEngineeringNumber(item.value[2])}\nPole: (${formatEngineeringNumber(item.value[0])}, ${formatEngineeringNumber(item.value[1])})` },
      grid: { left: 58, right: 50, top: 38, bottom: 44 },
      xAxis: xAxis,
      yAxis: yAxis,
      series: [{ type: "line", showSymbol: true, lineStyle: { width: 0 }, name: `${result.parameter} sweep`,
        data: result.branches.flatMap(branch => branch.poles.map(pole => ({
          value: [pole.real, pole.imaginary, branch.gain],
          symbol: branch.gain === result.selectedValue ? "diamond" : "circle",
          symbolSize: branch.gain === result.selectedValue ? 14 : 8,
        }))),
        label: { show: true, position: "top", color: theme.foreground,
          formatter: (item: any) => `${result.parameter}=${formatEngineeringNumber(item.value[2])}` },
      }], graphic: [] };
}
