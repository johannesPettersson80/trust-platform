import type { ControlLoopDefinition } from "../loopDefinition";
import { auxiliaryTraceStyle, chartTheme, engineeringLine, formatChartValue } from "./chartPrimitives";
import type { SignalResponseGroup } from "./threePageLiveSignals";
import { experimentSignalBindings } from "./threePageRecordedSignals";

/** One chart series per displayed trace; each response group owns its own axis pair. An auxiliary
 * trace's stroke comes from its position among the loop's declared auxiliary signals, so it does
 * not change when the selection changes. The applied output is held between updates, so it is drawn
 * as steps. */
export function responseChartSeries(theme: ReturnType<typeof chartTheme>, loop: ControlLoopDefinition | null,
  groups: readonly SignalResponseGroup[]) {
  const auxiliaryIds = loop ? experimentSignalBindings(loop).map(signal => signal.id).filter(id => !["sp", "pv", "mv"].includes(id)) : [];
  return groups.flatMap((group, index) => group.series.map(item => {
    const id = item.signalId;
    const auxiliary = !item.prediction && id && id !== "sp" && id !== "pv" && id !== "mv" ? auxiliaryTraceStyle(theme, auxiliaryIds.indexOf(id)) : undefined;
    const color = item.prediction ? theme.auxiliaryOrange : id === "sp" || id === "pv" || id === "mv" ? theme[id] : auxiliary?.color;
    const line = engineeringLine(item.name, index, item.points, auxiliary?.type ?? (item.dashed ? "dashed" : "solid"));
    return { ...line, xAxisIndex: index, ...(id ? { id } : {}), ...(id === "mv" ? { step: "end" as const } : {}),
      ...(color ? { itemStyle: { color }, lineStyle: { ...line.lineStyle, color } } : {}) };
  }));
}

/** Hovering the response plot lists every displayed trace at the nearest recorded time. */
export function responseChartTooltip(): { trigger: "axis"; confine?: boolean; valueFormatter?: (value: unknown) => string } {
  return { trigger: "axis", confine: true, valueFormatter: formatChartValue };
}
