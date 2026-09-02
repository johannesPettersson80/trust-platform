/// <reference lib="dom" />

import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import {
  AriaComponent,
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  MarkAreaComponent,
  MarkLineComponent,
  TooltipComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

import {
  CONTROL_CHART_SIGNALS,
  controlChartAxisLabels,
  controlChartAccessibleSummary,
  displayEngineeringUnit,
  controlChartEventRows,
  controlChartSaturationEvent,
  controlOutputAxisRange,
  identificationOutputAxisRange,
  controlOutputLimitLines,
  compactRevisionLabel,
  type ControlSaturationState,
  type ControlChartSample,
  type ControlChartValue,
  controlChartLayout,
  controlChartCsv,
  controlStagePlots,
  controlPlotView,
  directChartActionAvailability,
  selectControlPlot,
  decodeControlChartSession,
  decodeMimoPlotEvidence,
  createDesignHistory,
  commitDesignEdit,
  undoDesignEdit,
  redoDesignEdit,
  pinComparableDesign,
  displayedControlSamples,
  formatEngineeringNumber,
  normalizeChartWindow,
  parseGraphicalGain,
  pidEngineeringUnits,
  restoredControlChartSamples,
  type ControlWorkflowStage,
  type DesignHistory,
  type PinnedDesign,
  type MimoPlotEvidence,
} from "../chartContract";
import type { ControlPidCandidate } from "../controllerCandidate";
import { decodeLoopMonitorEnvelope, decodePlantMonitorEnvelope } from "../messages";
import type { LoopMonitorSample } from "../loopMonitor";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import type { IdentifiedModelPresentation } from "../identification";

echarts.use([
  LineChart,
  GridComponent,
  LegendComponent,
  MarkAreaComponent,
  MarkLineComponent,
  TooltipComponent,
  DataZoomComponent,
  AriaComponent,
  CanvasRenderer,
]);

// All Control Studio time plots go through this adapter so keyboard, export,
// theme, and navigation behavior cannot drift between workflow pages.
class ControlChartAdapter {
  private readonly chart: echarts.ECharts;
  private readonly status: HTMLElement;
  private readonly table: HTMLElement;
  private readonly tableBody: HTMLTableSectionElement;
  private readonly annotations: HTMLElement;
  private readonly events: HTMLElement;
  private readonly emptyState: HTMLElement;
  private samples: ControlChartSample[] = [];
  private selectedSample = -1;
  private windowStart = 0;
  private windowEnd = 100;
  private compactLayout: boolean | undefined;
  private processUnit = "engineering unit";
  private outputUnit = "%";
  private source = "Control loop";

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

  setSamples(samples: readonly ControlChartSample[]): void {
    this.samples = [...samples];
    this.selectedSample = samples.length > 0 ? samples.length - 1 : -1;
    const displayed = displayedControlSamples(samples, 4000);
    this.chart.setOption({
      series: [
        { id: "sp", data: displayed.map((sample) => [sample.elapsedSeconds, sample.sp]) },
        { id: "pv", data: displayed.map((sample) => [sample.elapsedSeconds, sample.pv]) },
        { id: "mv", data: displayed.map((sample) => [sample.elapsedSeconds, sample.mv]) },
      ],
    });
    this.renderTable();
    this.renderAnnotations(displayed);
    this.renderEvents(samples);
    this.emptyState.hidden = samples.length > 0;
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
    const labels = controlChartAxisLabels(displayedProcessUnit, displayedOutputUnit);
    this.chart.setOption({ yAxis: [{ name: labels.process }, { name: labels.output }] });
    this.updateAccessibleSummary();
    this.compactLayout = undefined;
    this.applyResponsiveLayout(this.host.clientWidth);
  }

  setSource(source: string): void {
    this.source = source;
    this.updateAccessibleSummary();
  }

  setOutputLimits(minimum: number, maximum: number): void {
    const lines = controlOutputLimitLines(minimum, maximum);
    const range = controlOutputAxisRange(minimum, maximum);
    this.chart.setOption({
      yAxis: [{}, { min: range.minimum, max: range.maximum }],
      series: [{ id: "mv", markLine: { silent: true, symbol: "none",
        label: { show: false }, lineStyle: { type: "dashed", width: 1 },
        data: lines.map((line) => ({
          name: line.name,
          yAxis: line.value,
        })) } }],
    });
  }

  setSignalSelected(id: string, selected: boolean): void {
    this.chart.dispatchAction({ type: selected ? "legendSelect" : "legendUnSelect", name: id.toUpperCase() });
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
      this.source, this.samples[this.samples.length - 1], this.processUnit, this.outputUnit
    ));
  }

  private applyOptions(): void {
    const theme = chartTheme();
    this.chart.setOption({
      animation: false,
      // The adapter supplies a unit-aware live summary after every update.
      // ECharts' generated label omits engineering units and would overwrite it.
      aria: { enabled: false, decal: { show: true } },
      color: [theme.sp, theme.pv, theme.mv],
      grid: { left: 58, right: 58, top: 38, bottom: 42 },
      legend: { top: 8, right: 8, selectedMode: true, textStyle: { color: theme.muted } },
      tooltip: {
        trigger: "axis",
        confine: true,
        valueFormatter: formatChartValue,
        axisPointer: { type: "cross", label: { color: theme.foreground } },
      },
      dataZoom: [{ type: "inside", xAxisIndex: 0, filterMode: "none" }],
      xAxis: {
        type: "value",
        name: "Elapsed time (s)",
        nameLocation: "middle",
        nameGap: 28,
        nameTextStyle: { color: theme.muted },
        axisLabel: { color: theme.muted, formatter: formatEngineeringNumber },
        axisLine: { lineStyle: { color: theme.border } },
        splitLine: { show: true, lineStyle: { color: theme.border, opacity: 0.28 } },
      },
      yAxis: [
        {
          type: "value",
          name: "SP / PV (engineering unit)",
          nameTextStyle: { color: theme.muted },
          axisLabel: { color: theme.muted, formatter: formatEngineeringNumber },
          axisLine: { show: true, lineStyle: { color: theme.border } },
          splitLine: { lineStyle: { color: theme.border, opacity: 0.28 } },
        },
        {
          type: "value",
          name: "MV (%)",
          nameLocation: "middle",
          nameGap: 42,
          nameRotate: -90,
          nameTextStyle: { color: theme.muted },
          axisLabel: { color: theme.muted, formatter: formatEngineeringNumber },
          axisLine: { show: true, lineStyle: { color: theme.border } },
          splitLine: { show: false },
        },
      ],
      series: [
        ...CONTROL_CHART_SIGNALS.slice(0, 3).map((signal) =>
          line(signal.id, signal.name, signal.axis, signal.line, signal.symbol)),
      ],
    });
  }

  private applyTheme(): void {
    const theme = chartTheme();
    this.chart.setOption({
      color: [theme.sp, theme.pv, theme.mv],
      legend: { textStyle: { color: theme.muted } },
      xAxis: { nameTextStyle: { color: theme.muted }, axisLabel: { color: theme.muted } },
      yAxis: [
        { nameTextStyle: { color: theme.muted }, axisLabel: { color: theme.muted } },
        { nameTextStyle: { color: theme.muted }, axisLabel: { color: theme.muted } },
      ],
    });
  }

  private applyResponsiveLayout(width: number): void {
    const layout = controlChartLayout(width);
    if (layout.compact === this.compactLayout) return;
    this.compactLayout = layout.compact;
    const labels = controlChartAxisLabels(this.processUnit, this.outputUnit);
    this.chart.setOption({
      grid: layout.grid,
      legend: { show: layout.showLegend },
      xAxis: { name: layout.showAxisNames ? "Elapsed time (s)" : "" },
      yAxis: [
        { name: layout.showAxisNames ? labels.process : "" },
        { name: layout.showAxisNames ? labels.output : "" },
      ],
    });
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
    this.chart.dispatchAction({ type: "showTip", seriesIndex: 0, dataIndex: this.selectedSample });
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
    download(image, `control-studio-${plotId}.png`);
    this.announce(`${plotLabel(plotId)} exported as PNG`);
  }

  private exportData(): void {
    if (this.samples.length === 0) {
      this.announce("No samples available to export");
      return;
    }
    download(`data:text/csv;charset=utf-8,${encodeURIComponent(controlChartCsv(this.samples))}`, "control-studio-monitor.csv");
    this.announce("Underlying samples exported as CSV");
  }

  private renderTable(): void {
    this.tableBody.replaceChildren(...this.samples.map((sample) => {
      const row = document.createElement("tr");
      for (const value of [sample.elapsedSeconds, sample.sp, sample.pv, sample.mv]) {
        const cell = document.createElement("td");
        cell.textContent = value === null ? "Missing" : formatEngineeringNumber(value);
        row.append(cell);
      }
      return row;
    }));
  }

  private renderAnnotations(samples: readonly ControlChartSample[]): void {
    const eventSamples = samples
      .map((sample, index) => ({ sample, index }))
      .filter(({ sample }, index) =>
        Boolean(sample.events?.length) || (index > 0 && sample.quality !== samples[index - 1].quality));
    this.annotations.replaceChildren(...eventSamples.map(({ sample, index }) => {
      const marker = document.createElement("span");
      marker.className = "chart-annotation";
      marker.style.left = `${samples.length > 1 ? (index / (samples.length - 1)) * 100 : 0}%`;
      marker.title = sample.events?.map((event) => event.label).join(", ")
        ?? `Data quality changed to ${sample.quality}`;
      marker.setAttribute("aria-hidden", "true");
      return marker;
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
    this.announce(`At ${formatEngineeringNumber(sample.elapsedSeconds)} seconds: SP ${spoken(sample.sp)}, PV ${spoken(sample.pv)}, MV ${spoken(sample.mv)} percent`);
  }

  private announce(message: string): void {
    this.status.textContent = message;
  }
}

type DesignSnapshot = { revision: number; candidate: ControlPidCandidate };

class ControlChartWorkspace {
  private stage: ControlWorkflowStage = "Monitor";
  private readonly selectedPlots: Partial<Record<ControlWorkflowStage, string>> = { Monitor: "response" };
  private readonly evidence = {
    samples: false, recording: false, model: false, candidate: false,
    constraints: false, mimo: false,
  };
  private history: DesignHistory<DesignSnapshot> | undefined;
  private pins: PinnedDesign<DesignSnapshot>[] = [];
  private selectedSignals = new Set(["sp", "pv", "mv"]);
  private revision = 0;
  private editTimer: number | undefined;

  constructor(
    private readonly root: HTMLElement,
    private readonly adapter: ControlChartAdapter
  ) {
    this.installIdentificationViews();
    this.installMimoViews();
    this.installGraphicalDesignHandle();
    window.addEventListener("trust-control-studio-stage", (event) => {
      const stage = (event as CustomEvent<{ stage?: ControlWorkflowStage }>).detail?.stage;
      if (stage) this.setStage(stage);
    });
    for (const action of ["undo", "redo", "pin", "expand"] as const) {
      document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`)?.addEventListener("click", () => {
        if (action === "undo") this.undo();
        else if (action === "redo") this.redo();
        else if (action === "pin") this.pin();
        else this.toggleExpanded();
      });
    }
    this.renderTabs();
    this.saveSessionState();
  }

  private installIdentificationViews(): void {
    const host = document.querySelector<HTMLElement>("[data-identification-fit-plot]");
    if (!host) return;
    host.className = "analysis-plot";
    this.installAnalysisView("fit", host);
    this.installAnalysisView("residuals");
    this.installAnalysisView("validation-data");
    this.installAnalysisView("poles-zeros");
  }

  private installAnalysisView(id: string, existingHost?: HTMLElement): void {
    const plotWrap = document.querySelector<HTMLElement>(".plot-wrap");
    if (!plotWrap) return;
    const view = document.createElement("div");
    view.className = "plot-view";
    view.dataset.plotView = id;
    view.hidden = true;
    const host = existingHost ?? document.createElement("div");
    host.className = "analysis-plot";
    host.dataset.identificationPlot = id;
    host.tabIndex = 0;
    host.setAttribute("role", "img");
    host.setAttribute("aria-label", plotLabel(id));
    view.append(host);
    plotWrap.append(view);
  }

  private installMimoViews(): void {
    this.installMimoView("channel-responses");
    this.installMimoView("rga");
    this.installMimoView("singular-values");
    this.installMimoView("interaction");
  }

  private installMimoView(id: string): void {
    const plotWrap = document.querySelector<HTMLElement>(".plot-wrap");
    if (!plotWrap) return;
    const view = document.createElement("div");
    view.className = "plot-view";
    view.dataset.plotView = id;
    view.hidden = true;
    const host = document.createElement("div");
    host.className = id === "rga" ? "signal-list" : "analysis-plot";
    host.dataset.mimoPlot = id;
    host.tabIndex = 0;
    host.setAttribute("role", id === "rga" ? "table" : "img");
    host.setAttribute("aria-label", plotLabel(id));
    view.append(host);
    plotWrap.append(view);
  }

  private installGraphicalDesignHandle(): void {
    const handle = document.querySelector<HTMLElement>(".design-handle");
    const row = document.querySelector<HTMLElement>("[data-control-designs]");
    const range = document.querySelector<HTMLInputElement>("[data-design-handle=\"proportionalGain\"]");
    const exact = document.querySelector<HTMLInputElement>("[data-design-exact=\"proportionalGain\"]");
    const error = document.querySelector<HTMLElement>("[data-design-error]");
    if (!handle || !row || !range || !exact || !error) return;
    const textNode = handle.firstChild;
    if (textNode?.nodeType === Node.TEXT_NODE) {
      const label = document.createElement("span");
      label.className = "design-label";
      label.textContent = textNode.textContent?.trim() ?? "Proportional gain (Kp)";
      handle.replaceChild(label, textNode);
    }
    handle.hidden = true;
    row.append(handle);
    const commit = (value: number): void => {
      range.value = exact.value = String(value);
      exact.setCustomValidity("");
      error.textContent = "";
      this.editGain(value);
    };
    range.addEventListener("input", () => commit(Number(range.value)));
    exact.addEventListener("change", () => {
      const result = parseGraphicalGain(exact.value, Number(range.min), Number(range.max));
      if (!result.ok) {
        exact.setCustomValidity(result.reason);
        error.textContent = result.reason;
        return;
      }
      commit(result.value);
    });
  }

  setMonitor(sample: LoopMonitorSample | undefined): void {
    this.evidence.samples = sample?.availability === "available";
    if (sample?.availability === "available") {
      this.adapter.setOutputLimits(sample.outputLimits.minimum, sample.outputLimits.maximum);
    }
    this.renderSignals(sample);
    this.renderTabs();
  }

  setRecordingAvailable(available: boolean): void {
    this.evidence.recording = available;
    this.renderTabs();
  }

  setModelAvailable(available: boolean): void {
    this.evidence.model = available;
    this.renderTabs();
  }

  setConstraintsAvailable(available: boolean): void {
    this.evidence.constraints = available;
    this.renderTabs();
  }

  setMimoAvailable(available: boolean): void {
    this.evidence.mimo = available;
    this.renderTabs();
  }

  setCandidate(candidate: ControlPidCandidate): void {
    this.evidence.candidate = true;
    const boundary = this.boundary();
    const snapshot = { revision: ++this.revision, candidate: copyValue(candidate) };
    if (!this.history) this.history = createDesignHistory(snapshot, boundary);
    else if (JSON.stringify(this.history.current.candidate.parameters) !== JSON.stringify(candidate.parameters)) {
      this.history = commitDesignEdit(this.history, snapshot, boundary);
    } else {
      this.revision -= 1;
      snapshot.revision = this.history.current.revision;
    }
    const range = document.querySelector<HTMLInputElement>("[data-design-handle=\"proportionalGain\"]");
    const exact = document.querySelector<HTMLInputElement>("[data-design-exact=\"proportionalGain\"]");
    if (range && exact) {
      const maximum = String(Math.max(20, candidate.parameters.proportionalGain * 4));
      range.max = exact.max = maximum;
      range.value = exact.value = String(candidate.parameters.proportionalGain);
      const processUnit = this.root.dataset.processUnit ?? "process";
      const outputUnit = this.root.dataset.outputUnit ?? "output";
      const unit = document.querySelector<HTMLElement>("[data-design-unit]");
      if (unit) unit.textContent = pidEngineeringUnits(processUnit, outputUnit).proportional;
    }
    this.renderDesigns();
    this.renderTabs();
    this.saveSessionState();
  }

  restoreSession(value: unknown): void {
    const session = decodeControlChartSession(value, this.boundary());
    if (!session) return;
    Object.assign(this.selectedPlots, session.selectedPlots);
    this.selectedSignals = new Set(session.selectedSignals);
    this.stage = session.activeStage;
    this.root.dataset.plotExpanded = String(session.expanded);
    this.adapter.restoreWindow(session.window);
    this.pins = (session.pinnedDesigns ?? []).map((pin) => ({
      candidate: { revision: pin.revision, candidate: pin.candidate as unknown as ControlPidCandidate },
      context: pin.context,
    }));
    window.dispatchEvent(new CustomEvent("trust-control-studio-restore-stage", {
      detail: { stage: session.activeStage },
    }));
    this.renderDesigns();
    this.renderTabs();
    const button = document.querySelector<HTMLButtonElement>("[data-chart-action=\"expand\"]");
    button?.setAttribute("aria-pressed", String(session.expanded));
    if (button) button.textContent = session.expanded ? "Restore" : "Expand";
  }

  private setStage(stage: ControlWorkflowStage): void {
    this.stage = stage;
    this.renderTabs();
  }

  private renderTabs(): void {
    const host = document.querySelector<HTMLElement>("[data-control-plot-tabs]");
    if (!host) return;
    const tabs = controlStagePlots(this.stage, this.evidence);
    const selected = selectControlPlot(tabs, this.selectedPlots[this.stage]);
    if (selected) this.selectedPlots[this.stage] = selected;
    host.setAttribute("aria-label", `Plots for ${this.stage}`);
    host.replaceChildren(...tabs.map((tab) => {
      const button = document.createElement("button");
      button.type = "button";
      button.role = "tab";
      button.textContent = plotLabel(tab.id);
      button.dataset.plotTab = tab.id;
      button.setAttribute("aria-selected", String(tab.id === selected));
      button.setAttribute("aria-disabled", String(!tab.available));
      button.disabled = !tab.available;
      if (tab.reason) button.title = tab.reason;
      button.addEventListener("click", () => {
        if (!tab.available) this.showUnavailable(tab.reason ?? "Required evidence is unavailable.");
        else {
          this.selectedPlots[this.stage] = tab.id;
          this.renderTabs();
        }
      });
      return button;
    }));
    const active = tabs.find((tab) => tab.id === selected);
    this.root.dataset.plotAvailable = String(Boolean(active?.available));
    if (active?.available) this.showPlot(selected!);
    else this.showUnavailable(active?.reason ?? "Required evidence is unavailable.");
    for (const [action, enabled] of Object.entries(
      directChartActionAvailability(Boolean(active?.available))
    )) {
      const button = document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`);
      if (button) button.disabled = !enabled;
    }
    host.querySelector<HTMLElement>("[aria-selected=\"true\"]")
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    const tuning = this.stage === "Tune" && this.evidence.candidate;
    for (const action of ["undo", "redo"] as const) {
      const button = document.querySelector<HTMLButtonElement>(`[data-chart-action="${action}"]`);
      if (button) button.hidden = !tuning;
    }
    const designHandle = document.querySelector<HTMLElement>(".design-handle");
    if (designHandle) {
      designHandle.hidden = !(tuning && selected !== undefined &&
        ["bode", "root-locus", "nichols"].includes(selected));
    }
    this.updateHistoryActions();
    this.saveSessionState();
  }

  private showPlot(id: string): void {
    const mapped = controlPlotView(this.stage, id);
    let found = false;
    for (const view of document.querySelectorAll<HTMLElement>("[data-plot-view]")) {
      const active = view.dataset.plotView === mapped;
      view.hidden = !active;
      found ||= active;
    }
    if (!found) this.showUnavailable(`${plotLabel(id)} is waiting for its qualified analysis result.`);
    this.adapter.resize();
    window.dispatchEvent(new CustomEvent("trust-control-studio-plot-shown", {
      detail: { id: mapped },
    }));
  }

  private showUnavailable(reason: string): void {
    for (const view of document.querySelectorAll<HTMLElement>("[data-plot-view]")) {
      view.hidden = view.dataset.plotView !== "unavailable";
    }
    const message = document.querySelector<HTMLElement>("[data-plot-unavailable]");
    if (message) message.textContent = reason;
  }

  private renderSignals(sample: LoopMonitorSample | undefined): void {
    const host = document.querySelector<HTMLElement>("[data-control-signals]");
    if (!host) return;
    if (sample?.availability !== "available") {
      host.textContent = "No current signals. Start the runtime to inspect signal values.";
      return;
    }
    const signals = [
      ...(sample.setpoint ? [{ id: "sp", name: "Setpoint", role: "SP", channel: "Primary loop", ...sample.setpoint }] : []),
      { id: "pv", name: "Process value", role: "PV", channel: "Primary loop", ...sample.processVariable },
      { id: "mv", name: "Controller output", role: "MV", channel: "Primary loop", ...sample.manipulatedVariable },
      ...(sample.additional ?? []).map((signal) => ({ name: signal.symbol, ...signal })),
    ];
    host.replaceChildren(...signals.map((signal) => {
      const row = document.createElement("label");
      row.className = "signal-row";
      const checked = document.createElement("input");
      checked.type = "checkbox";
      checked.checked = this.selectedSignals.has(signal.id);
      this.adapter.setSignalSelected(signal.id, checked.checked);
      checked.addEventListener("change", () => {
        if (checked.checked) this.selectedSignals.add(signal.id);
        else this.selectedSignals.delete(signal.id);
        this.adapter.setSignalSelected(signal.id, checked.checked);
        this.saveSessionState();
      });
      const name = document.createElement("span"); name.textContent = signal.name;
      const role = document.createElement("span"); role.className = "signal-secondary";
      role.textContent = `${signal.role} · ${signal.symbol}`;
      const value = document.createElement("span"); value.textContent = `${formatEngineeringNumber(signal.value)} ${signal.unit}`;
      const quality = document.createElement("span"); quality.className = "signal-secondary"; quality.textContent = signal.quality;
      row.append(checked, name, role, value, quality);
      return row;
    }));
  }

  private editGain(value: number): void {
    if (!this.history || !Number.isFinite(value) || value <= 0) return;
    const candidate = copyValue(this.history.current.candidate);
    candidate.parameters.proportionalGain = value;
    const active = document.querySelector<HTMLElement>("[data-active-design]");
    if (active) active.textContent = `Candidate · Kp ${formatEngineeringNumber(value, 4)} · Updating`;
    if (this.editTimer !== undefined) window.clearTimeout(this.editTimer);
    this.editTimer = window.setTimeout(() => {
      window.dispatchEvent(new CustomEvent("trust-control-studio-design-edit", {
        detail: { settings: candidate.parameters },
      }));
    }, prefersReducedMotion() ? 0 : 140);
  }

  private undo(): void {
    if (!this.history) return;
    this.history = undoDesignEdit(this.history, this.boundary());
    this.dispatchHistoryCandidate();
  }

  private redo(): void {
    if (!this.history) return;
    this.history = redoDesignEdit(this.history, this.boundary());
    this.dispatchHistoryCandidate();
  }

  private dispatchHistoryCandidate(): void {
    if (!this.history) return;
    window.dispatchEvent(new CustomEvent("trust-control-studio-design-edit", {
      detail: { settings: this.history.current.candidate.parameters },
    }));
    this.renderDesigns();
  }

  private pin(): void {
    if (!this.history) return;
    this.pins = pinComparableDesign(this.pins, this.history.current, {
      modelRevision: this.boundary(), evidenceRevision: this.boundary(), method: "qualified-v1",
    });
    this.renderDesigns();
  }

  private renderDesigns(): void {
    const row = document.querySelector<HTMLElement>("[data-control-designs]");
    if (row) row.hidden = !this.history;
    const active = document.querySelector<HTMLElement>("[data-active-design]");
    if (active && this.history) {
      const value = this.history.current.candidate;
      active.textContent = `Candidate ${this.history.current.revision} · ${value.structure} · Kp ${formatEngineeringNumber(value.parameters.proportionalGain, 4)}`;
    }
    const host = document.querySelector<HTMLElement>("[data-pinned-designs]");
    if (host) {
      host.replaceChildren(...this.pins.map((pin) => {
        const chip = document.createElement("span");
        chip.className = "design-chip";
        chip.textContent = `Pinned ${pin.candidate.revision} · Kp ${formatEngineeringNumber(pin.candidate.candidate.parameters.proportionalGain, 4)}`;
        return chip;
      }));
    }
    const pin = document.querySelector<HTMLButtonElement>("[data-chart-action=\"pin\"]");
    if (pin) pin.disabled = !this.history || this.pins.length >= 3;
    this.updateHistoryActions();
    this.saveSessionState();
  }

  private updateHistoryActions(): void {
    const undo = document.querySelector<HTMLButtonElement>("[data-chart-action=\"undo\"]");
    const redo = document.querySelector<HTMLButtonElement>("[data-chart-action=\"redo\"]");
    if (undo) undo.disabled = !this.history?.past.length;
    if (redo) redo.disabled = !this.history?.future.length;
  }

  private toggleExpanded(): void {
    const expanded = this.root.dataset.plotExpanded !== "true";
    this.root.dataset.plotExpanded = String(expanded);
    const button = document.querySelector<HTMLButtonElement>("[data-chart-action=\"expand\"]");
    button?.setAttribute("aria-pressed", String(expanded));
    if (button) button.textContent = expanded ? "Restore" : "Expand";
    requestAnimationFrame(() => this.adapter.resize());
    this.saveSessionState();
  }

  private saveSessionState(): void {
    const session = {
      schemaVersion: 1,
      boundary: this.boundary(),
      activeStage: this.stage,
      selectedPlots: this.selectedPlots,
      selectedSignals: [...this.selectedSignals],
      window: this.adapter.viewWindow(),
      expanded: this.root.dataset.plotExpanded === "true",
      ...(this.history ? { candidateRevision: this.history.current.revision } : {}),
      pinnedCandidateRevisions: this.pins.map((pin) => pin.candidate.revision),
      pinnedDesigns: this.pins.map((pin) => ({
        revision: pin.candidate.revision,
        candidate: pin.candidate.candidate as unknown as Record<string, unknown>,
        context: pin.context,
      })),
    };
    this.root.dataset.controlChartSession = JSON.stringify(session);
  }

  private boundary(): string {
    return `${this.root.dataset.loopId ?? "unbound"}:${this.root.dataset.loopRevision ?? "no-loop"}`;
  }
}

class EngineeringChartAdapter {
  private readonly time: echarts.ECharts;
  private readonly magnitude: echarts.ECharts;
  private readonly phase: echarts.ECharts;
  private readonly rootLocus: echarts.ECharts;
  private readonly nyquist: echarts.ECharts;
  private readonly nichols: echarts.ECharts;
  private readonly sensitivity: echarts.ECharts;
  private readonly requirements: echarts.ECharts;
  private readonly section: HTMLElement;
  private readonly summary: HTMLElement;
  private readonly table: HTMLTableSectionElement;

  constructor(timeHost: HTMLElement, magnitudeHost: HTMLElement, phaseHost: HTMLElement) {
    this.section = requiredElement<HTMLElement>("[data-engineering-plots]");
    this.summary = requiredElement<HTMLElement>("[data-engineering-plot-summary]");
    this.table = requiredElement<HTMLTableSectionElement>("[data-engineering-table]");
    this.time = echarts.init(timeHost, undefined, { renderer: "canvas" });
    this.magnitude = echarts.init(magnitudeHost, undefined, { renderer: "canvas" });
    this.phase = echarts.init(phaseHost, undefined, { renderer: "canvas" });
    this.magnitude.group = "control-studio-bode";
    this.phase.group = "control-studio-bode";
    echarts.connect("control-studio-bode");
    this.rootLocus = initEngineeringChart("[data-engineering-root-locus]");
    this.nyquist = initEngineeringChart("[data-engineering-nyquist]");
    this.nichols = initEngineeringChart("[data-engineering-nichols]");
    this.sensitivity = initEngineeringChart("[data-engineering-sensitivity]");
    this.requirements = initEngineeringChart("[data-engineering-requirements]");
    for (const host of [timeHost, magnitudeHost, phaseHost,
      requiredElement<HTMLElement>("[data-engineering-root-locus]"),
      requiredElement<HTMLElement>("[data-engineering-nyquist]"),
      requiredElement<HTMLElement>("[data-engineering-nichols]"),
      requiredElement<HTMLElement>("[data-engineering-sensitivity]"),
      requiredElement<HTMLElement>("[data-engineering-requirements]")]) {
      new ResizeObserver(() => this.resize()).observe(host);
    }
    window.addEventListener("trust-control-studio-plot-shown", () => {
      window.requestAnimationFrame(() => this.resize());
    });
  }

  setEvidence(evidence: EngineeringPlotMessageEvidence): void {
    const theme = chartTheme();
    const shared = {
      animation: false,
      aria: { enabled: true, decal: { show: true } },
      textStyle: { color: theme.foreground },
      legend: { top: 4, textStyle: { color: theme.muted } },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue,
        axisPointer: { type: "cross", label: { color: theme.foreground } } },
      grid: { left: 58, right: 50, top: 38, bottom: 44 },
      dataZoom: [{ type: "inside", filterMode: "none" }],
    };
    this.time.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme),
      yAxis: [axis("PV", "value", theme), axis("MV", "value", theme)],
      series: [
        engineeringLine("Current PV", 0, evidence.timeResponse.map((point) => [point.timeSeconds, point.currentPv]), "solid"),
        engineeringLine("Candidate PV", 0, evidence.timeResponse.map((point) => [point.timeSeconds, point.candidatePv]), "dashed"),
        engineeringLine("Current MV", 1, evidence.timeResponse.map((point) => [point.timeSeconds, point.currentMv]), "solid"),
        engineeringLine("Candidate MV", 1, evidence.timeResponse.map((point) => [point.timeSeconds, point.candidateMv]), "dashed"),
      ],
    });
    this.magnitude.setOption({ ...shared,
      xAxis: { ...axis("", "log", theme), axisLabel: { show: false } },
      yAxis: axis("Magnitude (dB)", "value", theme),
      series: bodeSeries(evidence, "magnitudeDb"),
    });
    this.phase.setOption({ ...shared,
      legend: { show: false },
      xAxis: axis("Frequency (rad/s)", "log", theme), yAxis: axis("Phase (°)", "value", theme),
      series: bodeSeries(evidence, "phaseDegrees"),
    });
    requiredElement<HTMLOutputElement>("[data-design-gain-margin]").value =
      `${formatEngineeringNumber(evidence.gainMarginDb, 2)} dB`;
    requiredElement<HTMLOutputElement>("[data-design-phase-margin]").value =
      `${formatEngineeringNumber(evidence.phaseMarginDegrees, 1)}°`;
    this.renderRootLocus(evidence, shared, theme);
    this.nyquist.setOption({ ...shared,
      xAxis: axis("Real", "value", theme), yAxis: axis("Imaginary", "value", theme),
      series: [
        engineeringLine("Open-loop contour", 0, evidence.nyquist.map((point) => [point.real, point.imaginary]), "solid"),
        { name: "Critical point −1 + 0j", type: "line", showSymbol: true, symbol: "diamond",
          symbolSize: 9, lineStyle: { width: 0 }, data: [[-1, 0]] },
      ],
    });
    this.nichols.setOption({ ...shared,
      xAxis: axis("Open-loop phase (°)", "value", theme),
      yAxis: axis("Open-loop magnitude (dB)", "value", theme),
      series: [engineeringLine("Candidate", 0,
        evidence.nichols.map((point) => [point.phaseDegrees, point.magnitudeDb]), "solid")],
    });
    this.sensitivity.setOption({ ...shared,
      xAxis: axis("rad/s", "log", theme), yAxis: axis("Magnitude (dB)", "value", theme),
      series: [
        engineeringLine("Sensitivity S", 0, evidence.sensitivity.map((point) =>
          [point.frequencyRadPerSecond, point.sensitivityMagnitudeDb]), "solid"),
        engineeringLine("Complementary sensitivity T", 0, evidence.sensitivity.map((point) =>
          [point.frequencyRadPerSecond, point.complementarySensitivityMagnitudeDb]), "dashed"),
        engineeringLine("Control sensitivity CS", 0, evidence.sensitivity.map((point) =>
          [point.frequencyRadPerSecond, point.controlSensitivityMagnitudeDb]), "dashed"),
      ],
    });
    this.renderRequirements(evidence, shared, theme);
    this.summary.textContent = `${evidence.evidence}; ${evidence.timeResponse.length} time points; `
      + `gain margin ${formatEngineeringNumber(evidence.gainMarginDb, 2)} dB; phase margin ${formatEngineeringNumber(evidence.phaseMarginDegrees, 1)} degrees.`;
    const stride = Math.max(1, Math.ceil(evidence.timeResponse.length / 25));
    this.table.replaceChildren(...evidence.timeResponse.filter((_, index) => index % stride === 0).map((point) => {
      const row = document.createElement("tr");
      for (const value of [point.timeSeconds, point.currentPv, point.candidatePv, point.currentMv, point.candidateMv]) {
        const cell = document.createElement("td"); cell.textContent = formatEngineeringNumber(value); row.append(cell);
      }
      return row;
    }));
    this.resize();
  }

  private renderRootLocus(
    evidence: EngineeringPlotMessageEvidence,
    shared: Record<string, unknown>,
    theme: ReturnType<typeof chartTheme>
  ): void {
    if (evidence.rootLocus.status !== "qualified") {
      this.rootLocus.clear();
      requiredElement<HTMLElement>("[data-engineering-root-locus]")
        .setAttribute("aria-description", evidence.rootLocus.reason);
      return;
    }
    const branchCount = Math.max(...evidence.rootLocus.branches.map((branch) => branch.poles.length));
    this.rootLocus.setOption({ ...shared,
      xAxis: axis("Real axis", "value", theme), yAxis: axis("Imaginary axis", "value", theme),
      series: Array.from({ length: branchCount }, (_, index) => engineeringLine(
        `Pole branch ${index + 1}`, 0,
        evidence.rootLocus.status === "qualified"
          ? evidence.rootLocus.branches.flatMap((branch) => branch.poles[index]
            ? [[branch.poles[index].real, branch.poles[index].imaginary]] : []) : [],
        index === 0 ? "solid" : "dashed"
      )),
    });
  }

  private renderRequirements(
    evidence: EngineeringPlotMessageEvidence,
    shared: Record<string, unknown>,
    theme: ReturnType<typeof chartTheme>
  ): void {
    const uncertainty = evidence.uncertainty.points;
    const lower = uncertainty.map((point) => [point.timeSeconds, point.lower]);
    const span = uncertainty.map((point) => [point.timeSeconds, point.upper - point.lower]);
    this.requirements.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme), yAxis: axis("Normalized response", "value", theme),
      series: [
        engineeringLine("Candidate", 0,
          evidence.timeResponse.map((point) => [point.timeSeconds, point.candidatePv]), "solid"),
        ...(uncertainty.length > 0 ? [
          { name: "Uncertainty lower", type: "line" as const, stack: "uncertainty", showSymbol: false,
            lineStyle: { width: 1, type: "dashed" }, areaStyle: { opacity: 0 }, data: lower },
          { name: `Uncertainty · ${evidence.uncertainty.method}`, type: "line" as const,
            stack: "uncertainty", showSymbol: false, lineStyle: { width: 0 },
            areaStyle: { opacity: 0.14 }, data: span },
        ] : []),
      ],
    });
    const summary = evidence.requirements.map((item) =>
      `${item.name}: ${item.status} (${formatEngineeringNumber(item.value)} ${item.unit}; ${item.direction} ${formatEngineeringNumber(item.limit)} ${item.unit})`).join(". ");
    requiredElement<HTMLElement>("[data-engineering-requirements]").setAttribute("aria-description", summary);
  }

  private resize(): void {
    for (const chart of [this.time, this.magnitude, this.phase, this.rootLocus,
      this.nyquist, this.nichols, this.sensitivity, this.requirements]) chart.resize();
  }
}

class MimoChartAdapter {
  private readonly channelResponses = initEngineeringChart("[data-mimo-plot=\"channel-responses\"]");
  private readonly singularValues = initEngineeringChart("[data-mimo-plot=\"singular-values\"]");
  private readonly interaction = initEngineeringChart("[data-mimo-plot=\"interaction\"]");
  private readonly rga = requiredElement<HTMLElement>("[data-mimo-plot=\"rga\"]");
  private evidence: MimoPlotEvidence | undefined;

  constructor() {
    for (const host of document.querySelectorAll<HTMLElement>("[data-mimo-plot]")) {
      if (host.dataset.mimoPlot !== "rga") new ResizeObserver(() => this.resize()).observe(host);
    }
    window.addEventListener("trust-control-studio-plot-shown", (event) => {
      const id = (event as CustomEvent<{ id?: string }>).detail?.id;
      if (["channel-responses", "singular-values", "interaction"].includes(id ?? "")) {
        window.requestAnimationFrame(() => {
          if (this.evidence) this.renderEvidence(this.evidence);
          else this.resize();
        });
      }
    });
  }

  setEvidence(evidence: MimoPlotEvidence): void {
    this.evidence = copyValue(evidence);
    this.renderEvidence(this.evidence);
  }

  private renderEvidence(evidence: MimoPlotEvidence): void {
    const theme = chartTheme();
    requiredElement<HTMLElement>("[data-mimo-plot=\"channel-responses\"]").setAttribute(
      "aria-label",
      `MIMO channel responses for ${evidence.outputs.map((output) => output.name).join(", ")} from ${evidence.inputs.map((input) => input.name).join(", ")}`
    );
    requiredElement<HTMLElement>("[data-mimo-plot=\"singular-values\"]").setAttribute(
      "aria-label", `MIMO singular values for model ${evidence.modelRevision}`
    );
    const columns = evidence.inputs.length;
    const rows = evidence.outputs.length;
    const gap = 4;
    const left = 8;
    const top = 8;
    const cellWidth = (92 - gap * (columns - 1)) / columns;
    const cellHeight = (76 - gap * (rows - 1)) / rows;
    const grids: Record<string, unknown>[] = [];
    const xAxes: Record<string, unknown>[] = [];
    const yAxes: Record<string, unknown>[] = [];
    const titles: Record<string, unknown>[] = [];
    const series: Record<string, unknown>[] = [];
    evidence.outputs.forEach((output, outputIndex) => evidence.inputs.forEach((input, inputIndex) => {
      const index = outputIndex * columns + inputIndex;
      grids.push({ left: `${left + inputIndex * (cellWidth + gap)}%`,
        top: `${top + outputIndex * (cellHeight + gap)}%`, width: `${cellWidth}%`, height: `${cellHeight}%`,
        containLabel: true });
      xAxes.push({ ...axis(outputIndex === rows - 1 ? `Time (s) · ${input.name}` : "", "value", theme),
        gridIndex: index });
      yAxes.push({ ...axis(`${output.name} (${output.unit})`, "value", theme), gridIndex: index });
      titles.push({ text: `${output.name} ← ${input.name}`, left: `${left + inputIndex * (cellWidth + gap)}%`,
        top: `${Math.max(0, top + outputIndex * (cellHeight + gap) - 3)}%`,
        textStyle: { color: theme.muted, fontSize: 10, fontWeight: "normal" } });
      const response = evidence.channelResponses.find((item) =>
        item.inputId === input.id && item.outputId === output.id)!;
      series.push({ ...engineeringLine(`${output.name} ← ${input.name}`, 0,
        response.points.map((point) => [...point]), "solid"), xAxisIndex: index, yAxisIndex: index });
    }));
    this.channelResponses.setOption({ animation: false, aria: { enabled: false, decal: { show: true } },
      tooltip: { trigger: "axis", confine: true }, title: titles, grid: grids, xAxis: xAxes, yAxis: yAxes, series });
    this.singularValues.setOption({ animation: false, aria: { enabled: false, decal: { show: true } },
      tooltip: { trigger: "axis", confine: true }, legend: { top: 4, textStyle: { color: theme.muted } },
      grid: { left: 58, right: 30, top: 38, bottom: 44 }, xAxis: axis("Frequency (rad/s)", "log", theme),
      yAxis: axis("Singular value", "log", theme),
      series: evidence.inputs.map((_, index) => engineeringLine(`σ${index + 1}`, 0,
        evidence.singularValues.map((point) => [point.frequencyRadPerSecond, point.values[index]]),
        index === 0 ? "solid" : "dashed")),
    });
    this.interaction.setOption({ animation: false, aria: { enabled: false, decal: { show: true } },
      tooltip: { trigger: "axis", confine: true }, legend: { top: 4, textStyle: { color: theme.muted } },
      grid: { left: 58, right: 30, top: 38, bottom: 54 },
      xAxis: { type: "category", data: evidence.inputs.map((input) => input.name),
        name: "Manipulated input", nameLocation: "middle", nameGap: 34,
        axisLabel: { color: theme.muted }, axisLine: { lineStyle: { color: theme.border } } },
      yAxis: axis("Relative gain λ", "value", theme),
      series: evidence.outputs.map((output, row) => engineeringLine(output.name, 0,
        evidence.relativeGainArray[row].map((value, column) => [column, value]), row === 0 ? "solid" : "dashed")),
    });
    this.renderRga(evidence);
    this.resize();
  }

  private renderRga(evidence: MimoPlotEvidence): void {
    const table = document.createElement("table");
    const caption = document.createElement("caption");
    caption.textContent = `Relative Gain Array · model ${evidence.modelRevision} · evidence ${evidence.evidenceRevision}`;
    table.append(caption);
    const head = document.createElement("thead");
    const headRow = document.createElement("tr");
    headRow.append(document.createElement("th"));
    for (const input of evidence.inputs) {
      const cell = document.createElement("th"); cell.scope = "col"; cell.textContent = `${input.name} (${input.unit})`; headRow.append(cell);
    }
    head.append(headRow); table.append(head);
    const body = document.createElement("tbody");
    evidence.outputs.forEach((output, rowIndex) => {
      const row = document.createElement("tr");
      const label = document.createElement("th"); label.scope = "row"; label.textContent = `${output.name} (${output.unit})`; row.append(label);
      evidence.relativeGainArray[rowIndex].forEach((value) => {
        const cell = document.createElement("td"); cell.textContent = formatEngineeringNumber(value, 3); row.append(cell);
      });
      body.append(row);
    });
    table.append(body); this.rga.replaceChildren(table);
  }

  private resize(): void {
    const charts = [
      [this.channelResponses, "[data-mimo-plot=\"channel-responses\"]"],
      [this.singularValues, "[data-mimo-plot=\"singular-values\"]"],
      [this.interaction, "[data-mimo-plot=\"interaction\"]"],
    ] as const;
    for (const [chart, selector] of charts) {
      const rect = requiredElement<HTMLElement>(selector).getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        chart.resize({ width: rect.width, height: rect.height });
      }
    }
  }
}

class IdentificationChartAdapter {
  private readonly fit: echarts.ECharts;
  private readonly residuals: echarts.ECharts;
  private readonly validation: echarts.ECharts;
  private readonly polesZeros: echarts.ECharts;
  private readonly summary: HTMLElement;

  constructor(host: HTMLElement) {
    this.summary = requiredElement<HTMLElement>("[data-identification-fit-summary]");
    this.fit = echarts.init(host, undefined, { renderer: "canvas" });
    this.residuals = initEngineeringChart("[data-identification-plot=\"residuals\"]");
    this.validation = initEngineeringChart("[data-identification-plot=\"validation-data\"]");
    this.polesZeros = initEngineeringChart("[data-identification-plot=\"poles-zeros\"]");
    for (const chartHost of document.querySelectorAll<HTMLElement>("[data-identification-plot]")) {
      new ResizeObserver(() => this.resize()).observe(chartHost);
    }
  }

  setEvidence(evidence: IdentifiedModelPresentation): void {
    const theme = chartTheme();
    const outputRange = identificationOutputAxisRange(evidence.predictedResponse.flatMap((point) =>
      [point.measured, point.predicted]));
    const shared = {
      animation: false,
      aria: { enabled: true, decal: { show: true } },
      textStyle: { color: theme.foreground },
      legend: { top: 4, textStyle: { color: theme.muted } },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue },
      grid: { left: 58, right: 58, top: 38, bottom: 44 },
    };
    this.fit.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme),
      yAxis: { ...axis(`PV (${evidence.units.output})`, "value", theme),
        min: outputRange.minimum, max: outputRange.maximum },
      series: [
        engineeringLine("Measured PV", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.measured]), "solid"),
        engineeringLine("Predicted PV", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.predicted]), "dashed"),
      ],
    });
    this.residuals.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme),
      yAxis: axis(`Residual (${evidence.units.output})`, "value", theme),
      series: [engineeringLine("Prediction residual", 0,
        evidence.predictedResponse.map((point) => [point.timeSeconds, point.residual]), "solid")],
    });
    this.validation.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme),
      yAxis: { ...axis(`PV (${evidence.units.output})`, "value", theme),
        min: outputRange.minimum, max: outputRange.maximum },
      series: [
        engineeringLine("Validation measurement", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.measured]), "solid"),
        engineeringLine("Model prediction", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.predicted]), "dashed"),
      ],
    });
    this.polesZeros.setOption({ ...shared,
      tooltip: { trigger: "item", confine: true, valueFormatter: formatChartValue },
      xAxis: axis("Real axis (1/s)", "value", theme),
      yAxis: axis("Imaginary axis (1/s)", "value", theme),
      series: [
        pointSeries("Poles", evidence.poles.map((value) => [value, 0]), "cross"),
        pointSeries("Zeros", evidence.zeros.map((value) => [value, 0]), "circle"),
      ],
    });
    this.summary.textContent = `${evidence.predictedResponse.length} measured and predicted points; ` +
      `estimation residual lag-one correlation ${formatEngineeringNumber(evidence.estimation.residualLagOneCorrelation, 4)}; ` +
      `validation residual lag-one correlation ${formatEngineeringNumber(evidence.validation.residualLagOneCorrelation, 4)}.`;
    this.resize();
  }

  private resize(): void {
    for (const chart of [this.fit, this.residuals, this.validation, this.polesZeros]) chart.resize();
  }
}

function initEngineeringChart(selector: string): echarts.ECharts {
  return echarts.init(requiredElement<HTMLElement>(selector), undefined, { renderer: "canvas" });
}

function axis(name: string, type: "value" | "log", theme: ReturnType<typeof chartTheme>) {
  return { type, name, nameLocation: "middle", nameGap: 30, nameTextStyle: { color: theme.muted },
    axisLabel: { color: theme.muted, formatter: formatEngineeringNumber, hideOverlap: true, margin: 8 },
    axisLine: { show: false }, axisTick: { show: false }, splitNumber: 5,
    splitLine: { lineStyle: { color: theme.border, opacity: 0.18 } } };
}

function engineeringLine(name: string, yAxisIndex: number, data: number[][], style: "solid" | "dashed") {
  return { name, type: "line" as const, yAxisIndex, showSymbol: false, lineStyle: { width: 2, type: style }, data };
}

function pointSeries(name: string, data: number[][], symbol: "circle" | "cross") {
  return { name, type: "line" as const, showSymbol: true, symbol, symbolSize: 10,
    lineStyle: { width: 0 }, data };
}

function bodeSeries(evidence: EngineeringPlotMessageEvidence, field: "magnitudeDb" | "phaseDegrees") {
  return [
    engineeringLine("Plant", 0, evidence.plantBode.map((point) => [point.frequencyRadPerSecond, point[field]]), "solid"),
    engineeringLine("Candidate open loop", 0,
      evidence.candidateOpenLoopBode.map((point) => [point.frequencyRadPerSecond, point[field]]), "dashed"),
  ];
}

function line(
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

function chartTheme() {
  const style = getComputedStyle(document.documentElement);
  const foreground = css(style, "--vscode-editor-foreground", "CanvasText");
  return {
    foreground,
    muted: css(style, "--vscode-descriptionForeground", foreground),
    border: css(style, "--vscode-panel-border", foreground),
    background: css(style, "--vscode-editor-background", "Canvas"),
    sp: css(style, "--vscode-charts-blue", foreground),
    pv: css(style, "--vscode-charts-green", foreground),
    mv: css(style, "--vscode-charts-red", css(style, "--vscode-charts-orange", foreground)),
  };
}

function css(style: CSSStyleDeclaration, name: string, fallback: string): string {
  return style.getPropertyValue(name).trim() || fallback;
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Control Studio chart element missing: ${selector}`);
  return element;
}

function spoken(value: ControlChartValue): string {
  return value === null ? "missing" : formatEngineeringNumber(value);
}

function formatChartValue(value: unknown): string {
  return formatEngineeringNumber(Array.isArray(value) ? value[value.length - 1] : value);
}

function download(url: string, filename: string): void {
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
}

function plotLabel(id: string): string {
  return id.split("-").map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(" ")
    .replace("Rga", "RGA");
}

function copyValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

const host = document.querySelector<HTMLElement>("[data-control-studio-chart]");
const studio = document.querySelector<HTMLElement>("[data-control-studio]");
const adapter = host ? new ControlChartAdapter(host) : undefined;
const workspace = host && studio && adapter ? new ControlChartWorkspace(studio, adapter) : undefined;
const mimo = workspace ? new MimoChartAdapter() : undefined;
const engineeringTime = document.querySelector<HTMLElement>("[data-engineering-time]");
const engineeringMagnitude = document.querySelector<HTMLElement>("[data-engineering-bode-magnitude]");
const engineeringPhase = document.querySelector<HTMLElement>("[data-engineering-bode-phase]");
const engineering = engineeringTime && engineeringMagnitude && engineeringPhase
  ? new EngineeringChartAdapter(engineeringTime, engineeringMagnitude, engineeringPhase) : undefined;
const identificationHost = document.querySelector<HTMLElement>("[data-identification-fit-plot]");
const identification = identificationHost ? new IdentificationChartAdapter(identificationHost) : undefined;
const receivedSamples: ControlChartSample[] = [];
const receivedMessages = new Map<string, string>();
let firstSimulationTime: bigint | undefined;
let firstLoopTimeMs: number | undefined;
let previousSaturation: ControlSaturationState | undefined;
adapter?.setSamples([]);

window.addEventListener("message", (event) => {
  if (!studio || !adapter) return;
  const raw = event.data as Record<string, unknown> | undefined;
  if (raw?.kind === "control-studio.mimo.result" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    const evidence = decodeMimoPlotEvidence(payload?.evidence);
    if (evidence) {
      mimo?.setEvidence(evidence);
      workspace?.setMimoAvailable(true);
      workspace?.setModelAvailable(true);
    }
    return;
  }
  if (raw?.kind === "control-studio.workflow-result" &&
      (raw.phase === "restored" || raw.phase === "opened") &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    const result = payload?.result as Record<string, unknown> | undefined;
    const recording = result?.recording as Record<string, unknown> | undefined;
    const samples = Array.isArray(recording?.rawSamples) ? recording.rawSamples : [];
    const units = result?.units as Record<string, unknown> | undefined;
    const controller = result?.controller as Record<string, unknown> | undefined;
    const restored = restoredControlChartSamples(samples as Parameters<typeof restoredControlChartSamples>[0]);
    adapter.setSamples(restored);
    workspace?.setRecordingAvailable(restored.length > 0);
    workspace?.setModelAvailable(Boolean(result?.model));
    if (result?.candidate) workspace?.setCandidate(result.candidate as unknown as ControlPidCandidate);
    workspace?.restoreSession(result?.chartSession);
    adapter.setSource(String(result?.source ?? "Saved result"));
    adapter.setUnits(String(units?.pv ?? "engineering unit"), String(units?.mv ?? "%"));
    const last = samples[samples.length - 1] as Record<string, unknown> | undefined;
    setText("[data-control-studio-source]", result?.source === "Physical" ? "Physical plant" : "Simulation");
    setText("[data-control-studio-connection]", "Saved result");
    setText("[data-control-studio-freshness]", `Historical · revision ${String(result?.resultRevision ?? "—")}`);
    setText("[data-control-studio-controller-mode]", String(last?.controllerMode ?? "Unavailable"));
    setText("[data-control-studio-sp]", savedReading(last?.sp, units?.sp));
    setText("[data-control-studio-pv]", savedReading(last?.pv, units?.pv));
    setText("[data-control-studio-mv]", savedReading(last?.appliedMv ?? last?.mv, units?.mv));
    const limits = last?.outputLimits as Record<string, unknown> | undefined;
    if (typeof limits?.minimum === "number" && typeof limits.maximum === "number") {
      adapter.setOutputLimits(limits.minimum, limits.maximum);
    }
    setText("[data-control-studio-quality]", `${String(last?.quality ?? "Unavailable")} · Output ${formatEngineeringNumber(limits?.minimum)}…${formatEngineeringNumber(limits?.maximum)} ${String(units?.mv ?? "")}`);
    const revision = document.querySelector<HTMLElement>("[data-control-studio-controller-revision]");
    if (revision) {
      revision.textContent = compactRevisionLabel(String(controller?.revision ?? "Unavailable"));
      revision.title = String(controller?.revision ?? "Unavailable");
    }
    return;
  }
  if (raw?.kind === "control-studio.engineering.result" && raw.stage === "tune" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    if (payload?.status === "complete" && payload.plots) {
      engineering?.setEvidence(payload.plots as EngineeringPlotMessageEvidence);
      workspace?.setModelAvailable(true);
      workspace?.setConstraintsAvailable(true);
      if (payload.candidate) workspace?.setCandidate(payload.candidate as unknown as ControlPidCandidate);
    }
    return;
  }
  if (raw?.kind === "control-studio.engineering.result" && raw.stage === "identify" &&
      raw.projectId === studio.dataset.projectId && raw.editorSessionId === studio.dataset.editorSessionId &&
      raw.loopId === studio.dataset.loopId && raw.expectedLoopRevision === studio.dataset.loopRevision) {
    const payload = raw.payload as Record<string, unknown> | undefined;
    if (payload?.status === "complete" && payload.presentation) {
      identification?.setEvidence(payload.presentation as unknown as IdentifiedModelPresentation);
      workspace?.setModelAvailable(true);
    }
    return;
  }
  if (studio.dataset.loopId && studio.dataset.loopRevision) {
    const loopEnvelope = decodeLoopMonitorEnvelope(event.data, {
      projectId: studio.dataset.projectId ?? "",
      editorSessionId: studio.dataset.editorSessionId ?? "",
      loopId: studio.dataset.loopId,
      loopRevision: studio.dataset.loopRevision,
    });
    if (loopEnvelope) {
      if (!acceptMessage(loopEnvelope.messageId, loopEnvelope)) return;
      renderLoopSample(loopEnvelope.payload, adapter);
      workspace?.setMonitor(loopEnvelope.payload);
      return;
    }
  }
  const envelope = decodePlantMonitorEnvelope(event.data, {
    projectId: studio.dataset.projectId ?? "",
    editorSessionId: studio.dataset.editorSessionId ?? "",
    plantId: studio.dataset.plantId ?? "",
    plantRevision: studio.dataset.plantRevision ?? "",
  });
  if (!envelope) return;
  if (!acceptMessage(envelope.messageId, envelope)) return;
  const snapshot = envelope.payload;
  requiredElement<HTMLElement>("[data-plant-summary]").hidden = false;
  if (snapshot.availability === "unavailable") {
    setText("[data-control-studio-connection]", connectionForReason(snapshot.reason));
    setText("[data-control-studio-freshness]", snapshot.reason);
    setText("[data-control-studio-quality]", `Unavailable · ${snapshot.reason}`);
    setText("[data-control-studio-sp]", "—");
    setText("[data-control-studio-pv]", "—");
    setText("[data-control-studio-mv]", "—");
    setText("[data-plant-state]", "Unavailable");
    setText("[data-plant-command]", "—");
    setText("[data-plant-measured]", "—");
    setText("[data-plant-published]", "—");
    setText("[data-plant-fault]", snapshot.reason);
    return;
  }

  const timestamp = BigInt(snapshot.simulationTimeNs);
  firstSimulationTime ??= timestamp;
  receivedSamples.push({
    elapsedSeconds: Number(timestamp - firstSimulationTime) / 1_000_000_000,
    sp: null,
    pv: first(snapshot.publishedFeedback),
    mv: first(snapshot.command),
    quality: snapshot.operatingState === "faulted" ? "bad" : "good",
    events: snapshot.currentFault
      ? [{ kind: "event", label: snapshot.currentFault }]
      : undefined,
  });
  if (receivedSamples.length > 12_000) receivedSamples.splice(0, receivedSamples.length - 12_000);
  adapter.setSamples(receivedSamples);

  setText("[data-control-studio-connection]", "Running");
  setText("[data-control-studio-freshness]", `Current · ${formatSimulationTime(snapshot.simulationTimeNs)} simulated`);
  setText("[data-control-studio-quality]", snapshot.operatingState === "faulted"
    ? `Fault · ${snapshot.currentFault ?? "Plant faulted"}`
    : "Good · Current simulation sample");
  setText("[data-control-studio-sp]", "—");
  setText("[data-control-studio-pv]", display(first(snapshot.publishedFeedback)));
  setText("[data-control-studio-mv]", display(first(snapshot.command)));
  setText("[data-plant-state]", titleCase(snapshot.operatingState));
  setText("[data-plant-command]", vector(snapshot.command));
  setText("[data-plant-measured]", vector(snapshot.measuredFeedback));
  setText("[data-plant-published]", vector(snapshot.publishedFeedback));
  setText("[data-plant-fault]", snapshot.currentFault ?? "None");
  setText("[data-plant-model]", snapshot.model);
  setText("[data-plant-applied]", vector(snapshot.appliedInput));
  setText("[data-plant-ideal]", vector(snapshot.idealOutput));
  setText("[data-plant-vector]", vector(snapshot.state));
  setText("[data-plant-time-scale]", String(snapshot.timeScale));
  setText("[data-plant-seed]", snapshot.effectiveSeed);
  setText("[data-plant-random]", `${snapshot.randomAlgorithm} v${snapshot.randomAlgorithmVersion}`);
  setText("[data-plant-effects]", list(snapshot.activeUncertainties));
  setText("[data-plant-dropouts]", list(snapshot.activeDropouts));
  setText("[data-plant-events]", String(snapshot.pendingEventCount));
  setText("[data-plant-delay]", String(snapshot.delayBufferDepth));
  setText("[data-plant-substeps]", String(snapshot.lastSubsteps));
  setText("[data-plant-runtime]", snapshot.runtimeVersion);
});

function renderLoopSample(
  snapshot: LoopMonitorSample,
  chart: ControlChartAdapter
): void {
  setText("[data-control-studio-source]", snapshot.source === "Physical" ? "Physical plant" : "Simulation");
  const studio = document.querySelector<HTMLElement>("[data-control-studio]");
  if (studio) studio.dataset.controlSource = snapshot.source.toLowerCase();
  const physicalTopology = document.querySelector<HTMLElement>("[data-physical-topology]");
  if (physicalTopology) physicalTopology.hidden = snapshot.source !== "Physical";
  if (snapshot.source !== "Physical") {
    const reviewed = document.querySelector<HTMLInputElement>("[data-physical-topology-reviewed]");
    if (reviewed) reviewed.checked = false;
  }
  if (snapshot.availability === "unavailable") {
    setText("[data-control-studio-connection]", connectionForReason(snapshot.reason));
    setText("[data-control-studio-freshness]", `Historical · ${snapshot.reason}`);
    setText("[data-control-studio-quality]", `Unavailable · ${snapshot.reason}`);
    setText("[data-control-studio-sp]", "—");
    setText("[data-control-studio-pv]", "—");
    setText("[data-control-studio-mv]", "—");
    return;
  }
  firstLoopTimeMs ??= snapshot.observedAtMs;
  chart.setSource(snapshot.source);
  chart.setUnits(snapshot.processVariable.unit, snapshot.manipulatedVariable.unit);
  const quality = [snapshot.setpoint, snapshot.processVariable, snapshot.manipulatedVariable]
    .filter((signal): signal is NonNullable<typeof signal> => signal !== undefined)
    .some((signal) => signal.quality === "forced") ? "bad" : "good";
  receivedSamples.push({
    elapsedSeconds: (snapshot.observedAtMs - firstLoopTimeMs) / 1_000,
    sp: snapshot.setpoint?.value ?? null,
    pv: snapshot.processVariable.value,
    mv: snapshot.manipulatedVariable.value,
    quality,
    events: (() => {
      const transition = controlChartSaturationEvent(previousSaturation, snapshot.saturation);
      previousSaturation = snapshot.saturation;
      return transition ? [transition] : undefined;
    })(),
  });
  if (receivedSamples.length > 12_000) receivedSamples.splice(0, receivedSamples.length - 12_000);
  chart.setSamples(receivedSamples);
  setText("[data-control-studio-connection]", "Running");
  setText("[data-control-studio-controller-mode]", snapshot.controller.mode);
  const revision = document.querySelector<HTMLElement>("[data-control-studio-controller-revision]");
  if (revision) {
    revision.textContent = compactRevisionLabel(snapshot.controller.revision);
    revision.title = snapshot.controller.revision;
  }
  setText("[data-control-studio-freshness]", `Current · scan ${snapshot.scan}`);
  setText("[data-control-studio-sp]", snapshot.setpoint ? reading(snapshot.setpoint) : "Open loop");
  setText("[data-control-studio-pv]", reading(snapshot.processVariable));
  setText("[data-control-studio-mv]", reading(snapshot.manipulatedVariable));
  const qualityText = quality === "good" ? "Good" : "Forced signal";
  setText("[data-control-studio-quality]",
    `${qualityText} · Output ${formatEngineeringNumber(snapshot.outputLimits.minimum)}…${formatEngineeringNumber(snapshot.outputLimits.maximum)} ${displayEngineeringUnit(snapshot.manipulatedVariable.unit)} · Saturation ${snapshot.saturation}`);
  const start = document.querySelector<HTMLButtonElement>("[data-control-studio-record-start]");
  const stop = document.querySelector<HTMLButtonElement>("[data-control-studio-record-stop]");
  if (start) start.disabled = snapshot.recording;
  if (stop) stop.disabled = !snapshot.recording;
}

function acceptMessage(messageId: string, envelope: unknown): boolean {
  if (receivedMessages.has(messageId)) return false;
  receivedMessages.set(messageId, JSON.stringify(envelope));
  if (receivedMessages.size > 1_024) {
    receivedMessages.delete(receivedMessages.keys().next().value as string);
  }
  return true;
}

function reading(signal: { value: number; unit: string }): string {
  const value = formatEngineeringNumber(signal.value);
  const unit = displayEngineeringUnit(signal.unit);
  return unit ? `${value} ${unit}` : value;
}

function savedReading(value: unknown, unit: unknown): string {
  const formatted = formatEngineeringNumber(value);
  const displayedUnit = displayEngineeringUnit(String(unit ?? ""));
  return formatted === "—" ? formatted : `${formatted}${displayedUnit ? ` ${displayedUnit}` : ""}`;
}

function setText(selector: string, value: string): void {
  requiredElement<HTMLElement>(selector).textContent = value;
}

function first(values: readonly number[]): number | null {
  return values.length > 0 ? values[0] : null;
}

function display(value: number | null): string {
  return formatEngineeringNumber(value);
}

function vector(values: readonly number[]): string {
  return values.length > 0 ? values.map((value) => formatEngineeringNumber(value)).join(", ") : "—";
}

function list(values: readonly string[]): string {
  return values.length > 0 ? values.join(", ") : "None";
}

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function connectionForReason(reason: string): string {
  if (reason === "Runtime is stopped") return "Stopped";
  if (reason === "Runtime is unreachable") return "Unreachable";
  if (reason === "Runtime authorization was rejected") return "Unauthorized";
  if (reason === "Plant status is unsupported") return "Unsupported";
  if (reason === "Running plant revision is stale") return "Stale";
  if (reason === "Runtime scan is stale") return "Stale";
  if (reason === "Plant no longer exists") return "Deleted";
  return "Unavailable";
}

function formatSimulationTime(nanoseconds: string): string {
  const value = BigInt(nanoseconds);
  if (value < 1_000_000n) return `${value / 1_000n} µs`;
  if (value < 1_000_000_000n) return `${value / 1_000_000n} ms`;
  const seconds = Number(value) / 1_000_000_000;
  return `${Number.isInteger(seconds) ? seconds : seconds.toFixed(3)} s`;
}
