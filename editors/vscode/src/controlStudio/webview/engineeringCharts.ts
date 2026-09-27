import type { DeclaredRootLocusResult } from "../declaredAnalysis";
import { IdentificationOutputSelection } from "./identificationOutputSelection";
import { declaredRootPlotOptions } from "./declaredRootPlot";
/// <reference lib="dom" />

import * as echarts from "echarts/core";

import {
displayEngineeringUnit,
formatEngineeringNumber,
identificationOutputAxisRange,
type MimoPlotEvidence
} from "../chartContract";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import type { IdentifiedModelPresentation } from "../identification";
import {
magnitudeCrossoverMarkers,
magnitudeReferenceLines,
nicholsCrossoverMarkers,
nicholsReferenceLines,
phaseCrossoverMarkers,
phaseReferenceLines,
type MarginMarkerInput,
} from "./marginMarkers";
import { emptyAxesOption,plotStateMessage,plotStateOverlay } from "./plotStates";
import { uncertaintyBandDescription,uncertaintyBandSeries } from "./uncertaintyBand";

import { axis,bodeSeries,chartTheme,copyValue,engineeringLine,formatChartValue,initEngineeringChart,marginMarkOptions,pointSeries,requiredElement } from "./chartPrimitives";
import { registerActiveChartActionTarget, type ActiveChartAction } from "./activeChartActionRouter";
import { executeChartToolbarAction } from "./chartToolbarActions";
export class EngineeringChartAdapter {
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
  // The candidate's last qualified plot evidence, kept so a plot that was hidden (and so
  // never reliably laid out) at the time setEvidence ran can be fully re-rendered — not
  // merely resized — once it actually becomes visible (finding V-03).
  private lastEvidence: EngineeringPlotMessageEvidence | undefined;
  private readonly toolbarWindows = new Map<string, [number, number]>();

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
    registerActiveChartActionTarget({
      plotIds: ["engineering-response", "bode", "root-locus", "nyquist", "nichols", "sensitivity", "requirements"],
      handle: (action, plotId) => this.handleToolbarAction(action, plotId),
    });
    for (const host of [timeHost, magnitudeHost, phaseHost,
      requiredElement<HTMLElement>("[data-engineering-root-locus]"),
      requiredElement<HTMLElement>("[data-engineering-nyquist]"),
      requiredElement<HTMLElement>("[data-engineering-nichols]"),
      requiredElement<HTMLElement>("[data-engineering-sensitivity]"),
      requiredElement<HTMLElement>("[data-engineering-requirements]")]) {
      new ResizeObserver(() => this.resize()).observe(host);
    }
    this.applyBaselineAxes();
    window.addEventListener("trust-control-studio-plot-shown", () => {
      window.requestAnimationFrame(() => {
        // A chart that was initialized (or last rendered) while its host was hidden or
        // mid-transition cannot be trusted to redraw correctly from a bare resize() —
        // replaying the full option from the retained evidence is what MimoChartAdapter
        // already relies on, and it has no matching blank-plot finding.
        if (this.lastEvidence) this.setEvidence(this.lastEvidence);
        else this.resize();
      });
    });
  }

  private applyBaselineAxes(): void {
    const theme = chartTheme();
    for (const [chart, xName, yName] of [
      [this.time, "Time (s)", "Response"],
      [this.magnitude, "Frequency (rad/s)", "Magnitude (dB)"],
      [this.phase, "Frequency (rad/s)", "Phase (°)"],
      [this.rootLocus, "Real axis", "Imaginary axis"],
      [this.nyquist, "Real", "Imaginary"],
      [this.nichols, "Open-loop phase (°)", "Open-loop magnitude (dB)"],
      [this.sensitivity, "rad/s", "Magnitude (dB)"],
      [this.requirements, "Time (s)", "Normalized response"],
    ] as const) {
      chart.setOption(emptyAxesOption(xName, yName, theme));
    }
  }

  /** Shows exactly one explanatory sentence on every engineering plot without discarding
   * whatever axes or series they already have (Specification 35 S5.7). */
  showFailure(reason: string): void {
    const theme = chartTheme();
    const message = plotStateMessage(reason, "The candidate engineering analysis failed.");
    for (const chart of [this.time, this.magnitude, this.phase, this.rootLocus,
      this.nyquist, this.nichols, this.sensitivity, this.requirements]) {
      chart.setOption({ graphic: [plotStateOverlay("engineering-failure", message, theme)] });
    }
    this.summary.textContent = message;
    this.resize();
  }

  /** A different saved revision cannot inherit cached predictions or empty bare axes. */
  resetEvidence(reason: string): void {
    this.lastEvidence = undefined;
    this.toolbarWindows.clear();
    this.table.replaceChildren();
    for (const chart of [this.time, this.magnitude, this.phase, this.rootLocus,
      this.nyquist, this.nichols, this.sensitivity, this.requirements]) chart.clear();
    this.applyBaselineAxes();
    this.showFailure(reason);
  }

  setDeclaredRootLocus(result: DeclaredRootLocusResult): void {
    const theme = chartTheme();
    const poles = result.branches.flatMap(branch => branch.poles);
    this.rootLocus.setOption(declaredRootPlotOptions(result, theme,
      axis("Real axis", "value", theme, poles.map(pole => pole.real)),
      axis("Imaginary axis", "value", theme, poles.map(pole => pole.imaginary))), { notMerge: true });
    requiredElement<HTMLElement>("[data-engineering-root-locus]").setAttribute("aria-description",
      `${result.parameter} sweep. ${result.provenance}. ${result.approximation}`);
    this.resize();
  }

  setEvidence(evidence: EngineeringPlotMessageEvidence): void {
    this.lastEvidence = evidence;
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
      graphic: [plotStateOverlay("engineering-failure", undefined, theme)],
    };
    const marginInput: MarginMarkerInput = {
      gainMarginDb: evidence.gainMarginDb === "unbounded" ? Number.POSITIVE_INFINITY : evidence.gainMarginDb,
      phaseMarginDegrees: evidence.phaseMarginDegrees,
      gainCrossover: evidence.gainCrossover,
      phaseCrossover: evidence.phaseCrossover,
    };
    const studio = document.querySelector<HTMLElement>("[data-control-studio]");
    const processUnit = displayEngineeringUnit(studio?.dataset.processUnit ?? "");
    const outputUnit = displayEngineeringUnit(studio?.dataset.outputUnit ?? "");
    const hasCurrent = evidence.timeResponse.some(point => point.currentPv !== undefined);
    this.time.setOption({ ...shared,
      color: hasCurrent ? [theme.pv, theme.pv, theme.mv, theme.mv] : [theme.pv, theme.mv],
      xAxis: axis("Time (s)", "value", theme, evidence.timeResponse.map((point) => point.timeSeconds)),
      yAxis: [
        axis(processUnit ? `PV (${processUnit})` : "PV", "value", theme, evidence.timeResponse.flatMap((point) => [point.currentPv, point.candidatePv]), { fitToData: true }),
        axis(outputUnit ? `MV (${outputUnit})` : "MV", "value", theme, evidence.timeResponse.flatMap((point) => [point.currentMv, point.candidateMv]), { fitToData: true }),
      ],
      series: [
        ...(hasCurrent ? [engineeringLine("Current PV", 0, evidence.timeResponse.map((point) => [point.timeSeconds, point.currentPv ?? null]), "solid")] : []),
        engineeringLine("Candidate PV", 0, evidence.timeResponse.map((point) => [point.timeSeconds, point.candidatePv]), "dashed"),
        ...(hasCurrent ? [engineeringLine("Current MV", 1, evidence.timeResponse.map((point) => [point.timeSeconds, point.currentMv ?? null]), "solid")] : []),
        engineeringLine("Candidate MV", 1, evidence.timeResponse.map((point) => [point.timeSeconds, point.candidateMv]), "dashed"),
      ],
    }, { replaceMerge: ["series"] });
    const magnitudeValues = [...evidence.plantBode, ...evidence.candidateOpenLoopBode].map((point) => point.magnitudeDb);
    this.magnitude.setOption({ ...shared,
      xAxis: { ...axis("", "log", theme), axisLabel: { show: false } },
      // A fixed 20 dB step (instead of an auto-picked count) keeps the margin grid
      // legible and comparable across candidates (finding V-05).
      yAxis: { ...axis("Magnitude (dB)", "value", theme, magnitudeValues), interval: 20 },
      series: bodeSeries(evidence, "magnitudeDb",
        marginMarkOptions(magnitudeReferenceLines(marginInput), magnitudeCrossoverMarkers(marginInput), theme)),
    });
    const phaseValues = [...evidence.plantBode, ...evidence.candidateOpenLoopBode].map((point) => point.phaseDegrees);
    this.phase.setOption({ ...shared,
      legend: { show: false },
      xAxis: axis("Frequency (rad/s)", "log", theme),
      yAxis: { ...axis("Phase (°)", "value", theme, phaseValues), interval: 45 },
      series: bodeSeries(evidence, "phaseDegrees",
        marginMarkOptions(phaseReferenceLines(marginInput), phaseCrossoverMarkers(marginInput), theme)),
    });
    requiredElement<HTMLOutputElement>("[data-design-gain-margin]").value =
      `${formatEngineeringNumber(evidence.gainMarginDb)} dB`;
    requiredElement<HTMLOutputElement>("[data-design-phase-margin]").value =
      `${formatEngineeringNumber(evidence.phaseMarginDegrees)}°`;
    this.renderRootLocus(evidence, shared, theme);
    this.nyquist.setOption({ ...shared,
      xAxis: axis("Real", "value", theme, [...evidence.nyquist.map((point) => point.real), -1]),
      yAxis: axis("Imaginary", "value", theme, [...evidence.nyquist.map((point) => point.imaginary), 0]),
      series: [
        engineeringLine("Open-loop contour", 0, evidence.nyquist.map((point) => [point.real, point.imaginary]), "solid"),
        { name: "Critical point −1 + 0j", type: "line", showSymbol: true, symbol: "diamond",
          symbolSize: 9, lineStyle: { width: 0 }, data: [[-1, 0]] },
      ],
    });
    this.nichols.setOption({ ...shared,
      xAxis: axis("Open-loop phase (°)", "value", theme, evidence.nichols.map((point) => point.phaseDegrees)),
      yAxis: axis("Open-loop magnitude (dB)", "value", theme, evidence.nichols.map((point) => point.magnitudeDb)),
      series: [{ ...engineeringLine("Candidate", 0,
        evidence.nichols.map((point) => [point.phaseDegrees, point.magnitudeDb]), "solid"),
        ...marginMarkOptions(nicholsReferenceLines(), nicholsCrossoverMarkers(marginInput), theme) }],
    });
    const sensitivityValues = [...evidence.sensitivity.map((point) => point.sensitivityMagnitudeDb),
      ...evidence.sensitivity.map((point) => point.complementarySensitivityMagnitudeDb),
      ...evidence.sensitivity.map((point) => point.controlSensitivityMagnitudeDb)];
    this.sensitivity.setOption({ ...shared,
      xAxis: axis("rad/s", "log", theme), yAxis: axis("Magnitude (dB)", "value", theme, sensitivityValues),
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
      + `gain margin ${formatEngineeringNumber(evidence.gainMarginDb)} dB; phase margin ${formatEngineeringNumber(evidence.phaseMarginDegrees)} degrees.`;
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
    const poles = evidence.rootLocus.branches.flatMap((branch) => branch.poles);
    this.rootLocus.setOption({ ...shared,
      xAxis: axis("Real axis", "value", theme, poles.map((pole) => pole.real)),
      yAxis: axis("Imaginary axis", "value", theme, poles.map((pole) => pole.imaginary)),
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
    this.requirements.setOption({ ...shared,
      xAxis: axis("Time (s)", "value", theme, evidence.timeResponse.map((point) => point.timeSeconds)),
      yAxis: axis("Normalized response", "value", theme, [
        ...evidence.timeResponse.map((point) => point.candidatePv),
        ...uncertainty.flatMap((point) => [point.lower, point.upper]),
      ]),
      series: [
        engineeringLine("Candidate", 0,
          evidence.timeResponse.map((point) => [point.timeSeconds, point.candidatePv]), "solid"),
        ...uncertaintyBandSeries(evidence.uncertainty.method, uncertainty, theme.foreground),
      ],
      // Keeps the engineering-failure overlay's hidden state explicit here too: an
      // omitted graphic id is left exactly as ECharts last rendered it, and a stale
      // failure message must not persist over a plot that just received good evidence.
      graphic: [
        plotStateOverlay("engineering-failure", undefined, theme),
        plotStateOverlay("requirements-uncertainty-state",
          uncertainty.length > 0 ? undefined : "Uncertainty: Not evaluated.", theme),
      ],
    });
    const summary = evidence.requirements.map((item) =>
      `${item.name}: ${item.status} (${formatEngineeringNumber(item.value)} ${item.unit}; ${item.direction} ${formatEngineeringNumber(item.limit)} ${item.unit})`).join(". ");
    requiredElement<HTMLElement>("[data-engineering-requirements]").setAttribute("aria-description",
      `${summary} ${uncertaintyBandDescription(evidence.uncertainty.method, uncertainty.length)}`);
  }

  private resize(): void {
    for (const chart of [this.time, this.magnitude, this.phase, this.rootLocus,
      this.nyquist, this.nichols, this.sensitivity, this.requirements]) chart.resize();
  }

  private handleToolbarAction(action: ActiveChartAction, plotId: string): boolean {
    const charts = plotId === "bode" ? [this.magnitude, this.phase] : [
      plotId === "engineering-response" ? this.time : plotId === "root-locus" ? this.rootLocus :
      plotId === "nyquist" ? this.nyquist : plotId === "nichols" ? this.nichols :
      plotId === "sensitivity" ? this.sensitivity : this.requirements,
    ];
    const host = document.querySelector<HTMLElement>(`[data-plot-view="${plotId}"] .analysis-plot, [data-plot-view="${plotId}"] [data-engineering-time]`);
    return executeChartToolbarAction(action, plotId, charts, host, this.toolbarWindows);
  }
}

export class MimoChartAdapter {
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
        const cell = document.createElement("td"); cell.textContent = formatEngineeringNumber(value); row.append(cell);
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

export class IdentificationChartAdapter {
  private readonly outputSelection: IdentificationOutputSelection;
  private readonly fit: echarts.ECharts;
  private readonly residuals: echarts.ECharts;
  private readonly validation: echarts.ECharts;
  private readonly polesZeros: echarts.ECharts;
  private readonly summary: HTMLElement;
  private readonly toolbarWindows = new Map<string, [number, number]>();
  // Retained so a plot switched to while hidden — the same class of bug reported for
  // Nichols in V-03 — can be fully re-rendered once it actually becomes visible.
  private lastEvidence: IdentifiedModelPresentation | undefined;

  constructor(host: HTMLElement) {
    this.outputSelection = new IdentificationOutputSelection(() => { if (this.lastEvidence) this.setEvidence(this.lastEvidence); });
    this.summary = requiredElement<HTMLElement>("[data-identification-fit-summary]");
    this.fit = echarts.init(host, undefined, { renderer: "canvas" });
    this.residuals = initEngineeringChart("[data-identification-plot=\"residuals\"]");
    this.validation = initEngineeringChart("[data-identification-plot=\"validation-data\"]");
    this.polesZeros = initEngineeringChart("[data-identification-plot=\"poles-zeros\"]");
    for (const chartHost of document.querySelectorAll<HTMLElement>("[data-identification-plot]")) {
      new ResizeObserver(() => this.resize()).observe(chartHost);
    }
    const theme = chartTheme();
    this.fit.setOption(emptyAxesOption("Time (s)", "PV", theme));
    this.residuals.setOption(emptyAxesOption("Time (s)", "Residual", theme));
    this.validation.setOption(emptyAxesOption("Time (s)", "PV", theme));
    this.polesZeros.setOption(emptyAxesOption("Real axis (1/s)", "Imaginary axis (1/s)", theme));
    registerActiveChartActionTarget({
      plotIds: ["fit", "residuals", "validation-data", "poles-zeros"],
      handle: (action, plotId) => executeChartToolbarAction(action, plotId, this.identificationCharts(plotId),
        document.querySelector<HTMLElement>(`[data-identification-plot="${plotId}"]`), this.toolbarWindows),
    });
    window.addEventListener("trust-control-studio-plot-shown", () => {
      window.requestAnimationFrame(() => {
        if (this.lastEvidence) this.setEvidence(this.lastEvidence);
        else this.resize();
      });
    });
  }

  /** Shows exactly one explanatory sentence on every identification plot without
   * discarding whatever axes or series they already have (Specification 35 S5.7). */
  showFailure(reason: string): void {
    const theme = chartTheme();
    const message = plotStateMessage(reason, "Identification failed.");
    for (const chart of [this.fit, this.residuals, this.validation, this.polesZeros]) {
      chart.setOption({ graphic: [plotStateOverlay("identification-failure", message, theme)] });
    }
    this.summary.textContent = message;
    this.resize();
  }

  setEvidence(evidence: IdentifiedModelPresentation): void {
    this.lastEvidence = evidence;
    evidence = this.outputSelection.select(evidence);
    const theme = chartTheme();
    const comparison = evidence.modelStructure;
    const comparisonPoints = comparison ? [...comparison.candidateA.fit.points, ...comparison.candidateB.fit.points] : [];
    const outputRange = identificationOutputAxisRange((comparison ? comparisonPoints : evidence.predictedResponse).flatMap((point) =>
      [point.measured, point.predicted]));
    const timeValues = (comparison ? comparison.candidateA.fit.points : evidence.predictedResponse).map((point) => point.timeSeconds);
    const shared = {
      animation: false,
      aria: { enabled: true, decal: { show: true } },
      textStyle: { color: theme.foreground },
      legend: { top: 4, textStyle: { color: theme.muted } },
      tooltip: { trigger: "axis", confine: true, valueFormatter: formatChartValue },
      dataZoom: [{ type: "inside", filterMode: "none" }],
      grid: { left: 58, right: 58, top: 38, bottom: 44 },
      graphic: [plotStateOverlay("identification-failure", undefined, theme)],
    };
    const fitSeries = comparison ? [
      engineeringLine("Measured PV", 0,
        comparison.candidateA.fit.points.map((point) => [point.timeSeconds, point.measured]), "solid"),
      engineeringLine("Candidate A", 0,
        comparison.candidateA.fit.points.map((point) => [point.timeSeconds, point.predicted]), "dashed"),
      engineeringLine("Candidate B", 0,
        comparison.candidateB.fit.points.map((point) => [point.timeSeconds, point.predicted]), "solid"),
    ] : [
      engineeringLine("Measured PV", 0,
        evidence.predictedResponse.map((point) => [point.timeSeconds, point.measured]), "solid"),
      engineeringLine("Predicted PV", 0,
        evidence.predictedResponse.map((point) => [point.timeSeconds, point.predicted]), "dashed"),
    ];
    this.fit.setOption({ ...shared,
      color: comparison ? [theme.pv, theme.auxiliaryOrange, theme.auxiliaryPurple] : [theme.pv, theme.sp],
      xAxis: axis("Time (s)", "value", theme, timeValues),
      yAxis: { ...axis(`PV (${displayEngineeringUnit(evidence.units.output)})`, "value", theme, [outputRange.minimum, outputRange.maximum]),
        min: outputRange.minimum, max: outputRange.maximum },
      series: comparison ? fitSeries : [
        fitSeries[0],
        { ...fitSeries[1],
          markLine: { silent: true, symbol: "none", lineStyle: { color: theme.muted, width: 1, type: "dotted" },
            label: { formatter: "{b}", color: theme.muted, position: "insideEndTop" },
            data: evidence.stepResponse ? [
              { name: "Baseline", yAxis: evidence.stepResponse.baseline.mean },
              { name: "Final estimate", yAxis: evidence.stepResponse.final.mean },
              { name: "Onset threshold", yAxis: evidence.stepResponse.onsetThreshold },
              { name: "63.2% level", yAxis: evidence.stepResponse.temperature63 },
              ...(evidence.stepResponse.detectableOnsetSeconds === undefined ? [] : [{ name: "Detectable onset",
                xAxis: evidence.stepResponse.stepTimeSeconds + evidence.stepResponse.detectableOnsetSeconds }]),
            ] : [] } },
      ],
    }, { replaceMerge: ["series"] });
    const residualSeries = comparison ? [
      { ...engineeringLine("Candidate A residual", 0,
        comparison.candidateA.fit.points.map((point) => [point.timeSeconds, point.residual]), "dashed"),
        markLine: { silent: true, symbol: "none", lineStyle: { color: theme.muted, width: 1, type: "dotted" },
          label: { formatter: "{b}", color: theme.muted, position: "insideEndTop" },
          data: [
            { name: "Residual limit", yAxis: comparison.requirements.maximumAbsoluteResidual },
            { name: "Residual limit", yAxis: -comparison.requirements.maximumAbsoluteResidual },
          ] },
      },
      engineeringLine("Candidate B residual", 0,
        comparison.candidateB.fit.points.map((point) => [point.timeSeconds, point.residual]), "solid"),
    ] : [engineeringLine("Prediction residual", 0,
      evidence.predictedResponse.map((point) => [point.timeSeconds, point.residual]), "solid")];
    const residualAxisValues = comparison ? [
      ...comparison.candidateA.fit.points.map((point) => point.residual),
      ...comparison.candidateB.fit.points.map((point) => point.residual),
      comparison.requirements.maximumAbsoluteResidual,
      -comparison.requirements.maximumAbsoluteResidual,
    ] : evidence.predictedResponse.map((point) => point.residual);
    const residualAxis = axis(`Residual (${displayEngineeringUnit(evidence.units.output)})`, "value", theme, residualAxisValues);
    if (comparison) {
      residualAxis.min = Math.min(...residualAxisValues);
      residualAxis.max = Math.max(...residualAxisValues);
    }
    this.residuals.setOption({ ...shared,
      color: comparison ? [theme.auxiliaryOrange, theme.auxiliaryPurple] : [theme.pv],
      xAxis: axis("Time (s)", "value", theme, timeValues),
      yAxis: residualAxis,
      series: residualSeries,
    }, { replaceMerge: ["series"] });
    this.validation.setOption({ ...shared,
      color: [theme.pv, theme.sp],
      xAxis: axis("Time (s)", "value", theme, timeValues),
      yAxis: { ...axis(`PV (${displayEngineeringUnit(evidence.units.output)})`, "value", theme, [outputRange.minimum, outputRange.maximum]),
        min: outputRange.minimum, max: outputRange.maximum },
      series: [
        engineeringLine("Validation measurement", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.measured]), "solid"),
        engineeringLine("Model prediction", 0,
          evidence.predictedResponse.map((point) => [point.timeSeconds, point.predicted]), "dashed"),
      ],
    });
    const plane = evidence.poleZeroPlane;
    const poles = plane ? plane.poles ?? [] : evidence.poles.map(real => ({ real, imaginary: 0 }));
    const zeros = plane ? plane.zeros ?? [] : evidence.zeros.map(real => ({ real, imaginary: 0 }));
    const coordinate = plane?.domain === "z" ? "z" : "1/s";
    this.polesZeros.setOption({ ...shared,
      title: plane ? { subtext: `${plane.scope}${plane.zeros === undefined ? " Zeros not evaluated." : ""}`, left: 58,
        subtextStyle: { color: theme.muted, fontSize: 10 } } : {},
      tooltip: { trigger: "item", confine: true, valueFormatter: formatChartValue },
      xAxis: axis(`Real axis (${coordinate})`, "value", theme, [...poles, ...zeros].map(value => value.real)),
      yAxis: axis(`Imaginary axis (${coordinate})`, "value", theme, [...poles, ...zeros].map(value => value.imaginary)),
      series: [
        pointSeries("Poles", poles.map(value => [value.real, value.imaginary]), "cross"),
        pointSeries("Zeros", zeros.map(value => [value.real, value.imaginary]), "circle"),
      ],
    });
    this.summary.textContent = `${evidence.predictedResponse.length} measured and predicted points; ` +
      `estimation residual lag-one correlation ${formatEngineeringNumber(evidence.estimation.residualLagOneCorrelation)}; ` +
      `validation residual lag-one correlation ${formatEngineeringNumber(evidence.validation.residualLagOneCorrelation)}.`;
    this.resize();
  }

  private resize(): void {
    for (const chart of [this.fit, this.residuals, this.validation, this.polesZeros]) chart.resize();
  }

  private identificationCharts(plotId: string): readonly echarts.ECharts[] {
    return [plotId === "fit" ? this.fit : plotId === "residuals" ? this.residuals : plotId === "validation-data" ? this.validation : this.polesZeros];
  }
}
