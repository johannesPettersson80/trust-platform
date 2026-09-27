import type * as echarts from "echarts/core";

import { formatEngineeringNumber, normalizeChartWindow } from "../chartContract";
import { chartTheme, download } from "./chartPrimitives";

export interface RecordedResponseChannel { readonly id: string; readonly unit: string }
export interface RecordedResponseSample {
  readonly timeSeconds: number;
  readonly values: Readonly<Record<string, { readonly value: number | null; readonly quality: string }>>;
}
export interface RecordedResponseRun {
  readonly evidence: string;
  readonly channels: readonly RecordedResponseChannel[];
  readonly samples: readonly RecordedResponseSample[];
  readonly conditions: { readonly modelRevision: string; readonly programRevision: string };
}
export interface RecordedResponsePlot {
  readonly chart: echarts.ECharts;
  readonly host: HTMLElement;
  readonly role: string;
  readonly run: RecordedResponseRun;
}

/** Shared interaction surface for immutable, actual recorded response charts. */
export class RecordedResponseInteraction {
  private windowStart = 0;
  private windowEnd = 100;
  private cursorTime: number | undefined;
  private synchronizing = false;
  private disposed = false;
  private pendingExport: string | undefined;
  private readonly listeners: Array<() => void> = [];

  constructor(private readonly plots: readonly RecordedResponsePlot[], private readonly status: HTMLElement) {
    for (const plot of plots) this.bind(plot);
    const onExport = (event: Event) => {
      const detail = (event as CustomEvent<{ requestId: string; result: { ok: boolean; filename?: string; reason?: string } }>).detail;
      if (detail.requestId !== this.pendingExport) return;
      this.pendingExport = undefined;
      this.status.textContent = detail.result.ok ? "Saved recorded response plot." : detail.result.reason === "Export cancelled." ? "Export cancelled." :
        `Recorded response comparison could not be exported: ${detail.result.reason ?? "unknown error"}.`;
    };
    window.addEventListener("trust-control-studio-export-result", onExport);
    this.listeners.push(() => window.removeEventListener("trust-control-studio-export-result", onExport));
  }

  dispose(): void { this.disposed = true; this.listeners.splice(0).forEach(remove => remove()); }

  handle(action: string): boolean {
    switch (action) {
      case "inspect":
        this.plots[0]?.host.focus();
        this.inspect(this.plots[0]);
        return true;
      case "zoom-in": this.zoom(0.75); return true;
      case "zoom-out": this.zoom(1.25); return true;
      case "reset": this.setWindow(0, 100); return true;
      case "export-figure": void this.exportComparison(); return true;
      default: return false;
    }
  }

  private bind(plot: RecordedResponsePlot): void {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault(); this.move(plot, event.key === "ArrowRight" ? 1 : -1);
      } else if (event.key === "+" || event.key === "=") {
        event.preventDefault(); this.zoom(0.75);
      } else if (event.key === "-") {
        event.preventDefault(); this.zoom(1.25);
      } else if (event.key === "Home") {
        event.preventDefault(); this.setWindow(0, 100);
      }
    };
    plot.host.addEventListener("keydown", keydown);
    this.listeners.push(() => plot.host.removeEventListener("keydown", keydown));
    const dataZoom = (event: unknown) => {
      if (this.synchronizing) return;
      const range = zoomRange(event);
      if (range) this.setWindow(range[0], range[1]);
    };
    plot.chart.on("datazoom", dataZoom);
    this.listeners.push(() => plot.chart.off("datazoom", dataZoom));
  }

  private inspect(plot: RecordedResponsePlot | undefined): void {
    if (!plot || plot.run.samples.length === 0) {
      this.status.textContent = "No recorded response samples are available for inspection.";
      return;
    }
    this.setCursor(this.cursorTime ?? plot.run.samples[0].timeSeconds);
  }

  private move(plot: RecordedResponsePlot, delta: number): void {
    if (plot.run.samples.length === 0) return this.inspect(undefined);
    const current = nearestIndex(plot.run.samples, this.cursorTime ?? plot.run.samples[0].timeSeconds);
    const next = Math.max(0, Math.min(plot.run.samples.length - 1, current + delta));
    this.setCursor(plot.run.samples[next].timeSeconds);
  }

  private setCursor(timeSeconds: number): void {
    this.cursorTime = timeSeconds;
    for (const plot of this.plots) {
      const index = nearestIndex(plot.run.samples, timeSeconds);
      if (index < 0) continue;
      plot.chart.dispatchAction({ type: "showTip", seriesIndex: 0, dataIndex: index });
      plot.chart.dispatchAction({ type: "updateAxisPointer", xAxisIndex: 0, value: timeSeconds });
    }
    this.status.textContent = this.plots.map(plot => describeSample(plot, timeSeconds)).join("; ");
  }

  private zoom(scale: number): void {
    const center = (this.windowStart + this.windowEnd) / 2;
    const width = Math.min(100, Math.max(1, (this.windowEnd - this.windowStart) * scale));
    this.setWindow(center - width / 2, center + width / 2);
  }

  private setWindow(start: number, end: number): void {
    const [nextStart, nextEnd] = normalizeChartWindow(start, end);
    if (nextStart === this.windowStart && nextEnd === this.windowEnd) return;
    this.windowStart = nextStart; this.windowEnd = nextEnd;
    this.synchronizing = true;
    for (const plot of this.plots) plot.chart.dispatchAction({ type: "dataZoom", start: nextStart, end: nextEnd });
    this.synchronizing = false;
    const [minimumTime, maximumTime] = timeExtent(this.plots);
    const duration = maximumTime - minimumTime;
    const startTime = minimumTime + duration * nextStart / 100;
    const endTime = minimumTime + duration * nextEnd / 100;
    this.status.textContent = `Time window ${formatEngineeringNumber(startTime)} s–${formatEngineeringNumber(endTime)} s (${formatEngineeringNumber(nextStart)}%–${formatEngineeringNumber(nextEnd)}%).`;
  }

  private async exportComparison(): Promise<void> {
    if (this.plots.length === 0) {
      this.status.textContent = "No recorded response plots are available to export.";
      return;
    }
    try {
      const theme = chartTheme();
      const images = this.plots.map(plot => plot.chart.renderToCanvas({ pixelRatio: 2, backgroundColor: theme.background }));
      if (this.disposed) return;
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(...images.map(image => image.width));
      let context = canvas.getContext("2d");
      if (!context) throw new Error("comparison canvas is unavailable");
      context.font = "26px sans-serif";
      const metadata = this.plots.map(plot => [
        `${plot.role} · ${plot.run.evidence}`,
        `Model ${plot.run.conditions.modelRevision}`,
        `Program ${plot.run.conditions.programRevision}`,
      ].flatMap(line => wrappedLines(context!, line, canvas.width - 16)));
      canvas.height = images.reduce((height, image, index) => height + image.height + 24 + metadata[index].length * 32, 0);
      context = canvas.getContext("2d");
      if (!context) throw new Error("comparison canvas is unavailable");
      context.fillStyle = theme.background; context.fillRect(0, 0, canvas.width, canvas.height);
      context.fillStyle = theme.foreground; context.font = "26px sans-serif";
      let top = 0;
      for (const [index, plot] of this.plots.entries()) {
        const image = images[index];
        for (const line of metadata[index]) { context.fillText(line, 16, top + 28); top += 32; }
        context.drawImage(image, 0, top + 8);
        top += image.height + 24;
      }
      if (this.disposed) return;
      this.pendingExport = download(canvas.toDataURL("image/png"), "control-studio-recorded-comparison.png");
      this.status.textContent = "Actual Baseline and Candidate comparison export pending…";
    } catch (error) {
      this.status.textContent = `Recorded response comparison could not be exported: ${error instanceof Error ? error.message : String(error)}.`;
    }
  }
}

function zoomRange(value: unknown): [number, number] | undefined {
  const event = value as { start?: unknown; end?: unknown; batch?: unknown } | undefined;
  const item = Array.isArray(event?.batch) ? event.batch[0] as { start?: unknown; end?: unknown } | undefined : event;
  return typeof item?.start === "number" && typeof item.end === "number" ? [item.start, item.end] : undefined;
}

function nearestIndex(samples: readonly RecordedResponseSample[], time: number): number {
  if (samples.length === 0) return -1;
  let chosen = 0;
  for (let index = 1; index < samples.length; index += 1) {
    if (Math.abs(samples[index].timeSeconds - time) < Math.abs(samples[chosen].timeSeconds - time)) chosen = index;
  }
  return chosen;
}

function timeExtent(plots: readonly RecordedResponsePlot[]): [number, number] {
  const times = plots.flatMap(plot => plot.run.samples.map(sample => sample.timeSeconds)).filter(Number.isFinite);
  return times.length === 0 ? [0, 0] : [Math.min(...times), Math.max(...times)];
}

function wrappedLines(context: CanvasRenderingContext2D, value: string, width: number): string[] {
  const words = value.split(" "); const lines: string[] = []; let line = "";
  for (const word of words) {
    if (context.measureText(word).width > width) {
      if (line) { lines.push(line); line = ""; }
      let fragment = "";
      for (const character of word) {
        if (fragment && context.measureText(`${fragment}${character}`).width > width) {
          lines.push(fragment); fragment = character;
        } else fragment += character;
      }
      line = fragment;
      continue;
    }
    const next = line ? `${line} ${word}` : word;
    if (line && context.measureText(next).width > width) { lines.push(line); line = word; }
    else line = next;
  }
  if (line) lines.push(line);
  return lines;
}

function describeSample(plot: RecordedResponsePlot, timeSeconds: number): string {
  const index = nearestIndex(plot.run.samples, timeSeconds);
  if (index < 0) return `${plot.role}: no actual samples.`;
  const sample = plot.run.samples[index];
  const values = plot.run.channels.map(channel => {
    const reading = sample.values[channel.id];
    return reading?.quality === "good" && typeof reading.value === "number"
      ? `${channel.id} ${formatEngineeringNumber(reading.value)} ${channel.unit}`
      : `${channel.id} Gap (${reading?.quality ?? "missing"})`;
  });
  return `${plot.role} at ${formatEngineeringNumber(sample.timeSeconds)} s: ${values.join(", ")}`;
}
