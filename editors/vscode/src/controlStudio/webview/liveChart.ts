/// <reference lib="dom" />

import * as echarts from "echarts/core";

import {
controlChartAccessibleSummary,
controlChartCsv,
controlChartEventRows,
controlChartSampleValue,
displayedControlSamples,
displayEngineeringUnit,
formatEngineeringNumber,
normalizeChartWindow,
type ControlChartSample
} from "../chartContract";
import { liveSignalPlot, liveSignals, type LiveSignal } from "./liveSignalPlot";

import { chartTheme,download,formatChartValue,plotLabel,requiredElement,spoken } from "./chartPrimitives";
export class ControlChartAdapter {
  private readonly chart: echarts.ECharts;
  private readonly status: HTMLElement;
  private readonly table: HTMLElement;
  private readonly tableBody: HTMLTableSectionElement;
  private readonly annotations: HTMLElement;
  private readonly events: HTMLElement;
  private readonly emptyState: HTMLElement;
  private samples: ControlChartSample[] = [];
  private selectedSample = -1;
  private displayedSamples: ControlChartSample[] = [];
  private signals: LiveSignal[] = [];
  private readonly selectedSignals = new Set(["sp", "pv", "mv"]);
  private outputLimits: { minimum: number; maximum: number } | undefined;
  private windowStart = 0;
  private windowEnd = 100;
  private processUnit = "engineering unit";
  private outputUnit = "%";
  private source = "Control loop";
  private readonly pendingExports = new Map<string, string>();

  private loopTopology(): "closed-loop" | "open-loop" {
    return this.host.closest<HTMLElement>("[data-control-studio]")?.dataset.loopTopology === "open-loop"
      ? "open-loop" : "closed-loop";
  }

  constructor(private readonly host: HTMLElement) {
    this.status = requiredElement<HTMLElement>("[data-chart-status]");
    this.table = requiredElement<HTMLElement>("[data-chart-table]");
    this.tableBody = requiredElement<HTMLTableSectionElement>("[data-chart-table] tbody");
    this.annotations = requiredElement<HTMLElement>("[data-chart-annotations]");
    this.events = requiredElement<HTMLElement>("[data-control-events]");
    this.emptyState = requiredElement<HTMLElement>(".plot-empty");
    this.emptyState.textContent = "No live samples yet. Verify the loop signal bindings in Live Values.";
    this.chart = echarts.init(host, undefined, { renderer: "canvas" });
    this.applyOptions();
    this.applyResponsiveLayout(host.clientWidth);
    this.bindControls();
    this.chart.on("datazoom", () => this.renderAnnotations(this.displayedSamples));
    this.chart.on("legendselectchanged", payload => {
      const event = payload as { selected: Record<string, boolean> };
      for (const signal of this.signals) {
        const selected = event.selected[signal.name];
        if (typeof selected !== "boolean") continue;
        if (selected === this.selectedSignals.has(signal.id)) continue;
        if (selected) this.selectedSignals.add(signal.id); else this.selectedSignals.delete(signal.id);
        window.dispatchEvent(new CustomEvent("trust-control-chart-signal-selection", {
          detail: { id: signal.id, selected },
        }));
      }
      this.renderPlot();
    });
    window.addEventListener("trust-control-studio-export-result", event => {
      const detail = (event as CustomEvent<{ requestId: string; result: { ok: boolean; filename?: string; reason?: string } }>).detail;
      const label = this.pendingExports.get(detail.requestId);
      if (!label) return;
      this.pendingExports.delete(detail.requestId);
      this.announce(detail.result.ok ? `${label} exported` : detail.result.reason === "Export cancelled." ? "Export cancelled." : `${label} export failed: ${detail.result.reason ?? "unknown error"}`);
    });

    new ResizeObserver((entries) => {
      this.chart.resize();
      this.applyResponsiveLayout(entries[0]?.contentRect.width ?? host.clientWidth);
    }).observe(host);
    new MutationObserver(() => this.applyTheme()).observe(document.body, {
      attributes: true,
      attributeFilter: ["class", "style"],
    });
    window.addEventListener("trust-control-chart-window", (event) => {
      const detail = (event as CustomEvent<{ source: HTMLElement; start: number; end: number }>).detail;
      if (detail?.source !== this.host) this.applyLinkedWindow(detail.start, detail.end);
    });
  }

  get hasSamples(): boolean { return this.samples.length > 0; }

  setSamples(samples: readonly ControlChartSample[]): void {
    this.samples = [...samples];
    this.selectedSample = samples.length > 0 ? samples.length - 1 : -1;
    const displayed = displayedControlSamples(samples, 4000);
    this.displayedSamples = displayed;
    this.renderPlot();
    this.renderTable();
    this.renderEvents(samples);
    this.emptyState.hidden = samples.length > 0;
    const tableButton = document.querySelector<HTMLButtonElement>('[data-chart-action="table"]');
    if (tableButton) tableButton.disabled = samples.length === 0;
    this.updateAccessibleSummary();
    this.host.setAttribute(
      "aria-description",
      samples.length === 0
        ? "No live samples"
        : `${displayed.length} displayed samples from ${samples.length} retained samples; missing values are gaps`
    );
    this.announceSelectedSample();
  }

  setUnits(processUnit: string, outputUnit: string): void {
    const displayedProcessUnit = displayEngineeringUnit(processUnit);
    const displayedOutputUnit = displayEngineeringUnit(outputUnit);
    if (displayedProcessUnit === this.processUnit && displayedOutputUnit === this.outputUnit) return;
    this.processUnit = displayedProcessUnit;
    this.outputUnit = displayedOutputUnit;
    const studio = document.querySelector<HTMLElement>("[data-control-studio]");
    if (studio) {
      studio.dataset.processUnit = displayedProcessUnit;
      studio.dataset.outputUnit = displayedOutputUnit;
    }
    this.renderPlot();
    this.updateAccessibleSummary();
  }

  setSource(source: string): void {
    this.source = source;
    this.updateAccessibleSummary();
  }

  setOutputLimits(minimum: number, maximum: number): void {
    if (this.outputLimits?.minimum === minimum && this.outputLimits.maximum === maximum) return;
    this.outputLimits = { minimum, maximum };
    this.renderPlot();
  }

  setSignalSelected(id: string, selected: boolean): void {
    if (this.selectedSignals.has(id) === selected) return;
    if (selected) this.selectedSignals.add(id); else this.selectedSignals.delete(id);
    this.renderPlot();
    this.renderTable();
  }

  setSelectedSignals(ids: readonly string[]): void {
    this.selectedSignals.clear();
    for (const id of ids) this.selectedSignals.add(id);
    this.renderPlot();
    this.renderTable();
  }

  viewWindow(): [number, number] {
    return [this.windowStart, this.windowEnd];
  }

  restoreWindow(window: [number, number]): void {
    this.setWindow(window[0], window[1], "Saved inspection window restored");
  }

  resize(): void {
    this.chart.resize();
  }

  private updateAccessibleSummary(): void {
    this.host.setAttribute("aria-label", controlChartAccessibleSummary(
      this.source, this.samples[this.samples.length - 1], this.processUnit, this.outputUnit, this.loopTopology()
    ));
  }

  private applyOptions(): void {
    this.chart.setOption({
      animation: false, aria: { enabled: false, decal: { show: true } },
      legend: { top: 8, right: 8, selectedMode: true },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue,
        axisPointer: { type: "cross" } },
    });
    this.renderPlot();
  }

  private renderPlot(): void {
    this.signals = liveSignals(this.samples, this.processUnit, this.outputUnit, this.loopTopology());
    this.chart.setOption(liveSignalPlot(this.displayedSamples, this.signals, this.selectedSignals,
      { width: this.host.clientWidth, height: this.host.clientHeight }, this.outputLimits,
      this.outputUnit, chartTheme()), { replaceMerge: ["grid", "xAxis", "yAxis", "series"] });
    this.renderAnnotations(this.displayedSamples);
  }

  private applyTheme(): void { this.renderPlot(); }

  private applyResponsiveLayout(width: number): void {
    void width;
    this.renderPlot();
  }

  private bindControls(): void {
    for (const button of document.querySelectorAll<HTMLButtonElement>("[data-chart-action]")) {
      button.addEventListener("click", () => this.runAction(button.dataset.chartAction ?? "", button));
    }
    this.host.addEventListener("keydown", (event) => {
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        this.moveSelection(event.key === "ArrowRight" ? 1 : -1);
      } else if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        this.zoom(0.75);
      } else if (event.key === "-") {
        event.preventDefault();
        this.zoom(1.25);
      } else if (event.key === "Home") {
        event.preventDefault();
        this.resetView();
      }
    });
  }

  private runAction(action: string, button: HTMLButtonElement): void {
    const delegated = new CustomEvent("trust-control-shared-chart-action", { detail: { action }, cancelable: true });
    if (!window.dispatchEvent(delegated)) return;
    switch (action) {
      case "inspect": this.host.focus(); this.announceSelectedSample(); break;
      case "zoom-in": this.zoom(0.75); break;
      case "zoom-out": this.zoom(1.25); break;
      case "pan-left": this.pan(-0.2); break;
      case "pan-right": this.pan(0.2); break;
      case "reset": this.resetView(); break;
      case "export-figure": this.exportFigure(); break;
      case "export-data": this.exportData(); break;
      case "table": this.toggleTable(button); break;
    }
  }

  private moveSelection(delta: number): void {
    if (this.samples.length === 0) {
      this.announce("No samples available for inspection");
      return;
    }
    this.selectedSample = Math.min(this.samples.length - 1, Math.max(0, this.selectedSample + delta));
    const sample = this.samples[this.selectedSample];
    const displayIndex = this.displayedSamples.findIndex(point => point.elapsedSeconds >= sample.elapsedSeconds);
    const seriesIndex = this.signals.findIndex(signal => this.selectedSignals.has(signal.id));
    if (seriesIndex >= 0) this.chart.dispatchAction({ type: "showTip", seriesIndex,
      dataIndex: displayIndex >= 0 ? displayIndex : this.displayedSamples.length - 1 });
    this.chart.dispatchAction({ type: "updateAxisPointer", xAxisIndex: 0, value: sample.elapsedSeconds });
    this.announceSelectedSample();
  }

  private zoom(scale: number): void {
    const center = (this.windowStart + this.windowEnd) / 2;
    const width = Math.min(100, Math.max(1, (this.windowEnd - this.windowStart) * scale));
    this.setWindow(center - width / 2, center + width / 2, scale < 1 ? "Zoomed in" : "Zoomed out");
  }

  private pan(fraction: number): void {
    const width = this.windowEnd - this.windowStart;
    const shift = width * fraction;
    this.setWindow(this.windowStart + shift, this.windowEnd + shift, fraction < 0 ? "Panned earlier" : "Panned later");
  }

  private setWindow(start: number, end: number, message: string): void {
    [this.windowStart, this.windowEnd] = normalizeChartWindow(start, end);
    this.chart.dispatchAction({ type: "dataZoom", start: this.windowStart, end: this.windowEnd });
    window.dispatchEvent(new CustomEvent("trust-control-chart-window", {
      detail: { source: this.host, start: this.windowStart, end: this.windowEnd },
    }));
    this.announce(message);
  }

  private applyLinkedWindow(start: number, end: number): void {
    [this.windowStart, this.windowEnd] = normalizeChartWindow(start, end);
    this.chart.dispatchAction({ type: "dataZoom", start: this.windowStart, end: this.windowEnd });
  }

  private resetView(): void {
    this.windowStart = 0;
    this.windowEnd = 100;
    this.chart.dispatchAction({ type: "dataZoom", start: 0, end: 100 });
    this.announce("View reset");
  }

  private toggleTable(button: HTMLButtonElement): void {
    const show = this.table.hidden;
    this.table.hidden = !show;
    button.setAttribute("aria-expanded", String(show));
    this.announce(show ? "Data table shown" : "Data table hidden");
  }

  setPlotVisible(visible: boolean): void {
    const button = document.querySelector<HTMLButtonElement>('[data-chart-action="table"]');
    if (button) {
      button.hidden = !visible;
      button.disabled = !this.hasSamples;
      if (!visible) button.setAttribute("aria-expanded", "false");
    }
    if (!visible) this.table.hidden = true;
  }

  private exportFigure(): void {
    const view = document.querySelector<HTMLElement>(".plot-view:not([hidden])");
    const plotId = view?.dataset.plotView ?? "response";
    const canvases = view ? [...view.querySelectorAll<HTMLCanvasElement>("canvas")] : [];
    if (canvases.length === 0) {
      this.announce("No visible plot is available to export");
      return;
    }
    let image: string;
    if (plotId === "response") {
      image = this.chart.getDataURL({ type: "png", pixelRatio: 2, backgroundColor: chartTheme().background });
    } else {
      // Bode uses coordinated magnitude and phase canvases. Composing every
      // canvas in the visible plot preserves exactly what the engineer sees.
      const composite = document.createElement("canvas");
      composite.width = Math.max(...canvases.map((canvas) => canvas.width));
      composite.height = canvases.reduce((height, canvas) => height + canvas.height, 0);
      const context = composite.getContext("2d");
      if (!context) {
        this.announce("The visible plot could not be exported");
        return;
      }
      context.fillStyle = chartTheme().background;
      context.fillRect(0, 0, composite.width, composite.height);
      let y = 0;
      for (const canvas of canvases) {
        context.drawImage(canvas, 0, y);
        y += canvas.height;
      }
      image = composite.toDataURL("image/png");
    }
    const requestId = download(image, `control-studio-${plotId}.png`);
    this.pendingExports.set(requestId, `${plotLabel(plotId)} PNG`);
    this.announce(`${plotLabel(plotId)} export pending`);
  }

  private exportData(): void {
    if (this.samples.length === 0) {
      this.announce("No samples available to export");
      return;
    }
    const requestId = download(`data:text/csv;charset=utf-8,${encodeURIComponent(controlChartCsv(this.samples))}`, "control-studio-monitor.csv");
    this.pendingExports.set(requestId, "Underlying samples CSV");
    this.announce("Underlying samples export pending");
  }

  private renderTable(): void {
    const signals = this.signals.filter(signal => this.selectedSignals.has(signal.id));
    const heading = this.table.querySelector("thead tr");
    heading?.replaceChildren(...["Elapsed time (s)", ...signals.map(signal => `${signal.name} (${signal.unit})`)].map(label => {
      const cell = document.createElement("th"); cell.scope = "col"; cell.textContent = label; return cell;
    }));
    this.tableBody.replaceChildren(...this.samples.map((sample) => {
      const row = document.createElement("tr");
      const time = document.createElement("td");
      time.textContent = formatEngineeringNumber(sample.elapsedSeconds);
      row.append(time);
      for (const signal of signals) {
        const value = controlChartSampleValue(sample, signal.id);
        const quality = ["sp", "pv", "mv"].includes(signal.id) ? sample.quality ?? "unknown"
          : sample.additional?.find(item => item.id === signal.id)?.quality ?? "unavailable";
        const cell = document.createElement("td");
        cell.textContent = value === null ? "Missing" : formatEngineeringNumber(value);
        const state = document.createElement("span");
        state.className = quality === "good" ? "sr-only" : "sample-quality";
        state.textContent = quality[0].toUpperCase() + quality.slice(1);
        cell.append(" ", state);
        row.append(cell);
      }
      return row;
    }));
  }

  private renderAnnotations(samples: readonly ControlChartSample[]): void {
    const grids = this.chart.getOption().grid as { top: number; height: number }[];
    const first = grids[0], last = grids[grids.length - 1];
    const eventSamples = samples
      .map((sample, index) => ({ sample, index }))
      .filter(({ sample }, index) =>
        Boolean(sample.events?.length) || (index > 0 && sample.quality !== samples[index - 1].quality));
    this.annotations.replaceChildren(...eventSamples.flatMap(({ sample }) => {
      const x = this.chart.convertToPixel({ xAxisIndex: 0 }, sample.elapsedSeconds) as number;
      if (!Number.isFinite(x) || !first || !last ||
          !this.chart.containPixel({ gridIndex: 0 }, [x, first.top + first.height / 2])) return [];
      const marker = document.createElement("span");
      marker.className = "chart-annotation";
      marker.style.left = `${x}px`;
      marker.style.top = `${first.top}px`;
      marker.style.bottom = "auto";
      marker.style.height = `${last.top + last.height - first.top}px`;
      const meaning = sample.events?.map((event) => event.label).join(", ")
        || `Data quality changed to ${sample.quality}`;
      marker.title = `${this.source} · ${formatEngineeringNumber(sample.elapsedSeconds)} s · ${meaning}`;
      marker.setAttribute("aria-hidden", "true");
      return [marker];
    }));
  }

  private renderEvents(samples: readonly ControlChartSample[]): void {
    const rows = controlChartEventRows(samples);
    if (rows.length === 0) {
      this.events.textContent = "No recorded events. Run or open an experiment to create event evidence.";
      return;
    }
    this.events.replaceChildren(...rows.map((event) => {
      const row = document.createElement("div");
      row.className = "event-row";
      const time = document.createElement("time");
      time.textContent = `${formatEngineeringNumber(event.timeSeconds)} s`;
      const kind = document.createElement("span");
      kind.className = "event-kind";
      kind.textContent = event.kind.split("-").join(" ");
      const label = document.createElement("span");
      label.textContent = event.label;
      row.append(time, kind, label);
      return row;
    }));
  }

  private announceSelectedSample(): void {
    if (this.selectedSample < 0 || this.samples.length === 0) {
      this.announce("No samples available for inspection");
      return;
    }
    const sample = this.samples[this.selectedSample];
    const values = this.signals.filter(signal => this.selectedSignals.has(signal.id)).map(signal => {
      const reading = sample.additional?.find(item => item.id === signal.id);
      const quality = reading && reading.quality !== "good" ? ` (${reading.quality})` : "";
      return `${signal.name} ${spoken(controlChartSampleValue(sample, signal.id))} ${signal.unit}${quality}`;
    });
    this.announce(`At ${formatEngineeringNumber(sample.elapsedSeconds)} seconds: ${values.join(", ")}`);
  }

  private announce(message: string): void {
    this.status.textContent = message;
  }
}
