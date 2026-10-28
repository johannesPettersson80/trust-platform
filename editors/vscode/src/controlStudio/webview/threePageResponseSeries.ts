import type { PlotSignal } from "../generated/OpenResult";
import { auxiliaryTraceStyle, chartTheme, engineeringLine, formatChartValue } from "./chartPrimitives";
import { isAuxiliary } from "./plotSignalKinds";
import type { SignalResponseGroup } from "./threePageLiveSignals";

/** One chart series per displayed trace; each response group owns its own axis pair. A trace's colour follows the kind the engine gives its
 * signal (`liveSides.plotSignals`, `PlotSignal.kind`): the setpoint, the measurement and the applied output keep a colour each, and every
 * other signal an auxiliary stroke from its position among the signals the engine lists, so it does not change when the selection changes.
 * The applied output is held between updates, so it is drawn as steps. */
export function responseChartSeries(theme: ReturnType<typeof chartTheme>, signals: readonly Pick<PlotSignal, "id" | "kind">[],
  groups: readonly SignalResponseGroup[]) {
  const auxiliaryIds = signals.filter(isAuxiliary).map(signal => signal.id);
  return groups.flatMap((group, index) => group.series.map(item => {
    const id = item.signalId;
    const signal = signals.find(listed => listed.id === id);
    const auxiliary = !item.prediction && id && signal && isAuxiliary(signal) ? auxiliaryTraceStyle(theme, auxiliaryIds.indexOf(id)) : undefined;
    const fixed = signal?.kind === "setpoint" ? theme.sp : signal?.kind === "measurement" ? theme.pv : signal?.kind === "appliedOutput" ? theme.mv : undefined;
    const color = item.prediction ? theme.auxiliaryOrange : fixed ?? auxiliary?.color;
    const line = engineeringLine(item.name, index, item.points, auxiliary?.type ?? (item.dashed ? "dashed" : "solid"));
    return { ...line, xAxisIndex: index, ...(id ? { id } : {}), ...(signal?.kind === "appliedOutput" ? { step: "end" as const } : {}),
      ...(color ? { itemStyle: { color }, lineStyle: { ...line.lineStyle, color } } : {}) };
  }));
}

/** Hovering the response plot lists every displayed trace at the nearest recorded time. */
export function responseChartTooltip(): { trigger: "axis"; confine?: boolean; valueFormatter?: (value: unknown) => string } {
  return { trigger: "axis", confine: true, valueFormatter: formatChartValue };
}
