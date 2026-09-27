/// <reference lib="dom" />

import * as echarts from "echarts/core";

import {
formatEngineeringNumber,
liveResponseAxisRange,
type ControlChartValue
} from "../chartContract";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import { axisTickFormatter,frequencyAxisTickFormatter,linearAxisFractionDigits } from "./axisFormat";
import { requestChartExport } from "./chartExportBridge";

export function initEngineeringChart(selector: string): echarts.ECharts {
  return echarts.init(requiredElement<HTMLElement>(selector), undefined, { renderer: "canvas" });
}

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
  return { type, name, min: range?.minimum ?? null, max: range?.maximum ?? null, nameLocation: "middle", nameGap: 30, nameTextStyle: { color: theme.muted },
    axisLabel: { color: theme.muted, formatter, hideOverlap: true, margin: 8 },
    axisLine: { show: false }, axisTick: { show: false }, splitNumber: 5,
    splitLine: { lineStyle: { color: theme.border, opacity: 0.18 } } };
}

export function engineeringLine(name: string, yAxisIndex: number, data: (number | null)[][], style: "solid" | "dashed") {
  return { name, type: "line" as const, yAxisIndex, showSymbol: false, lineStyle: { width: 2, type: style }, data };
}

export function pointSeries(name: string, data: number[][], symbol: "circle" | "cross") {
  return { name, type: "line" as const, showSymbol: true, symbol, symbolSize: 10,
    lineStyle: { width: 0 }, data };
}

export function bodeSeries(
  evidence: EngineeringPlotMessageEvidence,
  field: "magnitudeDb" | "phaseDegrees",
  candidateMarks?: Record<string, unknown>
) {
  return [
    engineeringLine("Plant", 0, evidence.plantBode.map((point) => [point.frequencyRadPerSecond, point[field]]), "solid"),
    { ...engineeringLine("Candidate open loop", 0,
      evidence.candidateOpenLoopBode.map((point) => [point.frequencyRadPerSecond, point[field]]), "dashed"),
      ...(candidateMarks ?? {}) },
  ];
}

/** Converts the plain reference-line and crossover-marker data from marginMarkers.ts into
 * the ECharts markLine/markPoint option a candidate series carries directly on the plot,
 * instead of margins existing only as text in the design-row chip (finding V-05). */
export function marginMarkOptions(
  lines: readonly { name: string; xAxis?: number; yAxis?: number }[],
  points: readonly { name: string; coord: readonly [number, number]; value: string }[],
  theme: ReturnType<typeof chartTheme>
) {
  return {
    markLine: {
      symbol: "none", silent: true,
      lineStyle: { type: "dashed", width: 1, color: theme.muted },
      label: { formatter: (param: { data: { name: string } }) => param.data.name, color: theme.muted, fontSize: 10 },
      data: lines.map((entry) => ({
        name: entry.name,
        ...(entry.xAxis !== undefined ? { xAxis: entry.xAxis } : {}),
        ...(entry.yAxis !== undefined ? { yAxis: entry.yAxis } : {}),
      })),
    },
    markPoint: {
      symbol: "circle", symbolSize: 7, itemStyle: { color: theme.foreground },
      label: {
        formatter: (param: { data: { value: string } }) => param.data.value,
        color: theme.foreground, fontSize: 10, position: "top",
      },
      data: points.map((entry) => ({ name: entry.name, coord: entry.coord, value: entry.value })),
    },
  };
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

export function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Control Studio chart element missing: ${selector}`);
  return element;
}

export function spoken(value: ControlChartValue): string {
  return value === null ? "missing" : formatEngineeringNumber(value);
}

export function formatChartValue(value: unknown): string {
  return formatEngineeringNumber(Array.isArray(value) ? value[value.length - 1] : value);
}

export function download(url: string, filename: string): string {
  const mimeType = /^data:([^;,]+)/.exec(url)?.[1] ?? "application/octet-stream";
  return requestChartExport(url, filename, mimeType);
}

export function plotLabel(id: string): string {
  return id.split("-").map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(" ")
    .replace("Rga", "RGA");
}

export function copyValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}
