import { format } from "echarts/core";
import { axis, type chartTheme } from "./chartPrimitives";

const AXIS_NAME_FONT = "12px sans-serif";

/** The response plot stacks one row per unit group in the chart's height. */
export function responseRowGrids(count: number, height: number) {
  return Array.from({ length: count }, (_row, index) => ({ left: 80, right: 24, top: index * height / count + 40, height: height / count - 100 }));
}

/** The y axis of one response row, named "signal (unit)". The rotated name is centred on its row; wider than the
 * row, it would run into the next row's name. It then puts the unit on its own line, and each line is kept
 * within the row. The legend and the plot description keep the full name. */
export function responseRowAxis(name: string, theme: ReturnType<typeof chartTheme>, values: readonly (number | null)[],
  gridIndex: number, show: boolean, rowPitch: number) {
  const width = Math.max(0, rowPitch - 16);
  const unitAt = name.indexOf(" (");
  const fits = format.truncateText(name, width, AXIS_NAME_FONT, "…") === name;
  const text = fits || unitAt <= 0 ? name : `${name.slice(0, unitAt)}\n${name.slice(unitAt + 1)}`;
  return { ...axis(text, "value", theme, values, { fitToData: true }), nameTruncate: { maxWidth: width, ellipsis: "…" }, show, gridIndex };
}
