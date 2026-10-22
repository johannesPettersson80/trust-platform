/// <reference lib="dom" />

import * as echarts from "echarts/core";

import { formatEngineeringNumber, liveResponseAxisRange } from "../chartContract";
import { axisTickFormatter, frequencyAxisTickFormatter, linearAxisFractionDigits } from "./axisFormat";

// A log-scaled axis (frequency) keeps formatEngineeringNumber's own per-tick magnitude
// policy through frequencyAxisTickFormatter, since one shared digit count would be wrong
// across several decades. A linear axis derives one digit count from its own value span
// (when the caller has the plotted values at hand) so nearby ticks cannot disagree on
// precision (finding V-01).
export function axis(
  name: string,
  type: "value" | "log",
  theme: ReturnType<typeof chartTheme>,
  values?: readonly (number | null | undefined)[],
  options: { fitToData?: boolean } = {},
) {
  const formatter = type === "log"
    ? frequencyAxisTickFormatter()
    : axisTickFormatter(values ? linearAxisFractionDigits(values) : undefined);
  const finiteValues = values?.filter((value): value is number => typeof value === "number" && Number.isFinite(value)) ?? [];
  const range = options.fitToData && type === "value" ? liveResponseAxisRange(finiteValues) : undefined;
  // A fitted axis is padded beyond the data; label its regular ticks, not the padded ends beside them.
  return { type, name, min: range?.minimum ?? null, max: range?.maximum ?? null, nameLocation: "middle", nameGap: 30, nameTextStyle: { color: theme.muted },
    axisLabel: { color: theme.muted, formatter, hideOverlap: true, margin: 8, ...(range ? { showMinLabel: false, showMaxLabel: false } : {}) },
    axisLine: { show: false }, axisTick: { show: false }, splitNumber: 5,
    splitLine: { lineStyle: { color: theme.border, opacity: 0.18 } } };
}

/** Degrees between the ticks of a phase axis. */
const PHASE_STEP_DEGREES = 45;

/** The span of a phase axis in degrees: the finite values widened outward to multiples of 45 degrees, and at least one step
 * wide. Floating-point noise around a zero phase (a pure gain) therefore spans −45 to 45, or 0 to 45 when all the noise is
 * positive, instead of scaling the axis to 2.5e-16 degrees. With no finite value, the span is that of a zero phase. Drawing only:
 * the engine's phases are not changed. */
export function phaseAxisRange(values: readonly (number | null | undefined)[] = []): { minimum: number; maximum: number } {
  const finite = values.filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  const least = finite.length > 0 ? Math.min(...finite) : 0;
  const most = finite.length > 0 ? Math.max(...finite) : 0;
  const minimum = Math.floor(least / PHASE_STEP_DEGREES) * PHASE_STEP_DEGREES;
  const maximum = Math.ceil(most / PHASE_STEP_DEGREES) * PHASE_STEP_DEGREES;
  return { minimum, maximum: maximum > minimum ? maximum : minimum + PHASE_STEP_DEGREES };
}

/** A phase axis in degrees for the frequency-response and loop Bode charts: the axis over phaseAxisRange, with a tick every
 * 45 degrees. */
export function phaseAxis(name: string, theme: ReturnType<typeof chartTheme>, values?: readonly (number | null | undefined)[]) {
  const { minimum, maximum } = phaseAxisRange(values);
  return { ...axis(name, "value", theme), min: minimum, max: maximum, interval: PHASE_STEP_DEGREES };
}

export function engineeringLine(name: string, yAxisIndex: number, data: (number | null)[][], style: "solid" | "dashed" | "dotted") {
  return { name, type: "line" as const, yAxisIndex, showSymbol: false, lineStyle: { width: 2, type: style }, data };
}

/** Stroke of the auxiliary signal at a stable position: the colour cycles through the auxiliary
 * palette and the line style changes after each full cycle, so nine auxiliary signals never look
 * alike or like the primary traces. */
export function auxiliaryTraceStyle(theme: Pick<ReturnType<typeof chartTheme>, "auxiliaryOrange" | "auxiliaryPurple" | "auxiliaryYellow">,
  position: number) {
  const palette = [theme.auxiliaryOrange, theme.auxiliaryPurple, theme.auxiliaryYellow];
  const index = Math.max(0, position);
  return { color: palette[index % palette.length], type: (["solid", "dashed", "dotted"] as const)[Math.floor(index / palette.length) % 3] };
}

export function line(
  id: string,
  name: string,
  yAxisIndex: number,
  type: "solid" | "dashed" | "dotted" | number[],
  symbol: "circle" | "rect" | "diamond" | "triangle" | "none"
) {
  return {
    id,
    name,
    type: "line" as const,
    yAxisIndex,
    showSymbol: false,
    showAllSymbol: false,
    symbol,
    symbolSize: 5,
    connectNulls: false,
    lineStyle: { type, width: 2 },
    data: [],
  };
}

export function chartTheme() {
  const style = getComputedStyle(document.documentElement);
  const foreground = css(style, "--vscode-editor-foreground", "CanvasText");
  const stroke = (name: string, fallback = foreground): string => {
    const value = css(style, name, fallback);
    return echarts.color.modifyAlpha(value, 1) ?? value;
  };
  return {
    foreground,
    muted: css(style, "--vscode-descriptionForeground", foreground),
    border: css(style, "--vscode-panel-border", foreground),
    background: css(style, "--vscode-editor-background", "Canvas"),
    sp: stroke("--vscode-charts-green"),
    pv: stroke("--vscode-charts-blue"),
    mv: stroke("--vscode-charts-red", css(style, "--vscode-charts-orange", foreground)),
    auxiliaryOrange: stroke("--vscode-charts-orange"),
    auxiliaryPurple: stroke("--vscode-charts-purple"),
    auxiliaryYellow: stroke("--vscode-charts-yellow"),
  };
}

export function css(style: CSSStyleDeclaration, name: string, fallback: string): string {
  return style.getPropertyValue(name).trim() || fallback;
}

export function formatChartValue(value: unknown): string {
  return formatEngineeringNumber(Array.isArray(value) ? value[value.length - 1] : value);
}

/** A phase in a result table: degrees to 0.01 degree, and a value that rounds to zero reads as 0, so floating-point noise such
 * as 1.9e-16 does not show as a number of its own. Other numbers keep formatEngineeringNumber. */
export function formatPhaseDegrees(value: number): string {
  if (!Number.isFinite(value)) return formatEngineeringNumber(value);
  const text = value.toFixed(2);
  return /^-?0\.00$/.test(text) ? "0" : text;
}

/** A phase in a tooltip, as a result table shows it ({@link formatPhaseDegrees}); the value is the point's last coordinate. */
export function formatPhaseChartValue(value: unknown): string {
  return formatPhaseDegrees(Number(Array.isArray(value) ? value[value.length - 1] : value));
}
