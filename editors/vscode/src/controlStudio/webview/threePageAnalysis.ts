import * as echarts from "echarts/core";
import { LineChart, ScatterChart } from "echarts/charts";
import { AriaComponent, GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { DeclaredControlAnalysis, DeclaredRootLocusResult } from "../declaredAnalysis";
import type { DeclaredRouthResult } from "../routhHurwitz";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import type { GeneralFeedbackResult } from "../generalFeedback";
import { decodeGeneralModel, type GeneralModel, type GeneralModelFrequencyResult } from "../generalModel";
import { isSavedGeneralModelAnalysis, isSavedPeriodicFrequencyAnalysis, validModelGrid, type ModelFrequencyGrid, type SavedGeneralModelAnalysis, type SavedPeriodicFrequencyAnalysis } from "../generalModelPersistence";
import { isSavedLeadLagAnalysis, type SavedLeadLagAnalysis } from "../leadLagSession";
import { declaredFeedbackDefinition } from "../declaredFeedbackModel";
import { linearTransferModel } from "../linearTransferModel";
import type { FrequencyResponseEstimate } from "../numerics/contracts";
import type { ModelTimeResponse } from "../generalModelTime";
import type { ModelPoleView } from "../generalModelPoleView";
import { rootsOfRealPolynomial } from "../numerics/classicalAnalysis";
import { axis, chartTheme, engineeringLine, formatChartValue } from "./chartPrimitives";
import { formatEngineeringNumber } from "../chartContract";

echarts.use([LineChart, ScatterChart, GridComponent, LegendComponent, TooltipComponent, AriaComponent, CanvasRenderer]);

export type ThreePageAnalysisTool = "root-locus" | "routh" | "poles" | "frequency" | "periodic-frequency" | "feedback" | "lead-lag" | "response" | "state-structure" | "rga";
export interface ThreePageAnalysisAvailability {
  declared?: DeclaredControlAnalysis;
  model?: SavedGeneralModelAnalysis;
  selectedModel?: GeneralModel;
  plots?: EngineeringPlotMessageEvidence;
}
export interface ThreePageAnalysisPresenter {
  receive(stage: string | undefined, payload: Record<string, unknown>): void;
  setAvailable(availability: ThreePageAnalysisAvailability): readonly ThreePageAnalysisTool[];
  setTool(tool: ThreePageAnalysisTool | undefined): void;
  run(): void;
  actionLabel(): string | undefined;
  choice(): string | undefined;
  availableTools(): readonly ThreePageAnalysisTool[];
  dispose(): void;
}

/** Presents an externally selected analysis tool; it owns no page navigation or runtime writes. */
export function initializeThreePageAnalysis(
  root: HTMLElement,
  post: (kind: string, payload?: Record<string, unknown>) => void = () => undefined,
): ThreePageAnalysisPresenter {
  const disclosure = root.querySelector<HTMLElement>("[data-three-page-analysis]");
  const result = root.querySelector<HTMLElement>("[data-three-page-analysis-result]");
  const plotHost = root.querySelector<HTMLElement>("[data-three-page-analysis-plot]");
  const summary = root.querySelector<HTMLElement>("[data-three-page-analysis-summary]");
  const tableDetails = root.querySelector<HTMLElement>("[data-three-page-analysis-table-details]");
  const detailsSummary = root.querySelector<HTMLElement>("[data-three-page-analysis-details-summary]");
  const table = root.querySelector<HTMLElement>("[data-three-page-analysis-table]");
  if (!disclosure || !result || !plotHost || !summary || !tableDetails || !detailsSummary || !table) return emptyPresenter();

  const status = document.createElement("p"); status.setAttribute("role", "status"); status.className = "studio-action-message";
  const editor = document.createElement("section"); editor.className = "studio-fields"; editor.hidden = true;
  disclosure.insertBefore(status, result); disclosure.insertBefore(editor, result);
  const chart = echarts.init(plotHost, undefined, { renderer: "canvas" });
  const resizeObserver = new ResizeObserver(() => chart.resize()); resizeObserver.observe(plotHost);
  const resize = () => chart.resize(); window.addEventListener("resize", resize);
  let available: ThreePageAnalysisAvailability = {};
  let selected: ThreePageAnalysisTool | undefined;
  let declaredRoot: DeclaredRootLocusResult | undefined;
  let routh: DeclaredRouthResult | undefined;
  let model: SavedGeneralModelAnalysis | undefined;
  let selectedModel: GeneralModel | undefined;
  let plots: EngineeringPlotMessageEvidence | undefined;
  let feedback: GeneralFeedbackResult | undefined;
  let pendingModelAnalysis = false;
  let pendingRouth = false;
  let channelIndex = 0;
  let frequencyDraft: ModelFrequencyGrid | undefined;
  let parameterDraft: number | undefined;
  let feedbackView: "bode" | "nyquist" | "sensitivity" = "bode";
  let periodicRecording: { input: number[]; output: number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string } | undefined;
  let periodicResult: FrequencyResponseEstimate | undefined;
  let periodicRecordingFailure: string | undefined;
  let leadLagResult: SavedLeadLagAnalysis["result"] | undefined;
  let leadLagMode: "lead" | "lag" = "lead";
  let periodicDraft = { frequencies: "1,2,4", sampleTimeSeconds: 0.1, segments: 4, minimumCoherence: 0.8, startupSamples: 16 };
  let leadLagDraft = { crossoverRadPerSecond: 1, phaseDegrees: 30, lowFrequencyGainRatio: 5 };
  let timeResponse: ModelTimeResponse | undefined;
  let poleView: ModelPoleView | undefined;
  let responseDraft = { stepSeconds: 0.1, durationSeconds: 10, initialState: "", input: "", connection: "open-loop" as "open-loop" | "feedback" };
  let responseOutputIndex = 0;
  let poleViewChoice: "unreduced" | "minimal" = "unreduced";
  let failure: string | undefined;
  let disposed = false;

  const toolSet = (): ThreePageAnalysisTool[] => {
    const tools: ThreePageAnalysisTool[] = [];
    if (available.declared || declaredRoot || plots?.rootLocus?.status === "qualified") tools.push("root-locus", "routh");
    if (available.model || model || selectedModel) tools.push("poles", "frequency");
    if (periodicRecording || periodicRecordingFailure) tools.push("periodic-frequency");
    if ((available.declared && selectedModel && declaredFeedbackDefinition(available.declared, selectedModel)) || available.plots || plots || feedback) tools.push("feedback");
    if (selectedModel && transferModel(selectedModel)) tools.push("lead-lag");
    if (available.model || model || selectedModel) tools.push("response");
    if (selectedModel && selectedModel.kind !== "transfer-function") tools.push("state-structure");
    if (selectedModel && selectedModel.inputs.length === selectedModel.outputs.length && selectedModel.inputs.length > 1) tools.push("rga");
    return [...new Set(tools)];
  };
  const setAvailable = (next: ThreePageAnalysisAvailability): readonly ThreePageAnalysisTool[] => {
    available = next; model = next.model; selectedModel = next.selectedModel ?? next.model?.model; feedback = next.model?.feedback?.result; plots = next.plots; timeResponse = next.model?.response; poleView = next.model?.poleView;
    frequencyDraft = next.model?.grid; parameterDraft = next.declared?.rootLocus.selectedValue; render(); return toolSet();
  };
  const setTool = (tool: ThreePageAnalysisTool | undefined): void => {
    selected = tool && toolSet().includes(tool) ? tool : undefined; render();
  };
  const actionLabel = (): string | undefined => selected === "root-locus" ? "Calculate root locus"
    : selected === "routh" ? (pendingRouth ? "Abort analysis" : "Calculate Routh–Hurwitz") : selected === "poles" ? (pendingModelAnalysis ? "Abort analysis" : "Calculate poles and zeros")
    : selected === "frequency" ? (pendingModelAnalysis ? "Abort analysis" : "Calculate frequency response")
    : selected === "periodic-frequency" && periodicRecording ? "Calculate measured frequency response"
    : selected === "lead-lag" ? "Calculate lead/lag design"
    : selected === "response" ? (pendingModelAnalysis ? "Abort analysis" : "Calculate initial-state response")
    : selected === "state-structure" ? (pendingModelAnalysis ? "Abort analysis" : "Calculate state structure")
    : selected === "rga" ? (pendingModelAnalysis ? "Abort analysis" : "Calculate relative gain array")
    : selected === "feedback" && (model || selectedModel) ? (pendingModelAnalysis ? "Abort analysis" : "Calculate feedback analysis") : undefined;
  const choice = (): string | undefined => selected ? `analysis:${selected}` : undefined;
  const run = (): void => {
    if (!selected) return;
    if (selected === "root-locus" || selected === "routh") {
      if (selected === "routh" && pendingRouth) { post("control-studio.routh.cancel"); return; }
      const input = editor.querySelector<HTMLInputElement>("[data-analysis-parameter-value]");
      failure = undefined;
      if (selected === "routh") pendingRouth = true;
      status.textContent = "Calculating analysis…";
      post(selected === "routh" ? "control-studio.routh.run" : "control-studio.declared-root.run", { ...(input?.value.trim() ? { selectedValue: Number(input.value) } : {}) });
      return;
    }
    if (selected === "periodic-frequency" && periodicRecording) {
      const value = (name: string) => editor.querySelector<HTMLInputElement>(`[data-analysis-${name}]`)?.value ?? "";
      const frequencies = parseNumberList(value("periodic-frequencies"));
      const sampleTime = parseFiniteNumber(value("periodic-sample-time")); const segments = parseFiniteNumber(value("periodic-segments")); const coherence = parseFiniteNumber(value("periodic-coherence")); const startup = parseFiniteNumber(value("periodic-startup"));
      if (!frequencies || frequencies.some(frequency => frequency <= 0) || sampleTime === undefined || sampleTime <= 0 || segments === undefined || !Number.isSafeInteger(segments) || segments < 2 || coherence === undefined || coherence < 0 || coherence > 1 || startup === undefined || !Number.isSafeInteger(startup) || startup < 1) { failure = "Enter finite, positive periodic settings without blank frequency entries."; render(); return; }
      periodicRecording = { ...periodicRecording, sampleTimeSeconds: sampleTime };
      periodicDraft = { frequencies: value("periodic-frequencies"), sampleTimeSeconds: sampleTime, segments, minimumCoherence: coherence, startupSamples: startup };
      periodicResult = undefined;
      post("control-studio.general-model.periodic", { frequencies, segmentCount: segments, minimumCoherence: coherence, startupSamples: startup, recordingId: periodicRecording.recordingId, recordingRevision: periodicRecording.recordingRevision });
      return;
    }
    if (selected === "lead-lag" && selectedModel) {
      const value = (name: string) => parseFiniteNumber(editor.querySelector<HTMLInputElement>(`[data-analysis-${name}]`)?.value ?? "");
      leadLagMode = (editor.querySelector<HTMLSelectElement>("[data-analysis-lead-lag-mode]")?.value ?? "lead") as typeof leadLagMode;
      const crossover = value("lead-lag-crossover"); const selectedValue = value(leadLagMode === "lead" ? "lead-lag-phase" : "lead-lag-ratio");
      if (crossover === undefined || selectedValue === undefined) { failure = "Enter finite lead/lag settings."; render(); return; }
      leadLagDraft = { ...leadLagDraft, crossoverRadPerSecond: crossover, ...(leadLagMode === "lead" ? { phaseDegrees: selectedValue } : { lowFrequencyGainRatio: selectedValue }) };
      leadLagResult = undefined;
      status.textContent = "Calculating lead/lag design…";
      post("control-studio.lead-lag.run", { mode: leadLagMode, crossoverRadPerSecond: leadLagDraft.crossoverRadPerSecond, value: selectedValue });
      return;
    }
    if (selected === "response" && (model || selectedModel)) {
      const selectedModelValue = model?.model ?? selectedModel; if (!selectedModelValue) return;
      const value = (name: string) => editor.querySelector<HTMLInputElement>(`[data-analysis-${name}]`)?.value ?? "";
      responseDraft = { stepSeconds: Number(value("response-step")), durationSeconds: Number(value("response-duration")), initialState: value("response-initial"), input: value("response-input"), connection: (editor.querySelector<HTMLSelectElement>("[data-analysis-response-connection]")?.value ?? "open-loop") as typeof responseDraft.connection };
      const stateCount = selectedModelValue.kind === "transfer-function" ? selectedModelValue.denominator.length - 1 : selectedModelValue.states.length;
      const initialState = parseNumberList(responseDraft.initialState, stateCount === 0); const input = parseNumberList(responseDraft.input);
      if (parseFiniteNumber(value("response-step")) === undefined || parseFiniteNumber(value("response-duration")) === undefined || !initialState || initialState.length !== stateCount || !input || input.length !== selectedModelValue.inputs.length) { failure = "Enter finite response settings and complete state/input vectors."; render(); return; }
      pendingModelAnalysis = true; render();
      post("control-studio.general-model.response", { stepSeconds: responseDraft.stepSeconds, durationSeconds: responseDraft.durationSeconds, initialState, input, connection: responseDraft.connection });
      return;
    }
    if (selected === "poles" && (model || selectedModel)) {
      if (pendingModelAnalysis) { post("control-studio.general-model.cancel"); return; }
      poleViewChoice = (editor.querySelector<HTMLSelectElement>("[data-analysis-pole-view]")?.value ?? "unreduced") as typeof poleViewChoice;
      pendingModelAnalysis = true; render();
      post("control-studio.general-model.pole-view", { view: poleViewChoice });
      return;
    }
    if ((selected === "frequency" || selected === "poles" || selected === "feedback" || selected === "state-structure" || selected === "rga") && (model || selectedModel)) {
      if (pendingModelAnalysis) { post("control-studio.general-model.cancel"); return; }
      if (selected === "state-structure" || selected === "rga") {
        const grid = frequencyDraft ?? model?.grid ?? { minimumFrequency: 0.1, maximumFrequency: 10, points: 32 };
        frequencyDraft = grid; pendingModelAnalysis = true; render();
        post("control-studio.general-model.analyze", { ...grid });
        return;
      }
      const value = (name: string) => parseFiniteNumber(editor.querySelector<HTMLInputElement>(`[data-analysis-${name}]`)?.value ?? "");
      const minimumFrequency = value("minimum-frequency"); const maximumFrequency = value("maximum-frequency"); const points = value("frequency-points");
      if (minimumFrequency === undefined || maximumFrequency === undefined || points === undefined) { failure = "Enter finite frequency bounds and point count."; render(); return; }
      frequencyDraft = { minimumFrequency, maximumFrequency, points };
      pendingModelAnalysis = true; render();
      post("control-studio.general-model.analyze", { minimumFrequency, maximumFrequency, points });
    }
  };
  const receive = (stage: string | undefined, payload: Record<string, unknown>): void => {
    if (disposed) return;
    if (stage === "declared-root" && isRecord(payload.result)) { declaredRoot = payload.result as unknown as DeclaredRootLocusResult; parameterDraft = declaredRoot.selectedValue; failure = undefined; }
    if (stage === "routh" && isRecord(payload.result)) { routh = payload.result as unknown as DeclaredRouthResult; parameterDraft = routh.selectedValue; pendingRouth = false; failure = undefined; }
    if (stage === "declared-root" && payload.status === "failed") { declaredRoot = undefined; failure = `Root locus unavailable: ${String(payload.reason ?? "the declared model was rejected")}`; }
    if (stage === "routh" && payload.status === "failed") { pendingRouth = false; routh = undefined; failure = `Routh–Hurwitz unavailable: ${String(payload.reason ?? "the declared model was rejected")}`; }
    if (stage === "routh" && payload.status === "cancelled") pendingRouth = false;
    if (stage === "general-model" && payload.status === "imported" && isRecord(payload.model)) {
      const imported = decodeGeneralModel(payload.model); if (imported) selectedModel = imported;
      model = undefined; feedback = undefined; leadLagResult = undefined; timeResponse = undefined; poleView = undefined; declaredRoot = undefined; routh = undefined; frequencyDraft = undefined; pendingModelAnalysis = false;
    }
    if (stage === "general-model" && ["edited", "cancelled"].includes(String(payload.status))) {
      model = undefined; feedback = undefined; leadLagResult = undefined; timeResponse = undefined; poleView = undefined; declaredRoot = undefined; routh = undefined; frequencyDraft = undefined; pendingModelAnalysis = false;
    }
    if (stage === "engineer" && payload.status === "model-ready" && isRecord(payload.model)) {
      const selected = decodeGeneralModel(payload.model);
      if (selected && selectedModel?.revision !== selected.revision) { model = undefined; feedback = undefined; leadLagResult = undefined; timeResponse = undefined; poleView = undefined; declaredRoot = undefined; routh = undefined; frequencyDraft = undefined; }
      if (selected) selectedModel = selected;
    }
    if (stage === "engineer" && payload.status === "edited") {
      model = undefined; selectedModel = undefined; feedback = undefined; leadLagResult = undefined; timeResponse = undefined; poleView = undefined; declaredRoot = undefined; routh = undefined; frequencyDraft = undefined;
    }
    if (stage === "engineer" && payload.status === "candidate") feedback = undefined;
    const recording = decodePeriodicRecording(payload);
    if (recording) { periodicRecording = recording; periodicRecordingFailure = undefined; periodicDraft = { ...periodicDraft, sampleTimeSeconds: recording.sampleTimeSeconds }; periodicResult = undefined; }
    else if (hasRawRecording(payload)) { periodicRecording = undefined; periodicRecordingFailure = "Periodic measurement requires finite good-quality samples without applied-output clipping."; periodicResult = undefined; }
    if (stage === "general-model" && isSavedGeneralModelAnalysis(payload.saved)) {
      model = payload.saved; selectedModel = payload.saved.model; feedback = payload.saved.feedback?.result;
      frequencyDraft = payload.saved.grid;
      timeResponse = payload.saved.response; if (timeResponse) responseDraft = { stepSeconds: timeResponse.settings.stepSeconds, durationSeconds: timeResponse.settings.durationSeconds, initialState: timeResponse.settings.initialState.join(","), input: timeResponse.settings.input.join(","), connection: timeResponse.settings.connection };
      poleView = payload.saved.poleView; if (poleView) poleViewChoice = poleView.view;
      pendingModelAnalysis = false; failure = undefined; channelIndex = Math.min(channelIndex, model.result.channels.length - 1);
    }
    if (stage === "general-model" && payload.status === "running") pendingModelAnalysis = true;
    if (stage === "general-model" && payload.status === "periodic-complete" && isSavedPeriodicFrequencyAnalysis(payload.periodic)) { periodicResult = payload.periodic.result; periodicRecordingFailure = undefined; }
    if (stage === "lead-lag" && payload.status === "complete" && isSavedLeadLagAnalysis(payload.saved)) {
      const saved = payload.saved;
      leadLagResult = saved.result; leadLagMode = saved.mode;
      leadLagDraft = { ...leadLagDraft, crossoverRadPerSecond: saved.crossoverRadPerSecond,
        ...(saved.mode === "lead" ? { phaseDegrees: saved.value } : { lowFrequencyGainRatio: saved.value }) };
      failure = undefined;
    }
    if (stage === "lead-lag" && payload.status === "failed") failure = `Lead/lag design unavailable: ${String(payload.reason ?? "the selected model was rejected")}`;
    if (stage === "general-model" && ["failed", "cancelled", "edited"].includes(String(payload.status))) pendingModelAnalysis = false;
    if (stage === "general-model" && payload.status === "failed") failure = `Model analysis unavailable: ${String(payload.reason ?? "the supplied model was rejected")}`;
    if (stage === "general-model" && ["imported", "edited", "cancelled"].includes(String(payload.status))) failure = undefined;
    if (isRecord(payload.plots)) plots = payload.plots as unknown as EngineeringPlotMessageEvidence;
    const selectedAnalysis = modelStudy(payload.modelAnalysis);
    if (selectedAnalysis) { model = selectedAnalysis; selectedModel = selectedAnalysis.model; frequencyDraft = selectedAnalysis.grid; periodicResult = selectedAnalysis.periodic?.result; }
    render();
  };

  const render = (): void => {
    if (disposed) return;
    const tools = toolSet();
    disclosure.hidden = selected === undefined || !tools.includes(selected);
    if (!selected || !tools.includes(selected)) { editor.hidden = true; result.hidden = true; chart.clear(); return; }
    editor.replaceChildren(); renderEditor();
    const drawsPlot = selected === "root-locus" || selected === "frequency" || selected === "periodic-frequency" || selected === "poles" || selected === "feedback" || selected === "response";
    plotHost.hidden = !drawsPlot;
    if (!drawsPlot) chart.clear();
    if (!hasEvidence(selected)) {
      result.hidden = true; status.textContent = failure ?? periodicRecordingFailure ?? (pendingModelAnalysis ? "Calculating model analysis…" : "Analysis evidence is not available yet."); chart.clear(); return;
    }
    result.hidden = false; status.textContent = failure ?? (pendingModelAnalysis ? "Calculating model analysis…" : ""); summary.replaceChildren(); detailsSummary.replaceChildren(); table.replaceChildren(); tableDetails.hidden = true; (tableDetails as HTMLDetailsElement).open = false;
    if (failure) addText("The new calculation failed; the retained result is shown for context.");
    if (selected === "root-locus") renderRootLocus();
    else if (selected === "routh") renderRouth();
    else if (selected === "frequency") renderFrequency();
    else if (selected === "periodic-frequency") renderPeriodicFrequency();
    else if (selected === "poles") renderPoles();
    else if (selected === "feedback") renderFeedback();
    else if (selected === "lead-lag") renderLeadLag();
    else if (selected === "response") renderResponse();
    else if (selected === "state-structure") renderStateStructure();
    else renderRga();
  };

  const hasEvidence = (tool: ThreePageAnalysisTool): boolean => tool === "root-locus" ? Boolean(declaredRoot || plots?.rootLocus?.status === "qualified")
    : tool === "routh" ? Boolean(routh)
    : tool === "frequency" || tool === "poles" ? Boolean(model)
    : tool === "periodic-frequency" ? Boolean(periodicResult)
    : tool === "feedback" ? Boolean(plots || feedback)
    : tool === "lead-lag" ? Boolean(leadLagResult)
    : tool === "response" ? Boolean(timeResponse)
    : tool === "state-structure" ? Boolean(model?.result.stateStructure)
    : Boolean(model?.result.relativeGainMatrix);

  const renderEditor = (): void => {
    if (!selected) return;
    editor.hidden = true;
    if (selected === "root-locus" || selected === "routh") renderDeclaredEditor();
    else if (selected === "frequency") renderFrequencyEditor();
    else if (selected === "poles") renderPoleViewEditor();
    else if (selected === "periodic-frequency") renderPeriodicEditor();
    else if (selected === "feedback") { renderFrequencyEditor(); renderFeedbackSelector(); }
    else if (selected === "lead-lag") renderLeadLagEditor();
    else if (selected === "response") renderResponseEditor();
    if (editor.childElementCount > 0) editor.hidden = false;
  };
  const renderDeclaredEditor = (): void => {
    const analysis = available.declared; if (!analysis) return;
    const parameterName = analysis.rootLocus.parameter; const parameter = analysis.controller.parameters[parameterName];
    const label = document.createElement("label"); label.className = "studio-field"; label.textContent = `${parameterName} (${parameter.unit})`;
    const input = document.createElement("input"); input.type = "number"; input.step = "any"; input.min = String(parameter.minimum); input.max = String(parameter.maximum); input.value = String(parameterDraft ?? analysis.rootLocus.selectedValue); input.addEventListener("input", () => { parameterDraft = Number(input.value); }); label.append(input); editor.append(label);
    input.dataset.analysisParameterValue = "true";
  };
  const renderFrequencyEditor = (): void => {
    const grid = frequencyDraft ?? model?.grid ?? { minimumFrequency: 0.1, maximumFrequency: 100, points: 128 };
    for (const [id, labelText, value] of [["minimum", "Minimum frequency (rad/s)", grid.minimumFrequency], ["maximum", "Maximum frequency (rad/s)", grid.maximumFrequency], ["points", "Frequency points", grid.points]] as const) {
      const label = document.createElement("label"); label.className = "studio-field"; label.textContent = labelText;
      const input = document.createElement("input"); input.type = "number"; input.step = "any"; input.value = String(value); input.setAttribute(`data-analysis-${id === "minimum" ? "minimum-frequency" : id === "maximum" ? "maximum-frequency" : "frequency-points"}`, ""); input.addEventListener("input", () => { const current = frequencyDraft ?? grid; frequencyDraft = { ...current, ...(id === "minimum" ? { minimumFrequency: Number(input.value) } : id === "maximum" ? { maximumFrequency: Number(input.value) } : { points: Number(input.value) }) }; }); label.append(input); editor.append(label);
    }
    const channels = model?.result.channels ?? [];
    const channel = document.createElement("select"); channel.className = "trust-input"; channels.forEach((item, index) => { const option = new Option(`${item.input.id} → ${item.output.id}`, String(index)); channel.append(option); }); channel.value = String(channelIndex); channel.addEventListener("change", () => { channelIndex = Number(channel.value); render(); });
    const channelLabel = document.createElement("label"); channelLabel.className = "studio-field"; channelLabel.textContent = "Displayed channel"; channelLabel.append(channel); editor.append(channelLabel);
  };
  const renderPeriodicEditor = (): void => {
    const fields: readonly [string, string, string][] = [
      ["periodic-frequencies", "Frequencies (rad/s, comma separated)", periodicDraft.frequencies],
      ["periodic-sample-time", "Recorded sample time (s)", String(periodicDraft.sampleTimeSeconds)],
      ["periodic-startup", "Discard startup samples", String(periodicDraft.startupSamples)],
      ["periodic-segments", "Segments", String(periodicDraft.segments)],
      ["periodic-coherence", "Minimum coherence", String(periodicDraft.minimumCoherence)],
    ];
    fields.forEach(([id, labelText, value]) => { const label = document.createElement("label"); label.className = "studio-field"; label.textContent = labelText; const input = document.createElement("input"); input.type = id === "periodic-frequencies" ? "text" : "number"; input.value = value; input.dataset.analysisPeriodic = id; input.setAttribute(`data-analysis-${id}`, ""); if (id === "periodic-sample-time") input.readOnly = true; input.addEventListener("input", () => { periodicDraft = { ...periodicDraft, ...(id === "periodic-frequencies" ? { frequencies: input.value } : id === "periodic-sample-time" ? { sampleTimeSeconds: Number(input.value) } : id === "periodic-segments" ? { segments: Number(input.value) } : id === "periodic-coherence" ? { minimumCoherence: Number(input.value) } : { startupSamples: Number(input.value) }) }; }); label.append(input); editor.append(label); });
  };
  const renderLeadLagEditor = (): void => {
    const mode = document.createElement("select"); mode.dataset.analysisLeadLagMode = "true"; mode.setAttribute("data-analysis-lead-lag-mode", ""); ["lead", "lag"].forEach(value => { const option = new Option(value === "lead" ? "Lead compensation" : "Lag compensation", value); mode.append(option); }); mode.value = leadLagMode;
    const modeLabel = document.createElement("label"); modeLabel.className = "studio-field"; modeLabel.textContent = "Compensation"; modeLabel.append(mode); editor.append(modeLabel);
    const crossover = document.createElement("input"); crossover.type = "number"; crossover.step = "any"; crossover.value = String(leadLagDraft.crossoverRadPerSecond); crossover.setAttribute("data-analysis-lead-lag-crossover", ""); crossover.addEventListener("input", () => { leadLagDraft.crossoverRadPerSecond = Number(crossover.value); }); const crossoverLabel = document.createElement("label"); crossoverLabel.className = "studio-field"; crossoverLabel.textContent = "Crossover (rad/s)"; crossoverLabel.append(crossover); editor.append(crossoverLabel);
    const value = document.createElement("input"); value.type = "number"; value.step = "any"; value.value = String(leadLagMode === "lead" ? leadLagDraft.phaseDegrees : leadLagDraft.lowFrequencyGainRatio); value.setAttribute(`data-analysis-lead-lag-${leadLagMode === "lead" ? "phase" : "ratio"}`, ""); value.addEventListener("input", () => { if (leadLagMode === "lead") leadLagDraft.phaseDegrees = Number(value.value); else leadLagDraft.lowFrequencyGainRatio = Number(value.value); }); const valueLabel = document.createElement("label"); valueLabel.className = "studio-field"; valueLabel.textContent = leadLagMode === "lead" ? "Phase contribution (°)" : "Low-frequency gain ratio"; valueLabel.append(value); editor.append(valueLabel); mode.addEventListener("change", () => { leadLagMode = mode.value as typeof leadLagMode; render(); });
  };
  const renderResponseEditor = (): void => {
    const selectedModelValue = model?.model ?? selectedModel; if (!selectedModelValue) return;
    const states = selectedModelValue.kind === "transfer-function" ? selectedModelValue.denominator.length - 1 : selectedModelValue.states.length;
    const defaults = { step: String(responseDraft.stepSeconds), duration: String(responseDraft.durationSeconds), initial: responseDraft.initialState || Array(states).fill(0).join(","), input: responseDraft.input || Array(selectedModelValue.inputs.length).fill(0).join(",") };
    [["response-step", "Step (s)", defaults.step], ["response-duration", "Duration (s)", defaults.duration], ["response-initial", "Initial state", defaults.initial], ["response-input", "Held input", defaults.input]].forEach(([id, labelText, value]) => { const label = document.createElement("label"); label.className = "studio-field"; label.textContent = labelText; const input = document.createElement("input"); input.type = "text"; input.value = value; input.setAttribute(`data-analysis-${id}`, ""); label.append(input); editor.append(label); });
    const connection = document.createElement("select"); connection.setAttribute("data-analysis-response-connection", ""); ["open-loop", "feedback"].forEach(value => { const option = new Option(value === "open-loop" ? "Open loop" : "Declared feedback", value); connection.append(option); }); connection.value = responseDraft.connection; const label = document.createElement("label"); label.className = "studio-field"; label.textContent = "Connection"; label.append(connection); editor.append(label);
    const output = document.createElement("select"); output.setAttribute("data-analysis-response-output", ""); selectedModelValue.outputs.forEach((item, index) => output.append(new Option(`${item.id} (${item.unit})`, String(index)))); output.value = String(Math.min(responseOutputIndex, selectedModelValue.outputs.length - 1)); responseOutputIndex = Number(output.value); output.addEventListener("change", () => { responseOutputIndex = Number(output.value); render(); }); const outputLabel = document.createElement("label"); outputLabel.className = "studio-field"; outputLabel.textContent = "Displayed output"; outputLabel.append(output); editor.append(outputLabel);
  };
  const renderPoleViewEditor = (): void => {
    const select = document.createElement("select"); select.setAttribute("data-analysis-pole-view", ""); ["unreduced", "minimal"].forEach(value => { const option = new Option(value === "unreduced" ? "Unreduced internal model" : "Minimal input/output view", value); select.append(option); }); select.value = poleViewChoice; const label = document.createElement("label"); label.className = "studio-field"; label.textContent = "Pole view"; label.append(select); editor.append(label);
  };
  const renderFeedbackSelector = (): void => {
    if (!plots?.nyquist?.length && !plots?.sensitivity?.length && !feedback && !(available.declared && selectedModel && declaredFeedbackDefinition(available.declared, selectedModel))) return;
    const select = document.createElement("select"); select.className = "trust-input"; ["bode", "nyquist", "sensitivity"].forEach(value => { const option = new Option(value[0].toUpperCase() + value.slice(1), value); select.append(option); });
    select.dataset.analysisFeedbackView = "true"; select.value = feedbackView;
    const label = document.createElement("label"); label.className = "studio-field"; label.textContent = "View"; label.append(select); editor.append(label); select.addEventListener("change", () => { feedbackView = select.value as typeof feedbackView; render(); });
  };

  function renderRootLocus(): void {
    const source = declaredRoot ? { parameter: declaredRoot.parameter, selectedValue: declaredRoot.selectedValue, approximation: declaredRoot.approximation, branches: declaredRoot.branches }
      : plots?.rootLocus?.status === "qualified" ? plots.rootLocus : undefined; if (!source) return;
    const branches = source.branches;
    chart.setOption({ ...base("Real axis", "Imaginary axis", "Root locus"), series: branches.map(branch => ({ ...engineeringLine(`${formatEngineeringNumber(branch.gain)} gain`, 0, branch.poles.map(pole => [pole.real, pole.imaginary]), "solid"), type: "scatter", symbolSize: 9 })) }, { notMerge: true });
    addText(`Parameter ${source.parameter}; selected value ${formatEngineeringNumber(source.selectedValue)}.`); addDetailsText(source.approximation); addRows(["Gain", "Poles"], branches.map(branch => [formatEngineeringNumber(branch.gain), branch.poles.map(pole => `(${formatEngineeringNumber(pole.real)}, ${formatEngineeringNumber(pole.imaginary)})`).join("; ")]));
  }
  function renderRouth(): void {
    if (!routh) return; const polynomial = routh.polynomial;
    addText(`${routh.parameter} = ${formatEngineeringNumber(routh.selectedValue)}. ${polynomial.interpretation}`); addRows(["Power of s", "Entries"], polynomial.rows.map(row => [String(row.power), row.entries.map(formatEngineeringNumber).join(", ")]));
    if (polynomial.symbolicRows?.length) addRows(["Power of s", "Exact symbolic entries"], polynomial.symbolicRows.map(row => [String(row.power), row.entries.join(", ")]));
    const interval = routh.interval;
    const displayBound = (value: number | undefined, infinity: string): string => value === undefined ? infinity : formatEngineeringNumber(value);
    const bounds = interval.intervals?.map(value => `(${displayBound(value.lower, "−∞")}, ${displayBound(value.upper, "∞")})`).join(" ∪ ") ?? (interval.kind === "open" ? `(${displayBound(interval.lower, "−∞")}, ${displayBound(interval.upper, "∞")})` : interval.kind);
    addText(`Stable interval: ${bounds}.`); addDetailsText(`First-column signs: ${polynomial.firstColumn.map(entry => entry.sign).join(", ")}. Allowed domain [${formatEngineeringNumber(routh.allowedDomain.minimum)}, ${formatEngineeringNumber(routh.allowedDomain.maximum)}]. ${interval.derivation}`);
    if (interval.boundaries?.length) addRows(["Boundary polynomial", "Lower", "Upper"], interval.boundaries.map(boundary => [boundary.polynomial.join(", "), boundary.lower, boundary.upper]));
  }
  function renderFrequency(): void {
    if (!model) return; const channel = model.result.channels[channelIndex] ?? model.result.channels[0];
    chart.setOption({ ...base("Frequency (rad/s)", "Magnitude / phase", `${channel.input.id} → ${channel.output.id}`), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), yAxis: [axis("Magnitude (dB)", "value", chartTheme()), axis("Phase (°)", "value", chartTheme())], series: [
      engineeringLine("Magnitude (dB)", 0, channel.points.map(point => [point.frequencyRadPerSecond, 20 * Math.log10(Math.max(Number.MIN_VALUE, point.magnitude))]), "solid"),
      { ...engineeringLine("Phase (°)", 1, channel.points.map(point => [point.frequencyRadPerSecond, point.phaseRadians * 180 / Math.PI]), "dashed"), yAxisIndex: 1 },
    ] }, { notMerge: true });
    addText(`Channel ${channel.input.id} → ${channel.output.id}`); addDetailsText(`Pole scope: ${model.result.poleScope}`); addRows(["Frequency (rad/s)", "Magnitude", "Phase (°)"], channel.points.slice(0, 128).map(point => [formatEngineeringNumber(point.frequencyRadPerSecond), formatEngineeringNumber(point.magnitude), formatEngineeringNumber(point.phaseRadians * 180 / Math.PI)]));
  }
  function renderPeriodicFrequency(): void {
    if (!periodicResult || periodicResult.status !== "qualified") { if (periodicResult?.status === "rejected") addText(`Measured frequency response unavailable: ${periodicResult.reason}`); return; }
    chart.setOption({ ...base("Frequency (rad/s)", "Magnitude / phase", "Measured periodic frequency response"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), yAxis: [axis("Magnitude (dB)", "value", chartTheme()), axis("Phase (°)", "value", chartTheme())], series: [{ ...engineeringLine("Measured magnitude", 0, periodicResult.points.map(point => [point.frequencyRadPerSecond, 20 * Math.log10(Math.max(Number.MIN_VALUE, point.magnitude))]), "solid"), showSymbol: true, symbol: "circle", symbolSize: 9 }, { ...engineeringLine("Measured phase", 1, periodicResult.points.map(point => [point.frequencyRadPerSecond, point.phaseRadians * 180 / Math.PI]), "dashed"), showSymbol: true, symbol: "circle", symbolSize: 8, yAxisIndex: 1 }] }, { notMerge: true });
    addText(`Measured ${periodicResult.points.length} frequency ${periodicResult.points.length === 1 ? "point" : "points"}.`); addRows(["Frequency (rad/s)", "Magnitude", "Phase (°)", "Coherence"], periodicResult.points.map(point => [formatEngineeringNumber(point.frequencyRadPerSecond), formatEngineeringNumber(point.magnitude), formatEngineeringNumber(point.phaseRadians * 180 / Math.PI), formatEngineeringNumber(point.coherence)]));
  }
  function renderPoles(): void {
    if (!model) return;
    const selectedView = poleView ? (poleView.view === "minimal" ? poleView.minimal : poleView.unreduced) : undefined;
    const discrete = model.model.kind === "discrete-state-space";
    const poles = selectedView ? rootsOfRealPolynomial(selectedView.denominator) ?? [] : model.result.poles;
    const zeros = selectedView ? rootsOfRealPolynomial(selectedView.numerator) ?? [] : model.model.kind === "transfer-function" ? rootsOfRealPolynomial(model.model.numerator) ?? [] : [];
    const series: Record<string, unknown>[] = [
      { ...engineeringLine("Poles", 0, poles.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "circle", symbolSize: 11 },
      ...(zeros.length ? [{ ...engineeringLine("Zeros", 0, zeros.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "diamond", symbolSize: 12 }] : []),
    ];
    if (discrete) series.push({ name: "Unit circle", type: "line", showSymbol: false, data: Array.from({ length: 65 }, (_, index) => { const angle = index * 2 * Math.PI / 64; return [Math.cos(angle), Math.sin(angle)]; }), lineStyle: { type: "dashed" } });
    chart.setOption({ ...base(discrete ? "Real axis (z)" : "Real axis (s)", discrete ? "Imaginary axis (z)" : "Imaginary axis (s)", discrete ? "Discrete pole-zero map" : "Continuous pole-zero map"), series }, { notMerge: true });
    addText(discrete ? "Discrete z-plane; the unit circle is the stability boundary." : "Continuous s-plane; negative real parts are the local model stability direction.");
    if (poleView?.warning) addText(poleView.warning);
    addRows(["Kind", "Coordinates"], [
      ...poles.map(point => ["Pole", `(${formatEngineeringNumber(point.real)}, ${formatEngineeringNumber(point.imaginary)})`]),
      ...zeros.map(point => ["Zero", `(${formatEngineeringNumber(point.real)}, ${formatEngineeringNumber(point.imaginary)})`]),
    ]);
  }
  function renderLeadLag(): void {
    if (!leadLagResult) return;
    const variable = leadLagMode === "lead" ? "s" : "s";
    addText(`${leadLagMode === "lead" ? "Lead" : "Lag"} compensator: C(${variable}) = ${formatEngineeringNumber(leadLagResult.gain)} · (1 + ${variable}/${formatEngineeringNumber(leadLagResult.zeroRadPerSecond)}) / (1 + ${variable}/${formatEngineeringNumber(leadLagResult.poleRadPerSecond)}).`);
    addDetailsText("This is an unapplied design result; no runtime controller was changed.");
    const result = leadLagResult as Record<string, unknown>;
    const labels: Record<string, string> = { gain: "Gain", zeroRadPerSecond: "Zero (rad/s)", poleRadPerSecond: "Pole (rad/s)", phaseContributionDegrees: "Phase contribution (°)", lowFrequencyGainRatio: "Low-frequency gain ratio", openLoopMagnitudeAtCrossover: "Open-loop magnitude at crossover" };
    addRows(["Quantity", "Value"], Object.entries(result).filter(([key, value]) => key !== "status" && typeof value === "number").map(([key, value]) => [labels[key] ?? key, formatEngineeringNumber(value as number)]));
  }
  function renderResponse(): void {
    if (!timeResponse) return;
    const outputIndex = Math.min(responseOutputIndex, timeResponse.model.outputs.length - 1); responseOutputIndex = Math.max(0, outputIndex); const output = timeResponse.model.outputs[responseOutputIndex];
    chart.setOption({ ...base("Time (s)", output?.unit ? `Output deviation (${output.unit})` : "Output deviation", "Initial-state response"), series: [engineeringLine("Zero-state input", 0, timeResponse.points.map(point => [point.timeSeconds, point.zeroState[outputIndex]]), "solid"), engineeringLine("Initial condition", 0, timeResponse.points.map(point => [point.timeSeconds, point.initialCondition[outputIndex]]), "dashed"), engineeringLine("Total", 0, timeResponse.points.map(point => [point.timeSeconds, point.total[outputIndex]]), "solid")] }, { notMerge: true });
    addText(output ? `${output.id} initial-state response` : "Initial-state response"); addDetailsText(`${timeResponse.scope} Transform: ${timeResponse.transform}`); addRows(["Time (s)", "Zero state", "Initial condition", "Total"], timeResponse.points.slice(0, 256).map(point => [formatEngineeringNumber(point.timeSeconds), formatEngineeringNumber(point.zeroState[outputIndex]), formatEngineeringNumber(point.initialCondition[outputIndex]), formatEngineeringNumber(point.total[outputIndex])]));
  }
  function renderPoleView(): void {
    if (!poleView) return;
    const selected = poleView.view === "minimal" ? poleView.minimal : poleView.unreduced;
    const poles = rootsOfRealPolynomial(selected.denominator) ?? []; const zeros = rootsOfRealPolynomial(selected.numerator) ?? [];
    chart.setOption({ ...base(`Real axis (${poleView.domain})`, `Imaginary axis (${poleView.domain})`, `${poleView.view} pole-zero view`), series: [{ ...engineeringLine("Poles", 0, poles.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "circle", symbolSize: 10 }, { ...engineeringLine("Zeros", 0, zeros.map(point => [point.real, point.imaginary]), "solid"), type: "scatter", symbol: "diamond", symbolSize: 10 }] }, { notMerge: true });
    addText(poleView.warning ?? ""); addRows(["View", "Cancelled factor"], [[poleView.view, poleView.cancelledFactor.map(formatEngineeringNumber).join(", ") || "none"]]);
  }
  function renderFeedback(): void {
    const selectedView = feedbackView;
    if (selectedView === "nyquist" && (plots?.nyquist || feedback?.openLoop)) {
      const values = plots?.nyquist?.map(point => [point.real, point.imaginary]) ?? feedback!.openLoop.map(point => [point.real, point.imaginary]);
      chart.setOption({ ...base("Real", "Imaginary", "Nyquist"), series: [engineeringLine(plots?.nyquist ? "Open-loop contour" : "Positive frequency", 0, values, "solid"),
        ...(!plots?.nyquist ? [engineeringLine("Negative frequency", 0, values.map(([real, imaginary]) => [real, -imaginary]), "dashed")] : []),
        { type: "scatter", name: "−1", data: [[-1, 0]], symbolSize: 12 }] }, { notMerge: true });
      const nyquist = feedback?.nyquistStability;
      addText(nyquist?.status === "qualified"
        ? `Closed loop ${nyquist.closedLoopStable ? "stable" : "unstable"}. Encirclements of −1 (counterclockwise): ${nyquist.encirclementsCounterClockwise}.`
        : nyquist?.status === "rejected" && nyquist.reason === "insufficient-contour-closure"
          ? "Stability not determined. Extend the frequency range and calculate again."
          : "Stability not determined from this sampled response.");
      if (feedback?.nyquistScope) addDetailsText(feedback.nyquistScope);
      return;
    }
    if (selectedView === "sensitivity" && (plots?.sensitivity || feedback?.sensitivity)) {
      const points = plots?.sensitivity ?? feedback?.sensitivity;
      if (!points) return;
      chart.setOption({ ...base("Frequency (rad/s)", "Magnitude (dB)", "Sensitivity"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), series: [engineeringLine("S", 0, points.map(point => [point.frequencyRadPerSecond, point.sensitivityMagnitudeDb]), "solid"), engineeringLine("T", 0, points.map(point => [point.frequencyRadPerSecond, point.complementarySensitivityMagnitudeDb]), "dashed"), engineeringLine("CS", 0, points.map(point => [point.frequencyRadPerSecond, point.controlSensitivityMagnitudeDb]), "dashed")] }, { notMerge: true }); return;
    }
    const bode = plots?.candidateOpenLoopBode ?? feedback?.openLoop.map(point => ({
      frequencyRadPerSecond: point.frequencyRadPerSecond,
      magnitudeDb: 20 * Math.log10(Math.max(Number.MIN_VALUE, point.magnitude)),
    }));
    if (bode?.length) chart.setOption({ ...base("Frequency (rad/s)", "Magnitude (dB)", "Bode"), xAxis: axis("Frequency (rad/s)", "log", chartTheme()), series: [{ ...engineeringLine("Loop", 0, bode.map(point => [point.frequencyRadPerSecond, point.magnitudeDb]), "solid") }] }, { notMerge: true });
    const margins = plots?.gainMarginDb !== undefined ? `Gain margin ${String(plots.gainMarginDb)} dB; phase margin ${formatEngineeringNumber(plots.phaseMarginDegrees ?? NaN)}°.` : feedback?.margins.status === "estimated" ? `Gain margin ${String(feedback.margins.gainMarginDb ?? "unavailable")} dB; phase margin ${formatEngineeringNumber(feedback.margins.phaseMarginDegrees)}°.` : "Margins unavailable."; addText(margins);
    const nyquist = feedback?.nyquistStability;
    if (nyquist) addText(nyquist.status === "qualified"
      ? `Nyquist contour: ${nyquist.encirclementsCounterClockwise} counter-clockwise encirclements; ${nyquist.openLoopRightHalfPlanePoles} open-loop and ${nyquist.closedLoopRightHalfPlanePoles} closed-loop right-half-plane poles. Closed-loop ${nyquist.closedLoopStable ? "stable" : "not stable"} within the shared contour limits.`
      : `Nyquist contour unavailable: ${nyquist.reason}`);
  }
  function renderStateStructure(): void {
    if (!model || model.model.kind === "transfer-function" || !model.result.stateStructure) return; const structure = model.result.stateStructure; addText(`Controllability rank ${structure.status === "qualified" ? structure.controllability.rank : "unavailable"}; observability rank ${structure.status === "qualified" ? structure.observability.rank : "unavailable"}.`); if (structure.status === "qualified") { addMatrix("Controllability", structure.controllability.matrix); addMatrix("Observability", structure.observability.matrix); }
  }
  function renderRga(): void {
    if (!model?.result.relativeGainMatrix) return;
    addVisibleMatrix("Relative gain array (RGA)", model.result.relativeGainMatrix);
  }
  const addText = (value: string): void => { const p = document.createElement("p"); p.textContent = value; summary.append(p); };
  const addDetailsText = (value: string): void => { const p = document.createElement("p"); p.textContent = value; detailsSummary.append(p); tableDetails.hidden = false; };
  const addRows = (headers: readonly string[], rows: readonly (readonly string[])[]): void => { tableDetails.hidden = false; const element = document.createElement("table"); const head = element.createTHead().insertRow(); headers.forEach(value => { const cell = document.createElement("th"); cell.textContent = value; head.append(cell); }); const body = element.createTBody(); rows.forEach(row => { const tr = body.insertRow(); row.forEach(value => { const cell = tr.insertCell(); cell.textContent = value; }); }); table.append(element); };
  const addMatrix = (label: string, matrix: readonly (readonly number[])[]): void => { addRows([label, "Values"], matrix.map((row, index) => [String(index + 1), row.map(formatEngineeringNumber).join(", ")])); };
  const addVisibleMatrix = (label: string, matrix: readonly (readonly number[])[]): void => {
    const element = document.createElement("table"); element.className = "trust-table"; element.createCaption().textContent = label;
    const head = element.createTHead().insertRow(); head.append(document.createElement("th"));
    matrix[0]?.forEach((_, index) => { const th = document.createElement("th"); th.scope = "col"; th.textContent = model?.model.inputs[index]?.id ?? `Input ${index + 1}`; head.append(th); });
    const body = element.createTBody();
    matrix.forEach((row, index) => { const tr = body.insertRow(); const name = document.createElement("th"); name.scope = "row"; name.textContent = model?.model.outputs[index]?.id ?? `Output ${index + 1}`; tr.append(name); row.forEach(value => { tr.insertCell().textContent = formatEngineeringNumber(value); }); });
    summary.append(element);
  };
  function base(xName: string, yName: string, label: string): Record<string, unknown> { const theme = chartTheme(); return { animation: false, aria: { enabled: true, description: label }, backgroundColor: theme.background, legend: { top: 4, textStyle: { color: theme.muted } }, tooltip: { trigger: "axis", valueFormatter: formatChartValue }, grid: { left: 64, right: 24, top: 36, bottom: 52 }, xAxis: axis(xName, "value", theme), yAxis: axis(yName, "value", theme) }; }
  const dispose = (): void => { disposed = true; resizeObserver.disconnect(); window.removeEventListener("resize", resize); chart.dispose(); };
  render();
  return { receive, setAvailable, setTool, run, actionLabel, choice, availableTools: toolSet, dispose };
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function transferModel(model: GeneralModel): import("../numerics/classicalAnalysis").TransferFunction | undefined {
  try { const transfer = linearTransferModel(model); return { numerator: [...transfer.numerator], denominator: [...transfer.denominator], delaySeconds: transfer.delaySeconds }; }
  catch { return undefined; }
}
function hasRawRecording(payload: Record<string, unknown>): boolean { const recording = isRecord(payload.recording) ? payload.recording : payload; return Array.isArray(recording.rawSamples); }
function parseFiniteNumber(value: string): number | undefined { if (!value.trim()) return undefined; const parsed = Number(value); return Number.isFinite(parsed) ? parsed : undefined; }
function parseNumberList(value: string, allowEmpty = false): number[] | undefined {
  if (!value.trim()) return allowEmpty ? [] : undefined;
  const values = value.split(",").map(item => item.trim());
  if (values.some(item => !item)) return undefined;
  const parsed = values.map(Number);
  return parsed.every(Number.isFinite) ? parsed : undefined;
}
function decodePeriodicRecording(payload: Record<string, unknown>): { input: number[]; output: number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string } | undefined {
  const recording = isRecord(payload.recording) ? payload.recording : payload;
  if (!Array.isArray(recording.rawSamples)) return undefined;
  const usable = recording.rawSamples.filter(isRecord);
  const evidence = isRecord(recording.evidence) ? recording.evidence : isRecord(recording.acquisition) ? recording.acquisition : undefined;
  if (usable.length < 32 || evidence?.completeness !== "complete" || !Array.isArray(evidence.missingScanRanges) || evidence.missingScanRanges.length !== 0
    || evidence.droppedObservedSamples !== 0 || evidence.actualCycleTimingAvailable !== true
    || usable.some((sample, index) => {
      if (sample.quality !== "good" || typeof sample.appliedMv !== "number" || !Number.isFinite(sample.appliedMv) || typeof sample.pv !== "number" || !Number.isFinite(sample.pv)
        || (isRecord(sample.outputLimits) && (sample.appliedMv === sample.outputLimits.minimum || sample.appliedMv === sample.outputLimits.maximum))) return true;
      if (index === 0) return false;
      const previous = usable[index - 1];
      return typeof sample.scan !== "number" || typeof previous.scan !== "number" || sample.scan !== previous.scan + 1 || sample.loopRevision !== usable[0].loopRevision;
    })) return undefined;
  const sampleTimeSeconds = typeof usable[0].actualCycleSeconds === "number" && Number.isFinite(usable[0].actualCycleSeconds) ? usable[0].actualCycleSeconds : undefined;
  if (!sampleTimeSeconds || !Number.isFinite(sampleTimeSeconds) || sampleTimeSeconds <= 0) return undefined;
  if (usable.some(sample => typeof sample.actualCycleSeconds !== "number" || !Number.isFinite(sample.actualCycleSeconds)
      || Math.abs(sample.actualCycleSeconds - sampleTimeSeconds) > 1e-6 * Math.max(1, sampleTimeSeconds))) return undefined;
  const provenance = isRecord(recording.provenance) ? recording.provenance : undefined;
  const recordingId = typeof evidence?.recordingId === "string" ? evidence.recordingId : undefined;
  const recordingRevision = typeof provenance?.resultRevision === "number" ? String(provenance.resultRevision) : typeof usable[0].loopRevision === "string" ? usable[0].loopRevision : undefined;
  if (!recordingId || !recordingRevision) return undefined;
  return { input: usable.map(sample => sample.appliedMv as number), output: usable.map(sample => sample.pv as number), sampleTimeSeconds, recordingId, recordingRevision };
}
function modelStudy(value: unknown): SavedGeneralModelAnalysis | undefined {
  if (isSavedGeneralModelAnalysis(value)) return value;
  if (!isRecord(value) || !isRecord(value.model) || !isRecord(value.grid) || !isRecord(value.result)) return undefined;
  const model = decodeGeneralModel(value.model); const result = value.result as GeneralModelFrequencyResult;
  const grid = value.grid;
  if (!model || result.status !== "qualified" || !validModelGrid(grid, model.inputs.length * model.outputs.length)) return undefined;
  return { model, grid: { minimumFrequency: grid.minimumFrequency, maximumFrequency: grid.maximumFrequency, points: grid.points }, result };
}
function emptyPresenter(): ThreePageAnalysisPresenter { return { receive: () => undefined, setAvailable: () => [], setTool: () => undefined, run: () => undefined, actionLabel: () => undefined, choice: () => undefined, availableTools: () => [], dispose: () => undefined }; }
