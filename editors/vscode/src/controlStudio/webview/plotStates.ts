// Exists because analysis plots that had never received data, or whose last analysis
// failed, rendered as a literally empty canvas with no axes, sentence, or action (finding
// V-10 / Specification 35 S5.7: "Keep axes and show exactly one explanation and one
// action in every empty state"). This module builds a bounded axes-only option and an
// ECharts graphic-layer text overlay so chart.ts can keep a plot's axes visible and show
// one sentence without tearing down whatever the plot last rendered.

export interface PlotStateTheme {
  readonly foreground: string;
  readonly muted: string;
}

/** Resolves the one sentence an empty or failed plot state must show: the runtime's own
 * reason when one was supplied, otherwise a stage-appropriate fallback. Never combines
 * more than one sentence, so the state cannot read as competing instructions. */
export function plotStateMessage(reason: string | undefined, fallback: string): string {
  const trimmed = (reason ?? "").trim();
  return trimmed.length > 0 ? trimmed : fallback;
}

/** An ECharts graphic-layer text overlay: reapplying it only changes visibility and text,
 * so it never tears down or replaces whatever axes and series the plot already has. Pass
 * undefined to hide it. */
export function plotStateOverlay(id: string, message: string | undefined, theme: PlotStateTheme): Record<string, unknown> {
  return {
    id,
    type: "text",
    left: "center",
    top: "middle",
    invisible: !message,
    silent: true,
    z: 10,
    style: {
      text: message ?? "",
      fill: theme.muted,
      fontSize: 12,
      overflow: "breakAll",
      width: 260,
      align: "center",
    },
  };
}

/** A bounded value-type axis pair so a plot with no evidence yet shows real (if empty)
 * axes instead of a blank canvas; callers overlay a plotStateOverlay message on top. */
export function emptyAxesOption(xName: string, yName: string, theme: PlotStateTheme): Record<string, unknown> {
  const axis = (name: string): Record<string, unknown> => ({
    type: "value", name, nameLocation: "middle", nameGap: 30,
    nameTextStyle: { color: theme.muted }, axisLabel: { color: theme.muted },
    axisLine: { show: false }, axisTick: { show: false },
    splitLine: { lineStyle: { color: theme.muted, opacity: 0.12 } },
    min: 0, max: 1,
  });
  return { animation: false, xAxis: axis(xName), yAxis: axis(yName), series: [] };
}
