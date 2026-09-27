import {
  CONTROL_CHART_SIGNALS, controlChartLayout, controlChartSampleValue,
  controlOutputAxisRange, controlOutputLimitLines, controlSignalLayout,
  displayEngineeringUnit, liveResponseAxisRange, type ControlChartSample,
} from "../chartContract";
import { readableSignalName } from "../signalNaming";
import { axisTickFormatter, linearAxisFractionDigits } from "./axisFormat";
import { chartTheme, line } from "./chartPrimitives";

export interface LiveSignal {
  id: string;
  name: string;
  unit: string;
  channel: string;
  role: string;
}

/** Plot metadata comes from the same recorded readings as the values. */
export function liveSignals(
  samples: readonly ControlChartSample[], processUnit: string, outputUnit: string,
  topology: "closed-loop" | "open-loop" = "closed-loop",
): LiveSignal[] {
  const additional = new Map<string, LiveSignal>();
  for (const sample of samples) for (const signal of sample.additional ?? []) {
    additional.set(signal.id, { ...signal, name: signal.displayName ?? readableSignalName(signal.id),
      unit: displayEngineeringUnit(signal.unit) || "engineering unit" });
  }
  const result = [
    ...(topology === "closed-loop" ? [{ id: "sp", name: "SP", unit: processUnit, channel: "Primary loop", role: "SP" }] : []),
    { id: "pv", name: topology === "open-loop" ? "Measured output" : "PV", unit: processUnit, channel: "Primary loop", role: "PV" },
    { id: "mv", name: topology === "open-loop" ? "Applied input" : "MV", unit: outputUnit, channel: "Primary loop", role: "MV" },
    ...additional.values(),
  ];
  return result.map(signal => result.some(other => other.id !== signal.id && other.name === signal.name)
    ? { ...signal, name: `${signal.name} · ${signal.id}` } : signal);
}

/** One ECharts option projection; shared unit grouping owns axes versus lanes. */
export function liveSignalPlot(
  samples: readonly ControlChartSample[], signals: readonly LiveSignal[], selected: ReadonlySet<string>,
  size: { width: number; height: number }, limits: { minimum: number; maximum: number } | undefined,
  outputUnit: string, theme: ReturnType<typeof chartTheme>,
) {
  const visible = signals.filter(signal => selected.has(signal.id));
  const grouping = controlSignalLayout(visible.length ? visible : signals.slice(0, 2));
  const lanes = grouping.mode === "lanes";
  const responsive = controlChartLayout(size.width);
  const count = lanes ? grouping.groups.length : 1;
  const top = responsive.compact ? 24 : 62;
  const gap = lanes ? 34 : 0;
  const height = Math.max(45, (size.height - top - 44 - gap * (count - 1)) / count);
  const elapsed = samples.map(sample => sample.elapsedSeconds);
  const grids = Array.from({ length: count }, (_, index) => ({
    left: responsive.grid.left, right: responsive.grid.right,
    top: top + index * (height + gap), height,
  }));
  const xAxis = grids.map((_, index) => ({
    type: "value" as const, gridIndex: index, min: elapsed[0] ?? 0, max: elapsed[elapsed.length - 1] ?? 1,
    name: index === count - 1 && responsive.showAxisNames ? "Elapsed time (s)" : "",
    nameLocation: "middle" as const, nameGap: 28, nameTextStyle: { color: theme.muted },
    axisLabel: { show: index === count - 1, color: theme.muted, hideOverlap: true, padding: [0, 2],
      formatter: axisTickFormatter(linearAxisFractionDigits(elapsed)) },
    axisLine: { lineStyle: { color: theme.border } },
    splitLine: { show: true, lineStyle: { color: theme.border, opacity: 0.28 } },
  }));
  const yAxis = grouping.groups.map((group, index) => {
    const members = signals.filter(signal => group.signalIds.includes(signal.id));
    const values = samples.flatMap(sample => members.map(signal => controlChartSampleValue(sample, signal.id)));
    const finite = values.filter((value): value is number => value !== null && Number.isFinite(value));
    const output = group.unit === outputUnit && limits !== undefined;
    const extent = finite.reduce((range, value) => [Math.min(range[0], value), Math.max(range[1], value)],
      [Infinity, -Infinity]);
    let range = finite.length ? liveResponseAxisRange(extent) : undefined;
    if (output) {
      const bound = controlOutputAxisRange(limits.minimum, limits.maximum);
      // A state/request can exceed an actuator bound. Preserve that measured
      // difference, keeping the configured limit lines at their actual values.
      range = finite.some(value => value < bound.minimum || value > bound.maximum)
        ? liveResponseAxisRange([...extent, bound.minimum, bound.maximum]) : bound;
    }
    const primaryOnly = members.every(signal => ["sp", "pv", "mv"].includes(signal.id));
    const roleNames: Record<string, string> = { SP: "Setpoint", PV: "Process value", MV: "Output",
      state: "Controller state", actuator: "Applied output", disturbance: "Disturbance",
      quality: "Signal quality", schedule: "Scheduling value" };
    const roles = [...new Set(members.map(signal => roleNames[signal.role] ?? signal.role))].join(" / ");
    const channels = [...new Set(members.map(signal => signal.channel))];
    const label = primaryOnly || members.length <= 2 ? members.map(signal => signal.name).join(" / ")
      : lanes ? `${channels.length === 1 ? `${readableSignalName(channels[0])} · ` : ""}${roles}`
        : roles;
    const right = !lanes && index === 1;
    return {
      type: "value" as const, gridIndex: lanes ? index : 0,
      position: right ? "right" as const : "left" as const,
      name: responsive.showAxisNames ? `${label} (${group.unit})` : "",
      nameLocation: right ? "middle" as const : "end" as const, nameGap: right ? 42 : 15,
      nameRotate: right ? -90 : 0, nameTextStyle: { color: theme.muted, align: right ? undefined : "left" as const },
      min: range?.minimum, max: range?.maximum,
      splitNumber: Math.max(2, Math.min(5, Math.floor(height / 24))),
      // Measured extents include padding, not engineering limits. Prefer the
      // regular ticks there so a rounded endpoint cannot repeat the next tick.
      axisLabel: { color: theme.muted, hideOverlap: true, showMinLabel: output, showMaxLabel: output,
        formatter: axisTickFormatter(linearAxisFractionDigits(
        range ? [range.minimum, range.maximum] : [])) },
      axisLine: { show: true, lineStyle: { color: theme.border } },
      splitLine: { show: !right, lineStyle: { color: theme.border, opacity: 0.28 } },
    };
  });
  const palette = [theme.auxiliaryOrange, theme.auxiliaryPurple, theme.auxiliaryYellow];
  const primaryCount = signals.filter(signal => ["sp", "pv", "mv"].includes(signal.id)).length;
  const series = signals.map((signal, index) => {
    const group = Math.max(0, grouping.groups.findIndex(item => item.signalIds.includes(signal.id)));
    const primaryId = signal.id === "sp" || signal.id === "pv" || signal.id === "mv" ? signal.id : undefined;
    const primary = primaryId ? CONTROL_CHART_SIGNALS.find(item => item.id === primaryId) : undefined;
    const auxiliaryIndex = index - primaryCount;
    const color = primaryId ? theme[primaryId] : palette[(auxiliaryIndex % palette.length + palette.length) % palette.length];
    const style = primary?.line ?? (["solid", "dashed", "dotted"] as const)[Math.floor(auxiliaryIndex / palette.length) % 3];
    const hasLimits = selected.has(signal.id) && signal.unit === outputUnit && limits &&
      !signals.slice(0, index).some(before => selected.has(before.id) && before.unit === outputUnit);
    return {
      ...line(signal.id, signal.name, group, style, primary?.symbol ?? "none"),
      z: signal.id === "sp" ? 3 : 2,
      xAxisIndex: lanes ? group : 0,
      itemStyle: { color }, lineStyle: { type: style, width: 2, color },
      data: samples.map(sample => [sample.elapsedSeconds, controlChartSampleValue(sample, signal.id)]),
      ...(hasLimits ? { markLine: { silent: true, symbol: "none", label: { show: false },
        lineStyle: { type: "dashed" as const, width: 1, color: theme.mv },
        data: controlOutputLimitLines(limits.minimum, limits.maximum).map(limit => ({ name: limit.name, yAxis: limit.value })),
      } } : {}),
    };
  });
  return {
    grid: grids, xAxis, yAxis, series,
    legend: { type: "scroll" as const, top: 8, right: 8, bottom: "auto", show: responsive.showLegend,
      data: visible.map(signal => signal.name),
      selected: Object.fromEntries(signals.map(signal => [signal.name, selected.has(signal.id)])),
      textStyle: { color: theme.muted } },
    axisPointer: { link: [{ xAxisIndex: "all" as const }] },
    dataZoom: [{ type: "inside" as const, xAxisIndex: grids.map((_, index) => index), filterMode: "none" as const }],
  };
}
