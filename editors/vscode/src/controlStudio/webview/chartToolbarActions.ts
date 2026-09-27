import * as echarts from "echarts/core";

import { chartTheme, download } from "./chartPrimitives";
import type { ActiveChartAction } from "./activeChartActionRouter";

export function executeChartToolbarAction(
  action: ActiveChartAction,
  plotId: string,
  charts: readonly echarts.ECharts[],
  host: HTMLElement | null | undefined,
  windows: Map<string, [number, number]>
): boolean {
  if (action === "inspect") {
    const options = charts[0]?.getOption() as { series?: Array<{ data?: unknown[] }> } | undefined;
    const data = options?.series?.[0]?.data;
    charts[0]?.dispatchAction({ type: "showTip", seriesIndex: 0, dataIndex: Math.max(0, (data?.length ?? 1) - 1) });
    host?.focus();
    return true;
  }
  if (action === "reset" || action === "zoom-in" || action === "zoom-out") {
    const previous = windows.get(plotId) ?? [0, 100];
    const width = action === "reset" ? 100 : Math.max(5, Math.min(100, (previous[1] - previous[0]) * (action === "zoom-in" ? 0.75 : 1.25)));
    const center = action === "reset" ? 50 : (previous[0] + previous[1]) / 2;
    const next: [number, number] = action === "reset" ? [0, 100] : [Math.max(0, center - width / 2), Math.min(100, center + width / 2)];
    windows.set(plotId, next);
    charts.forEach(chart => {
      chart.dispatchAction({ type: "dataZoom", start: next[0], end: next[1] });
      chart.dispatchAction({ type: "updateAxisPointer", currTrigger: "leave" });
    });
    return true;
  }
  const canvases = charts.map(chart => chart.getDom().querySelector("canvas"))
    .filter((canvas): canvas is HTMLCanvasElement => Boolean(canvas));
  if (canvases.length === 0) return true;
  const composite = document.createElement("canvas");
  composite.width = Math.max(...canvases.map(canvas => canvas.width));
  composite.height = canvases.reduce((height, canvas) => height + canvas.height, 0);
  const context = composite.getContext("2d");
  if (!context) return true;
  context.fillStyle = chartTheme().background;
  context.fillRect(0, 0, composite.width, composite.height);
  let y = 0;
  for (const canvas of canvases) { context.drawImage(canvas, 0, y); y += canvas.height; }
  download(composite.toDataURL("image/png"), `control-studio-${plotId}.png`);
  return true;
}
