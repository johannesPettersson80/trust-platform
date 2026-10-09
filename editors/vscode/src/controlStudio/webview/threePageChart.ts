import { decodeIdentificationExperiment, type IdentificationExperiment } from "../identificationExperiment";
import { LineChart } from "echarts/charts";
import { AriaComponent, GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { axis, chartTheme } from "./chartPrimitives";
import type { IdentifiedModelPresentation } from "../identification";
import type { StudioPage } from "./threePageHostState";
import { displayEngineeringUnit } from "../chartContract";
import type { EngineeringPlotMessageEvidence } from "../engineeringPlots";
import type { RunEvaluation } from "../runValidation";
import type { ControlLoopDefinition } from "../loopDefinition";
import { loopSignal } from "../loopChannels";
import { experimentInputBinding, experimentSignalBindings, recordedSignalResponses } from "./threePageRecordedSignals";
import { responseChartSeries, responseChartTooltip } from "./threePageResponseSeries";
import { responseRowAxis, responseRowGrids } from "./responseRows";
import type { WorkflowResultSample } from "../workflowResult";
import { decodeLoopMonitorEnvelope } from "../messages";
import { ThreePageLiveSignals, type SignalResponseGroup } from "./threePageLiveSignals";
import { signalRoleName, signalRoleTitle } from "./signalRoles";

type ResponseGroup = SignalResponseGroup;

echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, AriaComponent, CanvasRenderer]);
const host = document.querySelector<HTMLElement>("[data-control-studio-chart]");
if (host) {
  const root = document.querySelector<HTMLElement>("[data-studio-three-page]")!;
  const loop = JSON.parse(root.dataset.studioLoop ?? "null") as ControlLoopDefinition | null;
  const chart = echarts.init(host, undefined, { renderer: "canvas" });
  // The full symbols of the loop's signals show on hover; the legend and axes name them by role.
  if (loop) host.title = signalRoleTitle(loop);
  const empty = root.querySelector<HTMLElement>("[data-studio-chart-empty]");
  const plots: Partial<Record<StudioPage, ResponseGroup[]>> = {};
  let live = loop ? new ThreePageLiveSignals(loop) : undefined;
  let recordedSource = false;
  let livePage: StudioPage = "model";
  const selected = (kind: string) => Array.from(root.querySelector<HTMLSelectElement>(`[data-studio-${kind}]`)?.selectedOptions ?? [], option => option.value);
  let page: StudioPage = "model";
  let recording: readonly WorkflowResultSample[] = [];
  let selectedSignals: readonly string[] = [];
  const render = () => {
    const theme = chartTheme();
    const groups = plots[page] ?? (page === livePage && !recordedSource ? live?.responses(selected("inputs"), selected("outputs"),
      root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")?.value === "project"
        ? Array.from(root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]")?.selectedOptions ?? [], option => option.value) : undefined) : undefined) ?? [];
    // Tune has no plot before its first prediction; Identify and Validate keep their plot area and say what it waits for.
    host.closest<HTMLElement>(".studio-response")!.hidden = groups.length === 0 && page === "tune";
    if (empty) { empty.hidden = groups.length > 0; empty.textContent = groups.length ? "" : live?.emptyMessage() ?? "Start the runtime to see live values"; }
    const title = root.querySelector<HTMLElement>("[data-studio-response-title]")!;
    title.textContent = page === "tune" ? "Predicted response" : page === "verify" ? "Measured response" : "";
    title.hidden = !groups.length || !title.textContent;
    const rows: ResponseGroup[] = groups.length ? groups : [{ name: "", unit: "", series: [] }];
    // The Apply block sits above the plot on Validate, so the plot needs no room for it.
    const applicationHeight = 24;
    const available = window.innerHeight - (host.getBoundingClientRect().top + window.scrollY) - applicationHeight;
    const minimumRowHeight = page === "model" && rows.length > 1 ? 180 : 240;
    host.style.minHeight = `${rows.length * minimumRowHeight}px`;
    host.style.height = `${Math.max(rows.length * minimumRowHeight, available)}px`;
    const height = Math.max(host.clientHeight, rows.length * minimumRowHeight);
    const times = groups.flatMap(group => group.series.flatMap(series => series.points.map(point => point[0]).filter((time): time is number => time !== null)));
    const starts = groups.flatMap(group => group.timeStart === undefined ? [] : [group.timeStart]);
    const timeRange = times.length ? [...times, ...starts].reduce(([low, high], time) => [Math.min(low, time), Math.max(high, time)], [Infinity, -Infinity]) : undefined;
    const series = responseChartSeries(theme, loop, groups);
    const labels = new Map(groups.flatMap(group => group.series.map(item => [item.name, item.label ?? item.name] as const)));
    const description = groups.length ? `Response plot: ${[...labels.values()].join(", ")}. ${groups.map(group => `${group.name} (${displayEngineeringUnit(group.unit)})`).join(", ")}. Time in seconds.` : "Response plot";
    if (!groups.length) chart.clear();
    else chart.setOption({
      animation: false, aria: { enabled: true, description },
      backgroundColor: theme.background,
      color: [theme.pv, theme.auxiliaryOrange, theme.sp, theme.auxiliaryPurple],
      legend: { show: series.length > 0, top: 0, textStyle: { color: theme.foreground }, type: "scroll", formatter: (name: string) => labels.get(name) ?? name },
      tooltip: responseChartTooltip(),
      grid: responseRowGrids(rows.length, height),
      xAxis: rows.map((group, index) => ({ ...axis("Time (s)", "value", theme),
        ...(timeRange ? { min: timeRange[0], max: timeRange[1] } : {}), show: groups.length > 0, gridIndex: index })),
      yAxis: rows.map((group, index) => {
        const values = group.series.flatMap(item => item.points.map(point => point[1]));
        // A stationary zero-percent actuator needs a readable engineering scale,
        // not millionths of a percent. Moving responses retain fit-to-data axes.
        const zeroSpan = group.zeroSpan && values.some(value => value === 0) && values.every(value => value === null || value === 0) ? group.zeroSpan : undefined;
        return responseRowAxis(group.name ? `${group.name} (${displayEngineeringUnit(group.unit)})` : "", theme,
          zeroSpan ? [-zeroSpan, zeroSpan] : values, index, groups.length > 0, height / rows.length);
      }),
      series,
    }, { notMerge: true });
    chart.resize();
    host.dataset.seriesCount = String(series.length);
    host.dataset.timeRange = JSON.stringify(timeRange ?? []);
    host.dataset.pointCount = String(groups.reduce((sum, group) => sum + group.series.reduce((count, item) => count + item.points.length, 0), 0));
    host.setAttribute("aria-label", description);
  };
  window.addEventListener("message", event => {
    const envelope = decodeLoopMonitorEnvelope(event.data, {
      projectId: root.dataset.projectId ?? "", editorSessionId: root.dataset.editorSessionId ?? "",
      loopId: root.dataset.loopId ?? "", loopRevision: root.dataset.loopRevision ?? "",
    });
    if (!envelope || !live) return;
    live.receive(envelope.payload);
    if (page === livePage && !plots[page] && !recordedSource) render();
  });
  for (const select of root.querySelectorAll("[data-studio-inputs],[data-studio-outputs],[data-engineer-acquisition-method]")) {
    select.addEventListener("change", render);
    new MutationObserver(render).observe(select, { childList: true, subtree: true });
  }
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent<Record<string, unknown>>).detail;
    if (payload.liveIdentification === true && loop) {
      delete plots.model; recordedSource = false; livePage = "model"; live = new ThreePageLiveSignals(loop);
    }
    if (payload.liveModelValidation === true && loop) {
      delete plots.verify; recordedSource = false; livePage = "verify"; live = new ThreePageLiveSignals(loop);
    }
    if (payload.recordingCatalog) recordedSource = true;
    if (["edited", "model-ready"].includes(String(payload.status))) { delete plots.tune; delete plots.verify; }
    const experiment = decodeIdentificationExperiment(payload.identificationExperiment);
    if (experiment) { plots.model = experimentResponses(experiment, true, loop); recordedSource = true; }
    const presentation = payload.identificationPresentation as IdentifiedModelPresentation | undefined;
    if (presentation?.predictedResponse?.length) {
      const outputs = presentation.outputFits?.length ? presentation.outputFits : [{ id: presentation.outputId ?? JSON.parse(root.dataset.studioLoop ?? "null")?.signals.processVariable.symbol ?? "Measured output",
        unit: presentation.units.output, points: presentation.predictedResponse }];
      const fittedExperiment = decodeIdentificationExperiment(presentation.experiment);
      const retained = loop && recording.length ? recordedSignalResponses(loop, recording, selectedSignals) : [];
      const retainedSeries = retained.flatMap(group => group.series);
      plots.model = outputs.map((output, index) => ({ name: signalRoleName(loop, output.id) ?? output.id, unit: output.unit, series: [
        { signalId: loop ? experimentSignalBindings(loop).find(signal => signal.symbol === output.id)?.id : undefined, name: outputs.length === 1 ? "Measured" : `${signalRoleName(loop, output.id) ?? output.id} · Measured`, points: fittedExperiment?.outputs[index]?.points.map(point => [point.timeSeconds, point.value])
          ?? retainedSeries.find(series => series.name === output.id || outputs.length === 1 && series.signalId === "pv")?.points
          ?? output.points.map(point => [point.timeSeconds, point.measured]) },
        { prediction: true, name: outputs.length === 1 ? "Model prediction" : `${signalRoleName(loop, output.id) ?? output.id} · Model prediction`, points: output.points.map(point => [point.timeSeconds, point.predicted]), dashed: true },
      ] }));
      if (fittedExperiment) { plots.model.push(...experimentResponses(fittedExperiment, false, loop)); recordedSource = true; }
      else if (retained.length) plots.model.push(...retained.map(group => ({ ...group,
        series: group.series.filter(series => selected("inputs").includes(series.name)) })).filter(group => group.series.length > 0));
    }
    const prediction = payload.plots as EngineeringPlotMessageEvidence | undefined;
    if (loop && prediction?.timeResponse?.length) {
      const points = prediction.timeResponse;
      plots.tune = [{ name: signalRoleName(loop, loop.signals.processVariable.symbol) ?? loop.signals.processVariable.symbol, unit: loop.signals.processVariable.unit, series: [
        { name: "Tuned", points: points.map(point => [point.timeSeconds, point.candidatePv]) },
        ...(points.some(point => typeof point.currentPv === "number") ? [{ name: "Current", dashed: true, points: points.map(point => [point.timeSeconds, point.currentPv ?? null]) }] : []),
      ] }];
    }
    const evaluation = payload.candidate as RunEvaluation | undefined;
    if (loop && evaluation?.run?.samples?.length) plots.verify = runResponses(loop, evaluation, payload.baseline as RunEvaluation | undefined);
    const modelPrediction = payload.modelPrediction as { baseline: RunEvaluation; candidate: RunEvaluation } | undefined;
    if (loop && modelPrediction?.candidate?.run?.samples?.length) plots.tune = runResponses(loop, modelPrediction.candidate, modelPrediction.baseline);
    render();
  });
  window.addEventListener("trust-control-studio-three-page", event => {
    page = (event as CustomEvent<{ page: StudioPage }>).detail.page;
    render();
  });
  window.addEventListener("trust-control-studio-experiment-started", () => {
    if (!loop) return;
    recording = []; delete plots.model; recordedSource = false; livePage = "model";
    live = new ThreePageLiveSignals(loop); render();
  });
  const showRecording = (samples: readonly WorkflowResultSample[], signals: readonly string[]) => {
    recording = samples; selectedSignals = signals; recordedSource = true;
    if (loop) plots.model = recordedSignalResponses(loop, recording, selectedSignals);
    render();
  };
  window.addEventListener("trust-control-studio-experiment-recording", event => {
    const detail = (event as CustomEvent<{ recording: { rawSamples?: readonly WorkflowResultSample[] }; signalIds: readonly string[] }>).detail;
    showRecording(detail.recording.rawSamples ?? [], detail.signalIds);
  });
  window.addEventListener("trust-control-studio-recording-opened", event => {
    if (!loop) return;
    const detail = (event as CustomEvent<{ recording: { rawSamples: readonly WorkflowResultSample[] } }>).detail;
    const symbols = [...selected("outputs"), ...selected("inputs")];
    showRecording(detail.recording.rawSamples, experimentSignalBindings(loop).filter(signal => symbols.includes(signal.symbol)).map(signal => signal.id));
  });
  window.addEventListener("trust-control-studio-experiment-signals", event => {
    selectedSignals = (event as CustomEvent<{ signalIds: readonly string[] }>).detail.signalIds;
    if (loop && recording.length) plots.model = recordedSignalResponses(loop, recording, selectedSignals);
    render();
  });
  render();
  new ResizeObserver(() => chart.resize()).observe(host);
  window.addEventListener("resize", render);
  new MutationObserver(render).observe(document.body, { attributes: true, attributeFilter: ["class", "style", "data-vscode-theme-kind"] });
}

function runResponses(loop: ControlLoopDefinition, candidate: RunEvaluation, baseline?: RunEvaluation): ResponseGroup[] {
  const run = candidate.run;
  const current = baseline?.run?.samples?.length && baseline.run.conditions.modelRevision === run.conditions.modelRevision
    && baseline.run.conditions.durationSeconds === run.conditions.durationSeconds ? baseline.run : undefined;
  const outputs = loop.channels?.outputs.map(channel => loopSignal(loop, channel.signalId)) ?? [loop.signals.processVariable];
  return outputs.map(signal => ({ name: signalRoleName(loop, signal.symbol) ?? signal.symbol, unit: signal.unit, timeStart: 0, series: [
    { name: outputs.length === 1 ? "Tuned" : `${signal.symbol} · Tuned`, label: outputs.length === 1 ? undefined : `${signalRoleName(loop, signal.symbol) ?? signal.symbol} · Tuned`, points: run.samples.map(sample => [sample.timeSeconds, sample.values[signal.symbol]?.quality === "good" ? sample.values[signal.symbol].value : null]) },
    ...(current ? [{ name: outputs.length === 1 ? "Current" : `${signal.symbol} · Current`, label: outputs.length === 1 ? undefined : `${signalRoleName(loop, signal.symbol) ?? signal.symbol} · Current`, dashed: true,
      points: current.samples.map(sample => [sample.timeSeconds, sample.values[signal.symbol]?.quality === "good" ? sample.values[signal.symbol].value : null]) }] : []),
  ] }));
}

function experimentResponses(experiment: IdentificationExperiment, includeOutputs: boolean, loop?: ControlLoopDefinition | null): ResponseGroup[] {
  const bindings = loop ? experimentSignalBindings(loop) : [];
  const binding = (id: string) => bindings.find(signal => signal.symbol === id);
  const outputs: ResponseGroup[] = includeOutputs ? experiment.outputs.map(trace => ({ name: signalRoleName(loop, trace.id) ?? trace.id, unit: trace.unit,
    series: [{ signalId: binding(trace.id)?.id, name: `${signalRoleName(loop, trace.id) ?? trace.id} · Measured`, points: trace.points.map(point => [point.timeSeconds, point.value]) }] })) : [];
  const inputs = new Map<string, ResponseGroup>();
  for (const trace of experiment.inputs) {
    const group = inputs.get(trace.unit) ?? { name: "Applied input", unit: trace.unit, series: [] };
    const applied = loop ? experimentInputBinding(loop, trace.id) : undefined;
    group.series.push({ signalId: applied?.id, name: applied ? signalRoleName(loop, applied.symbol) ?? applied.symbol : `${trace.id} · Applied input`, points: trace.points.map(point => [point.timeSeconds, point.value]) });
    inputs.set(trace.unit, group);
  }
  return [...outputs, ...inputs.values()];
}
