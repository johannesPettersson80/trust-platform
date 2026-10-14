import * as echarts from "echarts/core";
import { LineChart, ScatterChart } from "echarts/charts";
import { AriaComponent, GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { Failure } from "../generated/Failure";
import type { AnalysisEnvelope, AnalysisInputs, ToolOffer } from "../generated/AnalyzeAnswer";
import type { AnalysisTool } from "../generated/AnalyzeRequest";
import type { AnalysisPage } from "../generated/PageRequest";
import { nyquistGeometry, nyquistSeries } from "./nyquistChart";
import { poleMapGeometry, type PlaneArea, type PlaneGeometry } from "./complexPlaneGeometry";
import { axis, chartTheme, engineeringLine, formatChartValue, formatPhaseChartValue, formatPhaseDegrees, phaseAxis } from "./chartPrimitives";
import { rootLocusChartSeries, rootLocusTooltip } from "./rootLocusChart";
import { sensitivitySeries } from "./sensitivityChart";
import { initialStateResponseRows, initialStateResponseSeries } from "./initialStateResponseChart";
import { formatMatrixRows, formatNumberList } from "./numberListText";
import { formatEngineeringNumber } from "../chartContract";
import { formatDurationSeconds, parseDurationSeconds } from "./durationField";
import {
  ANALYSIS_KIND, ENGINE_MODEL_CHANGED_EVENT, answerOf, type AnalysisUpdate, type EngineAnalysisEvents, type FeedbackAnswer, type FrequencyAnswer,
  type InitialStateAnswer, type LeadLagAnswer, type MeasuredFrequencyAnswer, type PoleZeroAnswer, type RelativeGainAnswer, type RootLocusAnswer,
  type RouthAnswer, type StateStructureAnswer,
} from "./engineAnalysis";

echarts.use([LineChart, ScatterChart, GridComponent, LegendComponent, TooltipComponent, AriaComponent, CanvasRenderer]);

/** The tools whose result is drawn in the plot area (B9.9). */
const PLOTTED_TOOLS = new Set<AnalysisTool>(["rootLocus", "frequency", "measuredFrequency", "poleZero", "feedback", "initialState"]);


/** The Tune page's analysis presenter (specification 35, B9.9). */
export interface ThreePageAnalysisPresenter extends EngineAnalysisEvents {
  /** Takes a stage of the page: a model change clears the shown answers and asks the engine for the tools that apply now. */
  receive(stage: string | undefined, payload: Record<string, unknown>): void;
  /** The study kept a recording: the tools that apply may now differ, so the engine is asked for them again (B9.9). */
  recordingsChanged(): void;
  /** Shows the tool the Analyze… list chose, or closes the analysis (undefined). A tool the engine does not offer is not shown. */
  setTool(tool: string | undefined): void;
  /** Calculates the shown tool, or aborts its running operation. */
  run(): void;
  actionLabel(): string | undefined;
  choice(): string | undefined;
  availableTools(): readonly string[];
  /** The tool the page shows with the inputs it holds for it; none when no tool is shown (kept with a parked loop, B9.9). */
  shown(): AnalysisPage | undefined;
  /** The tools the engine offers for the loop, with the labels and actions it gives them. */
  offers(): readonly ToolOffer[];
  dispose(): void;
}

/**
 * Presents the engine's analysis on the Tune page (specification 35, B9.9). The engine answers which tools apply, their labels,
 * default inputs and limits, and every result and sentence. This draws them, shows the editor's fields with the inputs the engine
 * read, and sends what the engineer entered: an entry that is blank or not a number is sent absent, so the engine's default
 * applies. It checks no input and computes no value.
 */
export function initializeThreePageAnalysis(
  root: HTMLElement,
  post: (kind: string, payload?: Record<string, unknown>) => void = () => undefined,
  changed: () => void = () => undefined,
  shownChanged: () => void = () => undefined,
): ThreePageAnalysisPresenter {
  const disclosure = root.querySelector<HTMLElement>("[data-three-page-analysis]");
  const result = root.querySelector<HTMLElement>("[data-three-page-analysis-result]");
  const plotHost = root.querySelector<HTMLElement>("[data-three-page-analysis-plot]");
  const summary = root.querySelector<HTMLElement>("[data-three-page-analysis-summary]");
  const tableDetails = root.querySelector<HTMLElement>("[data-three-page-analysis-table-details]");
  const detailsSummary = root.querySelector<HTMLElement>("[data-three-page-analysis-details-summary]");
  const table = root.querySelector<HTMLElement>("[data-three-page-analysis-table]");
  if (!disclosure || !result || !plotHost || !summary || !tableDetails || !detailsSummary || !table) return emptyPresenter();
  const plotArea: HTMLElement = plotHost;

  const statusLine = document.createElement("p"); statusLine.setAttribute("role", "status"); statusLine.className = "studio-action-message";
  const editor = document.createElement("section"); editor.className = "studio-fields"; editor.hidden = true;
  disclosure.insertBefore(statusLine, result); disclosure.insertBefore(editor, result);
  const chart = echarts.init(plotHost, undefined, { renderer: "canvas" });
  let redrawPlot: (() => void) | undefined;
  const resize = () => { chart.resize(); redrawPlot?.(); };
  const resizeObserver = new ResizeObserver(resize); resizeObserver.observe(plotHost);
  window.addEventListener("resize", resize);

  /** The tools the engine offers for the loop. */
  let offerList: readonly ToolOffer[] = [];
  let selected: AnalysisTool | undefined;
  /** The inputs the editor shows for each tool: the engine's defaults, or the inputs the engine last read (B9.9). */
  const inputsByTool = new Map<AnalysisTool, AnalysisInputs>();
  /** The last answer of each tool with a result; the result area shows the selected tool's. */
  const answers = new Map<AnalysisTool, AnalysisEnvelope>();
  /** The calculation the engine runs for the page: its tool when known, and its operation once the engine started one. */
  let pending: { tool?: AnalysisTool; operationId?: string; progress?: string } | undefined;
  let refusal: Failure | undefined;
  let notice: string | undefined;
  let channelIndex = 0;
  let feedbackView: "bode" | "nyquist" | "sensitivity" = "bode";
  let responseOutputIndex = 0;
  let disposed = false;

  const offerOf = (tool: string | undefined): ToolOffer | undefined => offerList.find(offer => offer.tool === tool);
  const inputsOf = (tool: AnalysisTool): AnalysisInputs => inputsByTool.get(tool) ?? offerOf(tool)?.defaults ?? {};
  /** The last answer of `tool` the page holds, from its last calculation (the result is read by the tool's name). */
  const answerFor = <K extends AnalysisTool>(tool: K) => answerOf(answers.get(tool)?.result ?? undefined, tool);

  const actionLabel = (): string | undefined => {
    const offer = offerOf(selected);
    if (!offer) return undefined;
    return pending && offer.operation ? offer.abortAction ?? "Abort analysis" : offer.action;
  };
  const choice = (): string | undefined => selected ? `analysis:${selected}` : undefined;
  const availableTools = (): readonly string[] => offerList.map(offer => offer.tool);
  const shown = (): AnalysisPage | undefined => selected ? { tool: selected, inputs: inputsOf(selected) } : undefined;
  /** The shown analysis the owner was last told of, as text: the owner is told only when it changes (a parked loop keeps it, §0.1a). */
  let reportedShown = JSON.stringify(null);
  const reportShown = (): void => {
    const key = JSON.stringify(shown() ?? null);
    if (key === reportedShown) return;
    reportedShown = key;
    shownChanged();
  };

  const run = (): void => {
    const offer = offerOf(selected);
    if (!selected || !offer) return;
    // The Tune button aborts a running operation as it starts one (B9.9): Abort sends the engine's cancel.
    if (pending) {
      if (pending.operationId && offer.operation) post(ANALYSIS_KIND.cancel);
      return;
    }
    calculate(selected, readInputs(selected));
  };

  /** Asks the engine to calculate the tool with these inputs: the editor shows them until the engine reads them back. */
  const calculate = (tool: AnalysisTool, inputs: AnalysisInputs): void => {
    inputsByTool.set(tool, inputs);
    refusal = undefined; notice = undefined;
    pending = { tool };
    renderResult();
    post(ANALYSIS_KIND.run, { tool, inputs });
  };

  /**
   * The study's model changed (B9.9): the shown answers belong to the model before it, and the tools that apply to the model now are
   * asked again, so the Analyze… list follows the model.
   */
  const modelChangedNow = (): void => {
    answers.clear(); refusal = undefined; notice = undefined;
    post(ANALYSIS_KIND.tools);
    render();
  };

  /** An engine model change reaches the page as this window event, when it shows or clears an identification (engineIdentify.ts). */
  const onModelChanged = (): void => { if (!disposed) modelChangedNow(); };
  window.addEventListener(ENGINE_MODEL_CHANGED_EVENT, onModelChanged);

  const setTool = (tool: string | undefined): void => {
    const offer = offerOf(tool);
    selected = offer?.tool;
    refusal = undefined; notice = undefined;
    // The page records the tool it shows with its inputs, so that reopening shows them (B9.9); closing records none.
    post(ANALYSIS_KIND.show, selected ? { tool: selected, inputs: inputsOf(selected) } : {});
    render();
  };

  /** Keeps the engine's answer of a calculation: the result is kept, and the inputs the engine read fill the editor. */
  const receiveAnswer = (envelope: AnalysisEnvelope): void => {
    if (pending && (pending.tool === undefined || pending.tool === envelope.tool)) pending = undefined;
    inputsByTool.set(envelope.tool, envelope.inputs);
    if (envelope.result) answers.set(envelope.tool, envelope);
    if (selected === envelope.tool) {
      refusal = envelope.failure ?? undefined; notice = undefined;
      renderEditor();
      renderResult();
    }
  };

  const presenter: ThreePageAnalysisPresenter = {
    receive(stage, payload) {
      if (disposed) return;
      // The TypeScript session's stages that change the model (a parked loop); the engine's changes arrive as the model-changed event.
      if (modelChanged(stage, payload.status)) modelChangedNow();
    },
    recordingsChanged() {
      // A recording the study keeps can change which tools apply (`measuredFrequency` needs one), so the tools are asked again.
      if (!disposed) post(ANALYSIS_KIND.tools);
    },
    setTool,
    run,
    actionLabel,
    choice,
    availableTools,
    shown,
    offers: () => offerList,
    dispose(): void {
      disposed = true;
      window.removeEventListener(ENGINE_MODEL_CHANGED_EVENT, onModelChanged);
      resizeObserver.disconnect();
      window.removeEventListener("resize", resize);
      chart.dispose();
    },
    tools(offers, failure) {
      if (disposed) return;
      offerList = offers ?? [];
      if (failure && selected) refusal = failure;
      if (selected && !offerOf(selected)) setTool(undefined);
      changed();
      render();
    },
    started(tool, operationId) {
      if (disposed) return;
      pending = { tool, operationId };
      renderResult();
    },
    answer(envelope) {
      if (!disposed) receiveAnswer(envelope);
    },
    refused(tool, failure) {
      if (disposed) return;
      if (tool && pending && (pending.tool === undefined || pending.tool === tool)) pending = undefined;
      if (tool === undefined || tool === selected) refusal = failure;
      renderResult();
    },
    update(update: AnalysisUpdate) {
      if (disposed) return;
      if (update.state === "running") {
        pending = { ...pending, operationId: update.operationId, ...(update.progress ? { progress: update.progress } : {}) };
        renderResult();
        return;
      }
      pending = undefined;
      if (update.state === "finished" && update.envelope) receiveAnswer(update.envelope);
      else if (update.state === "failed") refusal = update.failure;
      else if (update.state === "cancelled") { refusal = undefined; notice = "Analysis cancelled."; }
      renderResult();
    },
    restore(tool, inputs) {
      if (disposed || !offerOf(tool)) return;
      inputsByTool.set(tool, inputs);
      selected = tool; refusal = undefined; notice = undefined;
      render();
      // The analysis the page showed is calculated again when the panel reopens (A8.6, B9.9), with the inputs the engine answered
      // for it, as they were recorded: the editor is not read back, so no entry the engine kept is lost or changed.
      if (!pending) calculate(tool, inputs);
    },
  };

  const render = (): void => { renderEditor(); renderResult(); };

  const renderEditor = (): void => {
    editor.replaceChildren();
    editor.hidden = true;
    if (!selected || !offerOf(selected)) return;
    buildEditor(selected);
    editor.hidden = editor.childElementCount === 0;
  };

  const field = (labelText: string, control: HTMLElement): void => {
    const label = document.createElement("label"); label.className = "studio-field"; label.textContent = labelText;
    label.append(control); editor.append(label);
  };
  const numberField = (labelText: string, value: number | undefined, name: string): void => {
    const input = document.createElement("input"); input.type = "number"; input.step = "any";
    input.value = value === undefined ? "" : String(value);
    input.setAttribute(`data-analysis-${name}`, "");
    field(labelText, input);
  };
  const textField = (labelText: string, value: string, name: string, readOnly = false): void => {
    const input = document.createElement("input"); input.type = "text"; input.value = value; input.readOnly = readOnly;
    input.setAttribute(`data-analysis-${name}`, "");
    field(labelText, input);
  };
  const selectField = (labelText: string, choices: readonly (readonly [string, string])[], value: string | undefined, name: string): HTMLSelectElement => {
    const select = document.createElement("select"); select.setAttribute(`data-analysis-${name}`, "");
    for (const [optionValue, text] of choices) select.append(new Option(text, optionValue));
    if (value !== undefined) select.value = value;
    field(labelText, select);
    return select;
  };

  /** The editor of the selected tool, filled with the inputs the engine gave: its defaults, or the inputs it last read. */
  const buildEditor = (tool: AnalysisTool): void => {
    const inputs = inputsOf(tool);
    switch (tool) {
      case "rootLocus":
      case "routh":
        numberField(parameterLabel(tool), inputs.selectedValue ?? undefined, "parameter-value");
        return;
      case "frequency":
      case "feedback": {
        numberField("Minimum frequency (rad/s)", inputs.grid?.minimumFrequency, "minimum-frequency");
        numberField("Maximum frequency (rad/s)", inputs.grid?.maximumFrequency, "maximum-frequency");
        numberField("Frequency points", inputs.grid?.points, "frequency-points");
        const channels = answerFor("frequency")?.channels ?? [];
        if (tool === "frequency" && channels.length) {
          const channel = selectField("Displayed channel", channels.map((item, index) => [String(index), `${item.input.id} → ${item.output.id}`] as const), String(channelIndex), "channel");
          channel.addEventListener("change", () => { channelIndex = Number(channel.value); renderResult(); });
        }
        if (tool === "feedback" && answers.has("feedback")) {
          const view = selectField("View", [["bode", "Bode"], ["nyquist", "Nyquist"], ["sensitivity", "Sensitivity"]], feedbackView, "feedback-view");
          view.addEventListener("change", () => { feedbackView = view.value as typeof feedbackView; renderResult(); });
        }
        return;
      }
      case "measuredFrequency": {
        // The recording and its sample time are the engine's: the answer names the recording it measured (B10.3), and the page holds none of its own.
        const measured = answerFor("measuredFrequency");
        textField("Frequencies (rad/s, comma separated)", listText(inputs.frequencies ?? undefined), "periodic-frequencies");
        textField("Recording", measured?.recordingId ?? "", "periodic-recording", true);
        textField("Recorded sample time (s)", measured ? String(measured.sampleTimeSeconds) : "", "periodic-sample-time", true);
        numberField("Discard startup samples", inputs.startupSamples ?? undefined, "periodic-startup");
        numberField("Segments", inputs.segmentCount ?? undefined, "periodic-segments");
        numberField("Minimum coherence", inputs.minimumCoherence ?? undefined, "periodic-coherence");
        return;
      }
      case "leadLag": {
        const mode = selectField("Compensation", [["lead", "Lead compensation"], ["lag", "Lag compensation"]], inputs.mode ?? undefined, "lead-lag-mode");
        // A change of compensation shows the entries of that compensation only; the other value is not carried over.
        mode.addEventListener("change", () => { inputsByTool.set(tool, readInputs(tool)); renderEditor(); });
        const ratio = mode.value === "lag";
        numberField("Crossover (rad/s)", inputs.crossoverRadPerSecond ?? undefined, "lead-lag-crossover");
        numberField(ratio ? "Low-frequency gain ratio" : "Phase contribution (°)", inputs.value ?? undefined, ratio ? "lead-lag-ratio" : "lead-lag-phase");
        return;
      }
      case "initialState": {
        const stepSeconds = inputs.stepSeconds ?? undefined, durationSeconds = inputs.durationSeconds ?? undefined;
        textField("Step", stepSeconds === undefined ? "" : formatDurationSeconds(stepSeconds), "response-step");
        textField("Duration", durationSeconds === undefined ? "" : formatDurationSeconds(durationSeconds), "response-duration");
        textField("Initial state", listText(inputs.initialState ?? undefined), "response-initial");
        textField("Held input", listText(inputs.input ?? undefined), "response-input");
        selectField("Connection", [["open-loop", "Open loop"], ["feedback", "Declared feedback"]], inputs.connection ?? undefined, "response-connection");
        const outputs = answerFor("initialState")?.outputs ?? [];
        if (outputs.length) {
          const output = selectField("Displayed output", outputs.map((item, index) => [String(index), `${item.id} (${item.unit})`] as const),
            String(Math.min(responseOutputIndex, outputs.length - 1)), "response-output");
          output.addEventListener("change", () => { responseOutputIndex = Number(output.value); renderResult(); });
        }
        return;
      }
      case "poleZero":
        selectField("Pole view", [["unreduced", "Unreduced internal model"], ["minimal", "Minimal input/output view"]], inputs.view ?? undefined, "pole-view");
        return;
      default:
        // Stage state and the relative gain array take no inputs.
        return;
    }
  };

  /** The entries of the editor as the engine's inputs: a blank or unreadable entry is absent, so the engine's default applies. */
  function readInputs(tool: AnalysisTool): AnalysisInputs {
    const text = (name: string): string => editor.querySelector<HTMLInputElement | HTMLSelectElement>(`[data-analysis-${name}]`)?.value ?? "";
    const number = (name: string): number | undefined => numberOf(text(name));
    switch (tool) {
      case "rootLocus":
      case "routh":
        return { selectedValue: number("parameter-value") };
      case "frequency":
      case "feedback": {
        const minimumFrequency = number("minimum-frequency"), maximumFrequency = number("maximum-frequency"), points = number("frequency-points");
        // A grid is sent whole, or not at all: an absent grid takes the engine's grid.
        return { grid: minimumFrequency === undefined || maximumFrequency === undefined || points === undefined ? undefined
          : { minimumFrequency, maximumFrequency, points } };
      }
      case "measuredFrequency":
        // No recording is named here: the engine takes the study's last kept recording that meets the periodic rule (B9.9).
        return {
          frequencies: numberList(text("periodic-frequencies")),
          segmentCount: number("periodic-segments"),
          minimumCoherence: number("periodic-coherence"),
          startupSamples: number("periodic-startup"),
        };
      case "leadLag": {
        const mode = choiceOf(text("lead-lag-mode"), ["lead", "lag"] as const);
        return {
          mode,
          crossoverRadPerSecond: number("lead-lag-crossover"),
          value: number(mode === "lag" ? "lead-lag-ratio" : "lead-lag-phase"),
        };
      }
      case "initialState":
        return {
          stepSeconds: parseDurationSeconds(text("response-step")),
          durationSeconds: parseDurationSeconds(text("response-duration")),
          initialState: numberList(text("response-initial")),
          input: numberList(text("response-input")),
          connection: choiceOf(text("response-connection"), ["open-loop", "feedback"] as const),
        };
      case "poleZero":
        return { view: choiceOf(text("pole-view"), ["unreduced", "minimal"] as const) };
      default:
        return {};
    }
  }

  /** The parameter's name and unit, as the last answer names them; the label is the engine's, not the page's. */
  const parameterLabel = (tool: "rootLocus" | "routh"): string => {
    const answer = tool === "rootLocus" ? answerFor("rootLocus") : answerFor("routh");
    return answer ? `${answer.parameter} (${answer.parameterUnit})` : "Selected value";
  };

  const renderResult = (): void => {
    if (disposed) return;
    reportShown();
    const offer = offerOf(selected);
    disclosure.hidden = !offer;
    if (!selected || !offer) { editor.hidden = true; result.hidden = true; chart.clear(); return; }
    const envelope = answers.get(selected);
    statusLine.textContent = refusal?.message ?? (pending ? pending.progress ?? "Calculating analysis…" : notice)
      ?? (envelope ? "" : "Analysis evidence is not available yet.");
    plotHost.hidden = !PLOTTED_TOOLS.has(selected);
    redrawPlot = undefined; plotHost.style.height = "280px";
    chart.clear();
    summary.replaceChildren(); detailsSummary.replaceChildren(); table.replaceChildren(); tableDetails.hidden = true; (tableDetails as HTMLDetailsElement).open = false;
    result.hidden = !envelope?.result;
    if (!envelope?.result) return;
    const answer = answerOf(envelope.result, envelope.tool);
    if (!answer) return;
    for (const sentence of answer.summary) addText(sentence);
    for (const sentence of answer.details) addDetailsText(sentence);
    if (refusal?.detail) addDetailsText(refusal.detail);
    switch (envelope.tool) {
      case "rootLocus": { const drawn = answerFor("rootLocus"); if (drawn) drawRootLocus(drawn); break; }
      case "routh": { const drawn = answerFor("routh"); if (drawn) drawRouth(drawn); break; }
      case "poleZero": { const drawn = answerFor("poleZero"); if (drawn) drawPoles(drawn); break; }
      case "frequency": { const drawn = answerFor("frequency"); if (drawn) drawFrequency(drawn); break; }
      case "measuredFrequency": { const drawn = answerFor("measuredFrequency"); if (drawn) drawMeasured(drawn); break; }
      case "feedback": { const drawn = answerFor("feedback"); if (drawn) drawFeedback(drawn); break; }
      case "leadLag": { const drawn = answerFor("leadLag"); if (drawn) drawLeadLag(drawn); break; }
      case "initialState": { const drawn = answerFor("initialState"); if (drawn) drawResponse(drawn); break; }
      case "stateStructure": { const drawn = answerFor("stateStructure"); if (drawn) drawStateStructure(drawn); break; }
      case "relativeGain": { const drawn = answerFor("relativeGain"); if (drawn) drawRelativeGain(drawn); break; }
    }
  };

  function drawRootLocus(answer: RootLocusAnswer): void {
    const theme = chartTheme();
    const frame = base("Real axis", "Imaginary axis", "Root locus");
    // Padding keeps the ends of the paths and the outermost labels inside the plot frame.
    chart.setOption({ ...frame, xAxis: { ...(frame.xAxis as object), boundaryGap: ["8%", "8%"] }, yAxis: { ...(frame.yAxis as object), boundaryGap: ["18%", "18%"] },
      tooltip: rootLocusTooltip(answer),
      series: rootLocusChartSeries(answer, { path: theme.pv, inspected: theme.auxiliaryOrange, selected: theme.auxiliaryPurple, foreground: theme.foreground }) }, { notMerge: true });
    if (answer.approximation && !answer.details.includes(answer.approximation)) addDetailsText(answer.approximation);
    addRows(["Gain", "Poles"], answer.branches.map(branch => [formatEngineeringNumber(branch.gain), poleText(branch.poles)]));
  }

  function drawRouth(answer: RouthAnswer): void {
    const polynomial = answer.polynomial;
    addRows(["Power of s", "Entries"], polynomial.rows.map(row => [String(row.power), formatNumberList(row.entries)]));
    if (polynomial.symbolicRows?.length) addRows(["Power of s", "Exact symbolic entries"], polynomial.symbolicRows.map(row => [String(row.power), row.entries.join(", ")]));
    // The boundaries are the engine's exact enclosures as it writes them; the page shows the text and computes no decimal of its own.
    const boundaries = answer.interval.boundaries ?? [];
    if (boundaries.length) addRows(["Boundary polynomial", "Boundary"], boundaries.map(boundary =>
      [boundary.polynomial.join(", "), boundary.lower === boundary.upper ? boundary.lower : `${boundary.lower} – ${boundary.upper}`]));
  }

  function drawPoles(answer: PoleZeroAnswer): void {
    const discrete = answer.domain === "z";
    const poles = answer.poles, zeros = answer.zeros;
    const series: Record<string, unknown>[] = [
      { ...engineeringLine("Poles", 0, poles.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "circle", symbolSize: 11 },
      ...(zeros.length ? [{ ...engineeringLine("Zeros", 0, zeros.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "diamond", symbolSize: 12 }] : []),
    ];
    // The unit circle is drawn with equal units on both axes, so that a pole's distance from it reads true.
    if (discrete) series.push({ name: "Unit circle", type: "line", showSymbol: false, data: Array.from({ length: 65 }, (_, index) => { const angle = index * 2 * Math.PI / 64; return [Math.cos(angle), Math.sin(angle)]; }), lineStyle: { type: "dashed" } });
    drawComplexPlane(discrete ? "Real axis (z)" : "Real axis (s)", discrete ? "Imaginary axis (z)" : "Imaginary axis (s)", discrete ? "Discrete pole-zero map" : "Continuous pole-zero map",
      area => poleMapGeometry(poles, zeros, discrete, area), () => series);
    addRows(["Kind", "Coordinates"], [
      ...poles.map(point => ["Pole", `(${formatEngineeringNumber(point.real)}, ${formatEngineeringNumber(point.imaginary)})`]),
      ...zeros.map(point => ["Zero", `(${formatEngineeringNumber(point.real)}, ${formatEngineeringNumber(point.imaginary)})`]),
    ]);
  }

  function drawFrequency(answer: FrequencyAnswer): void {
    const channel = answer.channels[channelIndex] ?? answer.channels[0];
    if (!channel) return;
    chart.setOption({ ...base("Frequency (rad/s)", "Magnitude / phase", `${channel.input.id} → ${channel.output.id}`), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), yAxis: [axis("Magnitude (dB)", "value", chartTheme()), phaseAxis("Phase (°)", chartTheme(), channel.points.map(point => point.phaseDegrees))], series: [
      engineeringLine("Magnitude (dB)", 0, channel.points.map(point => [point.frequencyRadPerSecond, point.magnitudeDb]), "solid"),
      { ...engineeringLine("Phase (°)", 1, channel.points.map(point => [point.frequencyRadPerSecond, point.phaseDegrees]), "dashed"), yAxisIndex: 1, tooltip: { valueFormatter: formatPhaseChartValue } },
    ] }, { notMerge: true });
    addRows(["Frequency (rad/s)", "Magnitude", "Phase (°)"], channel.points.slice(0, 128).map(point => [formatEngineeringNumber(point.frequencyRadPerSecond), formatEngineeringNumber(point.magnitude), formatPhaseDegrees(point.phaseDegrees)]));
  }

  function drawMeasured(answer: MeasuredFrequencyAnswer): void {
    // A rejected measurement has no points: its sentences say why (summary).
    if (answer.status !== "qualified") return;
    const rows = answer.points;
    chart.setOption({ ...base("Frequency (rad/s)", "Magnitude / phase", "Measured periodic frequency response"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), yAxis: [axis("Magnitude (dB)", "value", chartTheme()), phaseAxis("Phase (°)", chartTheme(), rows.map(row => row.phaseDegrees))], series: [
      { ...engineeringLine("Measured magnitude", 0, rows.map(row => [row.frequencyRadPerSecond, row.magnitudeDb]), "solid"), showSymbol: true, symbol: "circle", symbolSize: 9 },
      { ...engineeringLine("Measured phase", 1, rows.map(row => [row.frequencyRadPerSecond, row.phaseDegrees]), "dashed"), showSymbol: true, symbol: "circle", symbolSize: 8, yAxisIndex: 1, tooltip: { valueFormatter: formatPhaseChartValue } },
    ] }, { notMerge: true });
    addRows(["Frequency (rad/s)", "Magnitude", "Phase (°)", "Coherence"], rows.map(row => [formatEngineeringNumber(row.frequencyRadPerSecond), formatEngineeringNumber(row.magnitude), formatPhaseDegrees(row.phaseDegrees), formatEngineeringNumber(row.coherence)]));
  }

  function drawFeedback(answer: FeedbackAnswer): void {
    if (feedbackView === "nyquist") {
      // A Nyquist plot keeps equal units on both axes, so that distances to −1 read true.
      const points = answer.openLoop.map(point => ({ real: point.real, imaginary: point.imaginary }));
      drawComplexPlane("Real", "Imaginary", "Nyquist", area => nyquistGeometry(points, area), () => {
        const theme = chartTheme();
        return nyquistSeries(points, { positive: theme.pv, negative: theme.auxiliaryPurple, critical: theme.mv, label: theme.foreground });
      });
      return;
    }
    if (feedbackView === "sensitivity") {
      // The engine gives no sensitivity when it cannot form one (the answer's null): the plot then shows nothing.
      if (!answer.sensitivity) return;
      chart.setOption({ ...base("Frequency (rad/s)", "Magnitude (dB)", "Sensitivity"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), series: sensitivitySeries(answer.sensitivity, { sensitivity: chartTheme().pv, complementary: chartTheme().auxiliaryOrange, control: chartTheme().auxiliaryPurple }) }, { notMerge: true });
      return;
    }
    // The loop's Bode view draws magnitude and phase against frequency, the phase on its own axis as the frequency response does.
    const loop = answer.openLoop;
    chart.setOption({ ...base("Frequency (rad/s)", "Magnitude (dB)", "Bode"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()),
      yAxis: [axis("Magnitude (dB)", "value", chartTheme()), phaseAxis("Phase (°)", chartTheme(), loop.map(point => point.phaseDegrees))],
      series: [
        engineeringLine("Loop magnitude (dB)", 0, loop.map(point => [point.frequencyRadPerSecond, point.magnitudeDb]), "solid"),
        { ...engineeringLine("Loop phase (°)", 1, loop.map(point => [point.frequencyRadPerSecond, point.phaseDegrees]), "dashed"), yAxisIndex: 1, tooltip: { valueFormatter: formatPhaseChartValue } },
      ] }, { notMerge: true });
  }

  function drawLeadLag(answer: LeadLagAnswer): void {
    const labels: Record<string, string> = { gain: "Gain", zeroRadPerSecond: "Zero (rad/s)", poleRadPerSecond: "Pole (rad/s)", phaseContributionDegrees: "Phase contribution (°)", lowFrequencyGainRatio: "Low-frequency gain ratio", openLoopMagnitudeAtCrossover: "Open-loop magnitude at crossover" };
    addRows(["Quantity", "Value"], Object.entries(answer.design).filter(([key, value]) => key in labels && typeof value === "number")
      .map(([key, value]) => [labels[key], formatEngineeringNumber(value as number)]));
  }

  function drawResponse(answer: InitialStateAnswer): void {
    const outputs = answer.outputs ?? [];
    const outputIndex = Math.min(responseOutputIndex, outputs.length - 1);
    responseOutputIndex = Math.max(0, outputIndex);
    const output = outputs[responseOutputIndex];
    chart.setOption({ ...base("Time (s)", output?.unit ? `Output deviation (${output.unit})` : "Output deviation", "Initial-state response"), series: initialStateResponseSeries(answer.points, responseOutputIndex, { total: chartTheme().pv, zeroState: chartTheme().auxiliaryOrange, initialCondition: chartTheme().auxiliaryYellow }) }, { notMerge: true });
    addRows(["Time (s)", "Zero state", "Initial condition", "Total"], initialStateResponseRows(answer.points.slice(0, 256), responseOutputIndex));
  }

  function drawStateStructure(answer: StateStructureAnswer): void {
    if (answer.order) addDetailsText(answer.order);
    if (answer.matrices.length) addRows(["Matrix", "Rows in declared order"], answer.matrices.map(matrix => [matrix.name, formatMatrixRows(matrix.rows)]));
  }

  function drawRelativeGain(answer: RelativeGainAnswer): void {
    if (answer.status.status !== "qualified" || !answer.status.matrix.length) return;
    addVisibleMatrix("Relative gain array (RGA)", answer.status.matrix, answer.outputChannels.map(channel => channel.id), answer.inputChannels.map(channel => channel.id));
  }

  const addText = (value: string): void => { const p = document.createElement("p"); p.textContent = value; summary.append(p); };
  const addDetailsText = (value: string): void => { const p = document.createElement("p"); p.textContent = value; detailsSummary.append(p); tableDetails.hidden = false; };
  const addRows = (headers: readonly string[], rows: readonly (readonly string[])[]): void => { tableDetails.hidden = false; const element = document.createElement("table"); element.className = "trust-table"; const head = element.createTHead().insertRow(); headers.forEach(value => { const cell = document.createElement("th"); cell.textContent = value; head.append(cell); }); const body = element.createTBody(); rows.forEach(row => { const tr = body.insertRow(); row.forEach(value => { tr.insertCell().textContent = value; }); }); table.append(element); };
  const addVisibleMatrix = (label: string, matrix: readonly (readonly number[])[], rowNames: readonly string[], columnNames: readonly string[]): void => {
    const element = document.createElement("table"); element.className = "trust-table"; element.createCaption().textContent = label;
    const head = element.createTHead().insertRow(); head.append(document.createElement("th"));
    matrix[0]?.forEach((_, index) => { const th = document.createElement("th"); th.scope = "col"; th.textContent = columnNames[index] ?? `Input ${index + 1}`; head.append(th); });
    const body = element.createTBody();
    matrix.forEach((row, index) => { const tr = body.insertRow(); const name = document.createElement("th"); name.scope = "row"; name.textContent = rowNames[index] ?? `Output ${index + 1}`; tr.append(name); row.forEach(value => { tr.insertCell().textContent = formatEngineeringNumber(value); }); });
    summary.append(element);
  };
  /** A complex-plane plot with equal units on both axes, redrawn whenever the plot area changes size. */
  function drawComplexPlane(xName: string, yName: string, label: string, geometryFor: (area: PlaneArea) => PlaneGeometry,
    series: () => Record<string, unknown>[]): void {
    plotArea.style.height = "440px";
    const draw = () => {
      const option = base(xName, yName, label) as Record<string, any>;
      const geometry = geometryFor({ width: plotArea.clientWidth, height: plotArea.clientHeight });
      const fitted = (current: Record<string, any>, range: { min: number; max: number } | undefined) =>
        ({ ...current, ...range, axisLabel: { ...current.axisLabel, showMinLabel: false, showMaxLabel: false } });
      chart.resize();
      chart.setOption({ ...option, tooltip: { trigger: "item", valueFormatter: formatChartValue }, grid: geometry.grid,
        xAxis: fitted(option.xAxis, geometry.x), yAxis: fitted(option.yAxis, geometry.y), series: series() }, { notMerge: true });
    };
    draw(); redrawPlot = draw;
  }
  function base(xName: string, yName: string, label: string): Record<string, unknown> { const theme = chartTheme(); return { animation: false, aria: { enabled: true, description: label }, backgroundColor: theme.background, legend: { top: 4, textStyle: { color: theme.muted } }, tooltip: { trigger: "axis", valueFormatter: formatChartValue }, grid: { left: 64, right: 24, top: 36, bottom: 52 }, xAxis: axis(xName, "value", theme), yAxis: axis(yName, "value", theme) }; }

  render();
  return presenter;
}

function poleText(poles: readonly { real: number; imaginary: number }[]): string {
  return poles.map(pole => `(${formatEngineeringNumber(pole.real)}, ${formatEngineeringNumber(pole.imaginary)})`).join("; ");
}

/** A stage that changes the model under the analysis: the answers shown for the model before it are then stale (B9.9). */
function modelChanged(stage: string | undefined, status: unknown): boolean {
  if (stage === "engineer") return status === "model-ready" || status === "edited";
  if (stage === "general-model") return ["imported", "edited", "cancelled"].includes(String(status));
  return stage === "identify" && status === "complete";
}

/** A number typed in a field: a blank or unreadable entry is none, so that the engine's default applies. */
function numberOf(text: string): number | undefined {
  if (!text.trim()) return undefined;
  const value = Number(text);
  return Number.isFinite(value) ? value : undefined;
}

/** Comma-separated numbers typed in a field: a blank or unreadable list is none, so that the engine's default applies. */
function numberList(text: string): number[] | undefined {
  if (!text.trim()) return undefined;
  const items = text.split(",").map(item => item.trim());
  if (items.some(item => item === "")) return undefined;
  const values = items.map(Number);
  return values.every(value => Number.isFinite(value)) ? values : undefined;
}

function listText(values: readonly number[] | undefined): string {
  return values ? values.join(", ") : "";
}

function choiceOf<T extends string>(text: string, choices: readonly T[]): T | undefined {
  return choices.find(choice => choice === text);
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }

function emptyPresenter(): ThreePageAnalysisPresenter {
  return {
    receive: () => undefined, recordingsChanged: () => undefined, setTool: () => undefined, run: () => undefined, actionLabel: () => undefined,
    choice: () => undefined, availableTools: () => [], shown: () => undefined, offers: () => [], dispose: () => undefined,
    tools: () => undefined, started: () => undefined, answer: () => undefined, refused: () => undefined, update: () => undefined, restore: () => undefined,
  };
}
