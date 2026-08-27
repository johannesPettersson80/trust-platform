/// <reference lib="dom" />

import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import {
  AriaComponent,
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  TooltipComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

import {
  CONTROL_CHART_SIGNALS,
  controlChartAxisLabels,
  controlChartAccessibleSummary,
  controlChartSaturationEvent,
  compactRevisionLabel,
  type ControlSaturationState,
  type ControlChartSample,
  type ControlChartValue,
  controlChartLayout,
  controlChartCsv,
  displayedControlSamples,
  formatEngineeringNumber,
  normalizeChartWindow,
  restoredControlChartSamples,
} from "../chartContract";
import { decodeLoopMonitorEnvelope, decodePlantMonitorEnvelope } from "../messages";
import type { LoopMonitorSample } from "../loopMonitor";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";

echarts.use([
  LineChart,
  GridComponent,
  LegendComponent,
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
    this.emptyState = requiredElement<HTMLElement>(".plot-empty");
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
    if (processUnit === this.processUnit && outputUnit === this.outputUnit) return;
    this.processUnit = processUnit;
    this.outputUnit = outputUnit;
    const studio = document.querySelector<HTMLElement>("[data-control-studio]");
    if (studio) {
      studio.dataset.processUnit = processUnit;
      studio.dataset.outputUnit = outputUnit;
    }
    const labels = controlChartAxisLabels(processUnit, outputUnit);
    this.chart.setOption({ yAxis: [{ name: labels.process }, { name: labels.output }] });
    this.updateAccessibleSummary();
    this.compactLayout = undefined;
    this.applyResponsiveLayout(this.host.clientWidth);
  }

  setSource(source: string): void {
    this.source = source;
    this.updateAccessibleSummary();
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
      aria: { enabled: true, decal: { show: true } },
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
    download(this.chart.getDataURL({ type: "png", pixelRatio: 2, backgroundColor: chartTheme().background }), "control-studio-monitor.png");
    this.announce("Displayed plot exported as PNG");
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

class EngineeringChartAdapter {
  private readonly time: echarts.ECharts;
  private readonly magnitude: echarts.ECharts;
  private readonly phase: echarts.ECharts;
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
    for (const host of [timeHost, magnitudeHost, phaseHost]) {
      new ResizeObserver(() => this.resize()).observe(host);
    }
  }

  setEvidence(evidence: EngineeringPlotMessageEvidence): void {
    const theme = chartTheme();
    const shared = {
      animation: false,
      aria: { enabled: true, decal: { show: true } },
      textStyle: { color: theme.foreground },
      legend: { top: 4, textStyle: { color: theme.muted } },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue },
      grid: { left: 58, right: 50, top: 38, bottom: 44 },
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
      xAxis: axis("rad/s", "log", theme), yAxis: axis("Magnitude (dB)", "value", theme),
      series: bodeSeries(evidence, "magnitudeDb"),
    });
    this.phase.setOption({ ...shared,
      xAxis: axis("rad/s", "log", theme), yAxis: axis("Phase (°)", "value", theme),
      series: bodeSeries(evidence, "phaseDegrees"),
    });
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
    this.section.hidden = false;
    this.resize();
  }

  private resize(): void { this.time.resize(); this.magnitude.resize(); this.phase.resize(); }
}

function axis(name: string, type: "value" | "log", theme: ReturnType<typeof chartTheme>) {
  return { type, name, nameLocation: "middle", nameGap: 30, nameTextStyle: { color: theme.muted },
    axisLabel: { color: theme.muted, formatter: formatEngineeringNumber }, axisLine: { lineStyle: { color: theme.border } },
    splitLine: { lineStyle: { color: theme.border, opacity: 0.28 } } };
}

function engineeringLine(name: string, yAxisIndex: number, data: number[][], style: "solid" | "dashed") {
  return { name, type: "line" as const, yAxisIndex, showSymbol: false, lineStyle: { width: 2, type: style }, data };
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
    showSymbol: true,
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

const host = document.querySelector<HTMLElement>("[data-control-studio-chart]");
const studio = document.querySelector<HTMLElement>("[data-control-studio]");
const adapter = host ? new ControlChartAdapter(host) : undefined;
const engineeringTime = document.querySelector<HTMLElement>("[data-engineering-time]");
const engineeringMagnitude = document.querySelector<HTMLElement>("[data-engineering-bode-magnitude]");
const engineeringPhase = document.querySelector<HTMLElement>("[data-engineering-bode-phase]");
const engineering = engineeringTime && engineeringMagnitude && engineeringPhase
  ? new EngineeringChartAdapter(engineeringTime, engineeringMagnitude, engineeringPhase) : undefined;
const receivedSamples: ControlChartSample[] = [];
const receivedMessages = new Map<string, string>();
let firstSimulationTime: bigint | undefined;
let firstLoopTimeMs: number | undefined;
let previousSaturation: ControlSaturationState | undefined;
adapter?.setSamples([]);

window.addEventListener("message", (event) => {
  if (!studio || !adapter) return;
  const raw = event.data as Record<string, unknown> | undefined;
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
    adapter.setSource(String(result?.source ?? "Saved result"));
    adapter.setUnits(String(units?.pv ?? "engineering unit"), String(units?.mv ?? "%"));
    const last = samples[samples.length - 1] as Record<string, unknown> | undefined;
    setText("[data-control-studio-source]", result?.source === "Physical" ? "Physical plant" : "Simulation");
    setText("[data-control-studio-connection]", "Saved result");
    setText("[data-control-studio-freshness]", `Historical · revision ${String(result?.resultRevision ?? "—")}`);
    setText("[data-control-studio-controller-mode]", String(last?.controllerMode ?? "Unavailable"));
    setText("[data-control-studio-sp]", savedReading(last?.sp, units?.sp));
    setText("[data-control-studio-pv]", savedReading(last?.pv, units?.pv));
    setText("[data-control-studio-mv]", savedReading(last?.mv, units?.mv));
    const limits = last?.outputLimits as Record<string, unknown> | undefined;
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
    if (payload?.status === "complete" && payload.plots) engineering?.setEvidence(payload.plots as EngineeringPlotMessageEvidence);
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
    .some((signal) => signal.quality === "forced") ? "bad" : "good";
  receivedSamples.push({
    elapsedSeconds: (snapshot.observedAtMs - firstLoopTimeMs) / 1_000,
    sp: snapshot.setpoint.value,
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
  setText("[data-control-studio-sp]", reading(snapshot.setpoint));
  setText("[data-control-studio-pv]", reading(snapshot.processVariable));
  setText("[data-control-studio-mv]", reading(snapshot.manipulatedVariable));
  const qualityText = quality === "good" ? "Good" : "Forced signal";
  setText("[data-control-studio-quality]",
    `${qualityText} · Output ${formatEngineeringNumber(snapshot.outputLimits.minimum)}…${formatEngineeringNumber(snapshot.outputLimits.maximum)} ${snapshot.manipulatedVariable.unit} · Saturation ${snapshot.saturation}`);
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
  return signal.unit ? `${value} ${signal.unit}` : value;
}

function savedReading(value: unknown, unit: unknown): string {
  const formatted = formatEngineeringNumber(value);
  return formatted === "—" ? formatted : `${formatted}${String(unit ?? "").trim() ? ` ${String(unit)}` : ""}`;
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
